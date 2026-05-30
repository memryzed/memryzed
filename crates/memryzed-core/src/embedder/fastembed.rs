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

//! `fastembed-rs` integration.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::embedder::Embedder;
use crate::error::{Error, Result};

/// Commit a global ONNX Runtime environment whose thread pool is
/// capped, so embedding never saturates the machine. Runs at most
/// once per process; subsequent calls are no-ops.
///
/// The intra-op thread count is capped at min(2, cores). The global
/// environment must be committed before any session is created for
/// the cap to apply, which is why this is called at the very start of
/// embedder load. Best-effort: failures are logged and ignored.
fn init_capped_thread_pool() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let cap = std::thread::available_parallelism()
            .map(|n| n.get().min(2))
            .unwrap_or(1);
        let result = (|| -> ort::Result<bool> {
            let pool = ort::environment::GlobalThreadPoolOptions::default()
                .with_intra_threads(cap)?
                .with_inter_threads(1)?;
            Ok(ort::init()
                .with_name("memryzed")
                .with_global_thread_pool(pool)
                .commit())
        })();
        match result {
            Ok(true) => tracing::debug!(
                target: "memryzed::embedder",
                intra_threads = cap,
                "committed capped global ONNX thread pool",
            ),
            Ok(false) => tracing::debug!(
                target: "memryzed::embedder",
                "global ONNX environment already committed; cap not applied",
            ),
            Err(e) => tracing::warn!(
                target: "memryzed::embedder",
                error = %e,
                "could not cap ONNX thread pool; embedding may use more CPU",
            ),
        }
    });
}

/// Stable model identifier stored alongside every embedding.
///
/// Matches `[retrieval] embedding_model` in the default config.
/// Persisted to the database so we can detect model changes and
/// trigger re-embedding.
pub const FASTEMBED_MODEL_ID: &str = "bge-small-en-v1.5";

/// Output dimension of the BGE-small-en-v1.5 model.
pub const FASTEMBED_DIMENSION: usize = 384;

/// Embedder backed by fastembed's `TextEmbedding` and
/// BGE-small-en-v1.5.
///
/// Cheap to clone via `Arc`. Loading the model from cache takes a
/// few hundred milliseconds; downloading it on the first cold start
/// can take much longer depending on the network.
#[derive(Clone)]
pub struct FastembedEmbedder {
    inner: Arc<Mutex<TextEmbedding>>,
    cache_dir: PathBuf,
}

impl FastembedEmbedder {
    /// Load (and download if necessary) the embedding model into
    /// `cache_dir`.
    ///
    /// Creates the directory if it does not exist. Stamps the load
    /// in the tracing log.
    pub fn load(cache_dir: &Path) -> Result<Self> {
        if !cache_dir.exists() {
            std::fs::create_dir_all(cache_dir)?;
        }
        let cache_dir = cache_dir.to_path_buf();

        // Cap how many cores ONNX Runtime uses for inference, so
        // embedding stays a gentle background task instead of
        // saturating every core. This is done by committing a global
        // ONNX thread pool with a small intra-op thread count before
        // any session is created; fastembed's session then uses it.
        // Idempotent and best-effort: if a global environment was
        // already committed (or commit fails), we proceed with the
        // default and just accept higher CPU rather than failing.
        init_capped_thread_pool();

        let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_cache_dir(cache_dir.clone())
            .with_show_download_progress(true);

        let model = TextEmbedding::try_new(options)
            .map_err(|e| Error::Validation(format!("failed to load embedding model: {e}")))?;

        tracing::debug!(
            target: "memryzed::embedder",
            model = FASTEMBED_MODEL_ID,
            dimension = FASTEMBED_DIMENSION,
            cache = %cache_dir.display(),
            "fastembed embedder ready",
        );

        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
            cache_dir,
        })
    }

    /// The cache directory the embedder is using.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

impl Embedder for FastembedEmbedder {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let owned: Vec<String> = texts.iter().map(|s| (*s).to_string()).collect();
        let mut model = self
            .inner
            .lock()
            .map_err(|_| Error::Validation("embedder mutex poisoned".into()))?;
        let embeddings = model
            .embed(owned, None)
            .map_err(|e| Error::Validation(format!("embedding failed: {e}")))?;
        Ok(embeddings)
    }

    fn dimension(&self) -> Option<usize> {
        Some(FASTEMBED_DIMENSION)
    }

    fn model_id(&self) -> &str {
        FASTEMBED_MODEL_ID
    }
}

#[cfg(test)]
mod tests {
    // Real-model tests are skipped in CI because they require network
    // access and ~130 MB of disk. Run them locally with:
    //
    //     cargo test -p memryzed-core fastembed_real -- --ignored
    //
    use super::*;

    #[test]
    #[ignore = "downloads the BGE-small model on first run"]
    fn fastembed_real_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let e = FastembedEmbedder::load(dir.path()).unwrap();
        assert_eq!(e.dimension(), Some(FASTEMBED_DIMENSION));
        assert_eq!(e.model_id(), FASTEMBED_MODEL_ID);
        let v = e.embed(&["I prefer pnpm", "uses Vitest"]).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].len(), FASTEMBED_DIMENSION);
        assert_eq!(v[1].len(), FASTEMBED_DIMENSION);
    }
}
