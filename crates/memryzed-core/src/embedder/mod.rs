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

//! Embedding generation.
//!
//! v0.1.0-alpha.3 ships [`FastembedEmbedder`] backed by `fastembed-rs`
//! with the BGE-small-en-v1.5 model (384-dimensional output). Tests
//! and CI use [`NoopEmbedder`] via the `MEMRYZED_DISABLE_EMBEDDING`
//! environment variable so no network or model download is required.
//!
//! The model identifier reported by [`Embedder::model_id`] becomes
//! the `embedding_model` column on every memory and is used to
//! detect model changes that would require re-embedding.

mod fastembed;
mod noop;

use std::path::Path;
use std::sync::Arc;

pub use self::fastembed::{FastembedEmbedder, FASTEMBED_MODEL_ID};
pub use self::noop::NoopEmbedder;

use crate::error::Result;

/// Environment variable that, when set to any value, disables real
/// embedding. The factory returns a [`NoopEmbedder`] instead.
pub const ENV_DISABLE: &str = "MEMRYZED_DISABLE_EMBEDDING";

/// Common interface for everything that produces embeddings.
///
/// Implementations are expected to be cheap to clone (typically
/// holding the model behind an `Arc`) so they can be passed across
/// helper functions without lifetime acrobatics.
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts. Returns one vector per input, in the
    /// same order. Implementations that do not produce embeddings
    /// (such as [`NoopEmbedder`]) return an empty `Vec` for each
    /// entry.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// The dimension of every produced embedding. `None` for the
    /// no-op embedder.
    fn dimension(&self) -> Option<usize>;

    /// Stable model identifier used to track which embeddings need
    /// regeneration when the model changes.
    fn model_id(&self) -> &str;

    /// `true` for embedders that actually produce vectors.
    fn is_active(&self) -> bool {
        self.dimension().is_some()
    }
}

/// Construct the default embedder.
///
/// Returns a [`NoopEmbedder`] if `MEMRYZED_DISABLE_EMBEDDING` is
/// set, otherwise loads (and downloads if needed) the
/// [`FastembedEmbedder`] using `models_dir` as the cache.
pub fn make_default(models_dir: &Path) -> Result<Arc<dyn Embedder>> {
    if std::env::var(ENV_DISABLE).is_ok() {
        tracing::debug!(
            target: "memryzed::embedder",
            "MEMRYZED_DISABLE_EMBEDDING set; using NoopEmbedder",
        );
        return Ok(Arc::new(NoopEmbedder));
    }
    let embedder = FastembedEmbedder::load(models_dir)?;
    Ok(Arc::new(embedder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_default_returns_noop_when_env_is_set() {
        // SAFETY: tests run single-threaded for env mutation.
        std::env::set_var(ENV_DISABLE, "1");
        let dir = tempfile::tempdir().unwrap();
        let e = make_default(dir.path()).unwrap();
        assert!(!e.is_active(), "expected noop when env is set");
        std::env::remove_var(ENV_DISABLE);
    }
}
