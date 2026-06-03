// Copyright 2026 Memryzed contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Benchmark runner.
//!
//! Loads every document into a fresh in-memory Memryzed store, then
//! runs each question through the same hybrid retrieval the product
//! uses and computes recall at K: the fraction of questions whose
//! answer document appears in the top-K results.
//!
//! The embedder is whatever the environment selects. For a real
//! quality number, run with the BGE-small model active. With
//! `MEMRYZED_DISABLE_EMBEDDING=1` only the full-text leg contributes,
//! which is useful for a fast smoke run but is not a headline number.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::embedder::{make_default, Embedder};
use memryzed_core::memory::{insert_with_embedder, NewMemory, Scope};
use memryzed_core::retrieval::{search, SearchOptions};
use memryzed_core::{Database, NoopEmbedder};

use crate::dataset::Dataset;

/// Result of one benchmark run, serialized to JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    /// Dataset name.
    pub dataset: String,
    /// Memryzed version that produced the result.
    pub memryzed_version: String,
    /// Embedding model id, or "noop" when embedding was disabled.
    pub embedding_model: String,
    /// The K values reported.
    pub k_values: Vec<usize>,
    /// recall@K for each K in `k_values`, same order.
    pub recall_at_k: Vec<f64>,
    /// Number of questions evaluated.
    pub questions: usize,
    /// Number of documents in the store.
    pub documents: usize,
}

/// Run the benchmark over `dataset` for the given K values, using the
/// embedder selected by the environment and `models_dir` as its
/// cache.
pub fn run(
    dataset: &Dataset,
    k_values: &[usize],
    models_dir: &std::path::Path,
) -> anyhow::Result<BenchResult> {
    let embedder: Arc<dyn Embedder> = match make_default(models_dir) {
        Ok(e) => e,
        Err(_) => Arc::new(NoopEmbedder),
    };
    let (hits, n) = score_dataset(dataset, k_values, embedder.as_ref())?;
    let recall_at_k = hits.iter().map(|&h| h as f64 / n as f64).collect();
    Ok(BenchResult {
        dataset: dataset.name.clone(),
        memryzed_version: memryzed_core::VERSION.to_string(),
        embedding_model: embedder.model_id().to_string(),
        k_values: k_values.to_vec(),
        recall_at_k,
        questions: dataset.questions.len(),
        documents: dataset.documents.len(),
    })
}

/// Evaluate every `*.json` dataset in `dir` against its own haystack,
/// loading the embedding model only once, and aggregate recall@K
/// across all questions. This is the per-scene protocol used by
/// LongMemEval-S, where each question has its own multi-session
/// haystack.
pub fn run_scene_dir(
    dir: &std::path::Path,
    k_values: &[usize],
    models_dir: &std::path::Path,
) -> anyhow::Result<BenchResult> {
    let embedder: Arc<dyn Embedder> = match make_default(models_dir) {
        Ok(e) => e,
        Err(_) => Arc::new(NoopEmbedder),
    };

    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    files.sort();

    let mut agg_hits = vec![0usize; k_values.len()];
    let mut total_q = 0usize;
    let mut total_docs = 0usize;
    let n_files = files.len();
    for (i, path) in files.iter().enumerate() {
        let ds = Dataset::load(path)?;
        let (hits, n) = score_dataset(&ds, k_values, embedder.as_ref())?;
        for (a, h) in agg_hits.iter_mut().zip(hits.iter()) {
            *a += *h;
        }
        total_q += n;
        total_docs += ds.documents.len();
        if (i + 1) % 25 == 0 {
            eprintln!("  {}/{} scenes, {} questions", i + 1, n_files, total_q);
        }
    }

    let recall_at_k = agg_hits
        .iter()
        .map(|&h| h as f64 / total_q as f64)
        .collect();
    Ok(BenchResult {
        dataset: format!("{} (per-scene, {n_files} scenes)", dir.display()),
        memryzed_version: memryzed_core::VERSION.to_string(),
        embedding_model: embedder.model_id().to_string(),
        k_values: k_values.to_vec(),
        recall_at_k,
        questions: total_q,
        documents: total_docs,
    })
}

/// Load one dataset into a fresh store and return (hits_at_k, n_questions).
fn score_dataset(
    dataset: &Dataset,
    k_values: &[usize],
    embedder: &dyn Embedder,
) -> anyhow::Result<(Vec<usize>, usize)> {
    let mut db = Database::open_in_memory()?;
    let now = now_epoch_seconds();

    let mut content_to_doc: HashMap<String, String> = HashMap::new();
    for doc in &dataset.documents {
        if doc.text.trim().is_empty() {
            continue;
        }
        let mem = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, doc.text.clone()),
            embedder,
            now,
        )?;
        content_to_doc.insert(mem.id, doc.id.clone());
    }

    let max_k = k_values.iter().copied().max().unwrap_or(10);
    let mut hits_at_k: Vec<usize> = vec![0; k_values.len()];

    for q in &dataset.questions {
        let opts = SearchOptions {
            limit: max_k,
            ..Default::default()
        };
        let results = search(&db, embedder, &q.query, &opts)?;
        let ranked_doc_ids: Vec<&String> = results
            .iter()
            .filter_map(|r| content_to_doc.get(&r.memory.id))
            .collect();

        for (idx, &k) in k_values.iter().enumerate() {
            let hit = ranked_doc_ids
                .iter()
                .take(k)
                .any(|id| q.answer_doc_ids.iter().any(|a| a == *id));
            if hit {
                hits_at_k[idx] += 1;
            }
        }
    }
    Ok((hits_at_k, dataset.questions.len()))
}
