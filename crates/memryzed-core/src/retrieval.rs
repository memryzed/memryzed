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

//! Hybrid retrieval.
//!
//! Combines three signals into a single score:
//!
//! - Vector similarity. Cosine similarity between the query embedding
//!   and each candidate's stored embedding.
//! - Full-text relevance. BM25 score from the FTS5 index, normalized
//!   to `[0, 1]`.
//! - Recency. `exp(-age_days / 30)` capped at 1.0.
//!
//! Pinned memories receive a small additive boost so they sort above
//! equally-scored unpinned items.
//!
//! v0.1.0-alpha.4 ships the in-Rust pipeline with an embedding-model
//! filter (only memories embedded with the active model take the
//! vector path). When sqlite-vec is adopted in a later release the
//! vector lookup migrates to a virtual table; the public API here
//! does not change.

use std::collections::HashMap;

use crate::clock::now_epoch_seconds;
use crate::embedder::Embedder;
use crate::error::{Error, Result};
use crate::memory::{Memory, Scope, Status};
use crate::storage::Database;

/// Default weights when the caller does not supply a [`SearchOptions`].
pub const DEFAULT_VECTOR_WEIGHT: f32 = 0.6;
/// Default BM25 weight.
pub const DEFAULT_FTS_WEIGHT: f32 = 0.3;
/// Default recency weight.
pub const DEFAULT_RECENCY_WEIGHT: f32 = 0.1;
/// Additive pinned bonus (clamped so the final score never exceeds 1.0).
pub const PINNED_BONUS: f32 = 0.1;
/// Default number of results returned.
pub const DEFAULT_LIMIT: usize = 10;

/// Inputs to a hybrid retrieval call.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Restrict to a scope kind. `None` searches across every scope.
    pub scope: Option<Scope>,
    /// Restrict to a specific project or session ID.
    pub scope_id: Option<String>,
    /// Maximum results to return.
    pub limit: usize,
    /// Vector similarity weight.
    pub vector_weight: f32,
    /// FTS5 BM25 weight.
    pub fts_weight: f32,
    /// Recency weight.
    pub recency_weight: f32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            scope: None,
            scope_id: None,
            limit: DEFAULT_LIMIT,
            vector_weight: DEFAULT_VECTOR_WEIGHT,
            fts_weight: DEFAULT_FTS_WEIGHT,
            recency_weight: DEFAULT_RECENCY_WEIGHT,
        }
    }
}

/// One ranked result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The memory itself.
    pub memory: Memory,
    /// Combined hybrid score in `[0, 1]`.
    pub score: f32,
    /// Cosine similarity, when the candidate had a comparable embedding.
    pub vector_score: Option<f32>,
    /// Normalized BM25 score, when the candidate matched the FTS query.
    pub fts_score: Option<f32>,
    /// Recency boost.
    pub recency_score: f32,
}

/// Run a hybrid search.
///
/// Empty query strings are rejected. Result count is capped at the
/// caller-supplied `limit`.
pub fn search(
    db: &Database,
    embedder: &dyn Embedder,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(Error::Validation("search query must not be empty".into()));
    }
    let now = now_epoch_seconds();

    let mut candidates: HashMap<String, Candidate> = HashMap::new();

    // FTS leg.
    let fts_rows = fts_candidates(db, trimmed, options)?;
    if let Some(max_score) = fts_rows.iter().map(|r| r.bm25).fold(None, max_opt_f32) {
        for row in &fts_rows {
            // Lower BM25 in SQLite means a better match, and FTS5 returns
            // negative scores. Normalize to [0, 1] by inverting and dividing
            // by the best (most-negative) score in this query.
            let normalized = if max_score == 0.0 {
                0.0
            } else {
                (row.bm25 / max_score).clamp(0.0, 1.0)
            };
            candidates
                .entry(row.memory_id.clone())
                .or_default()
                .fts_score = Some(normalized);
        }
    }

    // Vector leg.
    let query_embedding = if embedder.is_active() {
        let mut v = embedder.embed(&[trimmed])?;
        v.pop().unwrap_or_default()
    } else {
        Vec::new()
    };
    if !query_embedding.is_empty() {
        let candidates_with_emb = vector_candidates(db, embedder.model_id(), options)?;
        for (memory_id, embedding) in candidates_with_emb {
            let score = cosine_similarity(&query_embedding, &embedding);
            candidates.entry(memory_id).or_default().vector_score = Some(score);
        }
    }

    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // Hydrate the surviving candidate IDs into Memory rows.
    let ids: Vec<String> = candidates.keys().cloned().collect();
    let memories = load_memories(db, &ids, options)?;

    // Score and rank.
    let mut ranked: Vec<SearchResult> = memories
        .into_iter()
        .filter_map(|m| {
            let cand = candidates.remove(&m.id)?;
            let recency = recency_boost(now, m.created_at);
            let score = combine(&cand, recency, options, m.pinned);
            Some(SearchResult {
                memory: m,
                score,
                vector_score: cand.vector_score,
                fts_score: cand.fts_score,
                recency_score: recency,
            })
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(options.limit);
    Ok(ranked)
}

/// Cosine similarity in `[-1, 1]`, clamped at zero on the low side.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return 0.0;
    }
    let s = dot / (na.sqrt() * nb.sqrt());
    s.clamp(0.0, 1.0)
}

fn recency_boost(now: i64, created_at: i64) -> f32 {
    let age_days = ((now - created_at).max(0) as f32) / 86_400.0;
    let raw = (-age_days / 30.0).exp();
    raw.clamp(0.0, 1.0)
}

fn combine(cand: &Candidate, recency: f32, opts: &SearchOptions, pinned: bool) -> f32 {
    let vec_part = cand.vector_score.unwrap_or(0.0) * opts.vector_weight;
    let fts_part = cand.fts_score.unwrap_or(0.0) * opts.fts_weight;
    let rec_part = recency * opts.recency_weight;
    let mut score = vec_part + fts_part + rec_part;
    if pinned {
        score += PINNED_BONUS;
    }
    score.clamp(0.0, 1.0)
}

fn max_opt_f32(acc: Option<f32>, x: f32) -> Option<f32> {
    Some(match acc {
        None => x,
        Some(prev) => {
            if x.abs() > prev.abs() {
                x
            } else {
                prev
            }
        }
    })
}

#[derive(Default)]
struct Candidate {
    fts_score: Option<f32>,
    vector_score: Option<f32>,
}

struct FtsRow {
    memory_id: String,
    bm25: f32,
}

fn fts_candidates(db: &Database, query: &str, opts: &SearchOptions) -> Result<Vec<FtsRow>> {
    // Translate a free-text query into an FTS5 MATCH expression that
    // tolerates extra whitespace and unrecognized punctuation.
    let match_expr = sanitize_fts_query(query);
    if match_expr.is_empty() {
        return Ok(Vec::new());
    }
    let mut sql = String::from(
        "SELECT m.id, bm25(memory_fts)
           FROM memory_fts
           JOIN memories m ON m.id = memory_fts.memory_id
          WHERE memory_fts MATCH ?1
            AND m.status IN ('approved','pinned')",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(match_expr)];
    push_scope_filter(&mut sql, &mut binds, opts);
    sql.push_str(&format!(" LIMIT {}", opts.limit * 5));

    let mut stmt = db.conn().prepare(&sql)?;
    let bind_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(bind_refs.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (id, bm25) = r?;
        out.push(FtsRow {
            memory_id: id,
            bm25: bm25 as f32,
        });
    }
    Ok(out)
}

fn vector_candidates(
    db: &Database,
    model_id: &str,
    opts: &SearchOptions,
) -> Result<Vec<(String, Vec<f32>)>> {
    let mut sql = String::from(
        "SELECT m.id, e.embedding
           FROM memory_embeddings e
           JOIN memories m ON m.id = e.memory_id
          WHERE e.model = ?1
            AND m.status IN ('approved','pinned')",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(model_id.to_string())];
    push_scope_filter(&mut sql, &mut binds, opts);
    let mut stmt = db.conn().prepare(&sql)?;
    let bind_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(bind_refs.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (id, bytes) = r?;
        let emb = bytes_to_embedding(&bytes)?;
        out.push((id, emb));
    }
    Ok(out)
}

fn load_memories(db: &Database, ids: &[String], opts: &SearchOptions) -> Result<Vec<Memory>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (1..=ids.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "SELECT m.id, m.scope_kind, m.scope_id, m.content, m.kind, m.status, m.pinned, m.confidence,
                m.created_at, m.updated_at, m.expires_at, m.source_turn_id, m.source_client
           FROM memories m
          WHERE m.id IN ({placeholders})
            AND m.status IN ('approved','pinned')",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> =
        ids.iter().map(|s| Box::new(s.clone()) as _).collect();
    push_scope_filter(&mut sql, &mut binds, opts);

    let mut stmt = db.conn().prepare(&sql)?;
    let bind_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(bind_refs.as_slice(), |row| Ok(row_to_memory(row)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

fn push_scope_filter(
    sql: &mut String,
    binds: &mut Vec<Box<dyn rusqlite::ToSql>>,
    opts: &SearchOptions,
) {
    if let Some(scope) = opts.scope {
        sql.push_str(&format!(" AND m.scope_kind = ?{}", binds.len() + 1));
        binds.push(Box::new(scope.as_db_str().to_string()));
        if let Some(id) = &opts.scope_id {
            sql.push_str(&format!(" AND m.scope_id = ?{}", binds.len() + 1));
            binds.push(Box::new(id.clone()));
        }
    }
}

fn row_to_memory(row: &rusqlite::Row<'_>) -> Result<Memory> {
    let id: String = row.get(0)?;
    let scope_str: String = row.get(1)?;
    let scope: Scope = scope_str.parse()?;
    let scope_id: Option<String> = row.get(2)?;
    let content: String = row.get(3)?;
    let kind_str: String = row.get(4)?;
    let kind: crate::memory::Kind = kind_str.parse()?;
    let status_str: String = row.get(5)?;
    let status: Status = status_str.parse()?;
    let pinned: i64 = row.get(6)?;
    let confidence: Option<f64> = row.get(7)?;
    let created_at: i64 = row.get(8)?;
    let updated_at: i64 = row.get(9)?;
    let expires_at: Option<i64> = row.get(10)?;
    let source_turn_id: Option<String> = row.get(11)?;
    let source_client: Option<String> = row.get(12)?;
    Ok(Memory {
        id,
        scope,
        scope_id,
        content,
        kind,
        status,
        pinned: pinned != 0,
        confidence,
        created_at,
        updated_at,
        expires_at,
        source_turn_id,
        source_client,
    })
}

fn bytes_to_embedding(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(Error::Validation(format!(
            "embedding bytes length {} is not a multiple of 4",
            bytes.len()
        )));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().expect("chunks_exact size 4");
        out.push(f32::from_le_bytes(arr));
    }
    Ok(out)
}

/// Translate a user-typed query into a string the FTS5 `MATCH`
/// operator accepts.
///
/// Strips non-word characters, lowercases, and ANDs each token. Returns
/// an empty string when nothing usable remains.
pub fn sanitize_fts_query(input: &str) -> String {
    let tokens: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect();
    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{insert_with_embedder, NewMemory};
    use crate::NoopEmbedder;

    fn fresh_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    /// Deterministic test embedder: encodes the first character as
    /// the only non-zero dimension.
    struct DEmb;
    impl Embedder for DEmb {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0_f32; 8];
                    if let Some(first) = t.chars().next() {
                        let idx = (first.to_lowercase().next().unwrap() as usize) % 8;
                        v[idx] = 1.0;
                    }
                    v
                })
                .collect())
        }
        fn dimension(&self) -> Option<usize> {
            Some(8)
        }
        fn model_id(&self) -> &str {
            "test-d"
        }
    }

    #[test]
    fn cosine_similarity_basic_cases() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]), 1.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert!(
            (cosine_similarity(&[1.0, 1.0], &[1.0, 0.0]) - std::f32::consts::FRAC_1_SQRT_2).abs()
                < 1e-3
        );
    }

    #[test]
    fn empty_query_is_rejected() {
        let db = fresh_db();
        let err = search(&db, &NoopEmbedder, "  ", &SearchOptions::default()).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn search_returns_empty_when_no_memories() {
        let db = fresh_db();
        let r = search(&db, &NoopEmbedder, "anything", &SearchOptions::default()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn search_returns_only_active_memories() {
        let mut db = fresh_db();
        let kept = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "I prefer pnpm"),
            &DEmb,
            100,
        )
        .unwrap();
        let archived = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "I prefer pnpm"),
            &DEmb,
            101,
        )
        .unwrap();
        crate::memory::archive(&db, &archived.id, 200).unwrap();

        let results = search(&db, &DEmb, "pnpm", &SearchOptions::default()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, kept.id);
    }

    #[test]
    fn pinned_memory_outranks_equal_scored_unpinned() {
        let mut db = fresh_db();
        // Two memories with the same content; pinning is the only difference.
        insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "always run tests"),
            &DEmb,
            100,
        )
        .unwrap();
        let mut pinned = NewMemory::new(Scope::Global, "always run tests");
        pinned.pinned = true;
        let pin = insert_with_embedder(&mut db, pinned, &DEmb, 100).unwrap();

        let results = search(&db, &DEmb, "tests", &SearchOptions::default()).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].memory.id, pin.id);
        assert!(results[0].memory.pinned);
    }

    #[test]
    fn scope_filter_excludes_other_scopes() {
        let mut db = fresh_db();
        insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "global note"),
            &DEmb,
            100,
        )
        .unwrap();
        let mut pscope = NewMemory::new(Scope::Project, "project note");
        pscope.scope_id = Some("proj_xyz".into());
        insert_with_embedder(&mut db, pscope, &DEmb, 101).unwrap();

        let opts = SearchOptions {
            scope: Some(Scope::Project),
            ..Default::default()
        };
        let results = search(&db, &DEmb, "note", &opts).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.memory.scope == Scope::Project));
    }

    #[test]
    fn limit_caps_results() {
        let mut db = fresh_db();
        for i in 0..5 {
            insert_with_embedder(
                &mut db,
                NewMemory::new(Scope::Global, format!("note number {i}")),
                &DEmb,
                100 + i,
            )
            .unwrap();
        }
        let opts = SearchOptions {
            limit: 2,
            ..Default::default()
        };
        let results = search(&db, &DEmb, "note", &opts).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn sanitize_fts_query_strips_punctuation() {
        assert_eq!(sanitize_fts_query("hello, world!"), "hello world");
        assert_eq!(sanitize_fts_query("  pnpm  vs   npm  "), "pnpm vs npm");
        assert_eq!(sanitize_fts_query("---"), "");
    }
}
