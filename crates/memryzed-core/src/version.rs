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

//! Version information for Memryzed.

/// The version of the Memryzed binary, taken from the workspace `Cargo.toml`.
///
/// This is the single source of truth for the version string. The CLI's
/// `--version` flag, the MCP server's handshake, and any audit-log entry
/// that needs a version pull from here.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty(), "VERSION must be set by Cargo");
    }

    #[test]
    fn version_starts_with_a_digit() {
        let first = VERSION.chars().next().expect("VERSION must not be empty");
        assert!(
            first.is_ascii_digit(),
            "VERSION must start with a digit, got {VERSION:?}"
        );
    }
}
