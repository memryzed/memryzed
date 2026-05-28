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

//! MCP server layer for Memryzed.
//!
//! This crate implements the eight MCP tools defined in
//! `docs/specs/v1.md`. v0.1.0-alpha.1 ships with no tools wired yet;
//! the crate exists so the workspace boundaries are correct from the
//! start.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Reserved entry point for the MCP server.
///
/// Returns an error in v0.1.0-alpha.1 because the server is not yet
/// implemented. The CLI's `serve` subcommand calls this and surfaces
/// the error to the user with a clear message about the alpha status.
pub fn serve_stdio() -> Result<(), &'static str> {
    Err("memryzed serve is not implemented in v0.1.0-alpha.1")
}
