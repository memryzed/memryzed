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

//! Core library for Memryzed.
//!
//! This crate is the foundation: it owns the data directory layout,
//! storage, projects, memory, and (in later alphas) retrieval and
//! sessions.
//!
//! See `docs/architecture.md` for the architecture and
//! `docs/data-model.md` for the schema.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod clock;
pub mod embedder;
pub mod error;
pub mod export;
pub mod id;
pub mod integrations;
pub mod memory;
pub mod paths;
pub mod projects;
pub mod retrieval;
pub mod sessions;
pub mod storage;
pub mod version;

pub use clock::now_epoch_seconds;
pub use embedder::{Embedder, NoopEmbedder};
pub use error::{Error, Result};
pub use paths::DataDir;
pub use retrieval::{search, SearchOptions, SearchResult};
pub use storage::Database;
pub use version::VERSION;
