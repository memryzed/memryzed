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

//! No-op embedder.
//!
//! Returns empty vectors so storage code can skip writing
//! embeddings. Used in tests and when the
//! `MEMRYZED_DISABLE_EMBEDDING` env var is set.

use crate::embedder::Embedder;
use crate::error::Result;

/// An embedder that produces no embeddings.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEmbedder;

impl Embedder for NoopEmbedder {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(vec![Vec::new(); texts.len()])
    }

    fn dimension(&self) -> Option<usize> {
        None
    }

    fn model_id(&self) -> &str {
        "noop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_returns_one_empty_vec_per_input() {
        let e = NoopEmbedder;
        let v = e.embed(&["a", "b", "c"]).unwrap();
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|emb| emb.is_empty()));
    }

    #[test]
    fn noop_is_inactive() {
        assert!(!NoopEmbedder.is_active());
        assert_eq!(NoopEmbedder.dimension(), None);
        assert_eq!(NoopEmbedder.model_id(), "noop");
    }
}
