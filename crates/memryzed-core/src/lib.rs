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
//! configuration loading, and (in later alpha releases) the storage,
//! retrieval, sessions, and extractor modules.
//!
//! v0.1.0-alpha.1 ships only the data-directory and version primitives.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod paths;
pub mod version;

pub use paths::DataDir;
pub use version::VERSION;
