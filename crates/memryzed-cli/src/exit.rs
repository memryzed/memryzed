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

//! CLI exit codes.
//!
//! These match the table in `docs/cli-reference.md`.

#![allow(dead_code)]

use std::fmt;

/// Successful exit.
pub const SUCCESS: i32 = 0;
/// General error.
pub const GENERAL_ERROR: i32 = 1;
/// Misuse: bad arguments or unknown command.
pub const MISUSE: i32 = 2;
/// Configuration error.
pub const CONFIG_ERROR: i32 = 3;
/// Storage error: database or filesystem problem.
pub const STORAGE_ERROR: i32 = 4;
/// Network error: download or update check failed.
pub const NETWORK_ERROR: i32 = 5;
/// Integration error: an MCP client config could not be read or written.
pub const INTEGRATION_ERROR: i32 = 6;

/// Wraps an error with a specific exit code.
///
/// The CLI inspects errors for [`Coded`] to decide which code to
/// return from `main`. Errors without a code use [`GENERAL_ERROR`].
#[derive(Debug)]
pub struct Coded {
    code: i32,
    message: String,
}

impl Coded {
    /// Construct a coded error.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The exit code to report.
    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Coded {}
