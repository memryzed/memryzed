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

//! Crate-wide error type for memryzed-core.
//!
//! The CLI maps these to exit codes; the MCP server maps them to
//! protocol error codes. See `docs/mcp-reference.md` and
//! `docs/cli-reference.md` for the full mapping.

use std::io;

use thiserror::Error;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors raised by `memryzed-core`.
#[derive(Debug, Error)]
pub enum Error {
    /// SQLite-level failure.
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),

    /// Filesystem or other I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Migration runner could not bring the schema up to date.
    #[error("migration error: {0}")]
    Migration(String),

    /// A lookup found no matching record.
    #[error("not found: {kind} {id}")]
    NotFound {
        /// What kind of record was being looked up (memory, project, session).
        kind: &'static str,
        /// The supplied identifier.
        id: String,
    },

    /// Caller supplied invalid input.
    #[error("validation error: {0}")]
    Validation(String),

    /// Invalid configuration.
    #[error("configuration error: {0}")]
    Config(String),
}

impl Error {
    /// Construct a `NotFound` for a memory.
    pub fn memory_not_found(id: impl Into<String>) -> Self {
        Error::NotFound {
            kind: "memory",
            id: id.into(),
        }
    }

    /// Construct a `NotFound` for a project.
    pub fn project_not_found(id: impl Into<String>) -> Self {
        Error::NotFound {
            kind: "project",
            id: id.into(),
        }
    }
}
