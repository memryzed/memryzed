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

//! Project identity computation.
//!
//! A project's stable identifier is derived from its git remote URL
//! when available, otherwise from its absolute path. Both produce
//! deterministic, opaque IDs so the same repo yields the same ID
//! across machines and the same local-only directory always yields
//! the same local ID.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::Result;
use crate::id::{project_id_from_local_path, project_id_from_remote};

/// Resolved identity of a working directory.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProjectIdentity {
    /// The computed project ID (`proj_*` or `proj_local_*`).
    pub id: String,
    /// The normalized git remote URL when one was found.
    pub git_remote: Option<String>,
    /// The absolute path of the working directory used.
    pub absolute_path: PathBuf,
    /// The display name (last path component of `absolute_path`).
    pub display_name: String,
}

/// Compute the identity of the project at `cwd`.
///
/// `cwd` should already be an absolute path; callers are expected
/// to canonicalize before invoking.
pub fn compute(cwd: &Path) -> Result<ProjectIdentity> {
    let absolute_path = cwd.to_path_buf();
    let display_name = absolute_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| absolute_path.display().to_string());

    let git_remote = git_remote_url(cwd);

    let id = match &git_remote {
        Some(remote) => project_id_from_remote(remote),
        None => project_id_from_local_path(&absolute_path.to_string_lossy()),
    };

    Ok(ProjectIdentity {
        id,
        git_remote,
        absolute_path,
        display_name,
    })
}

fn git_remote_url(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["config", "--get", "remote.origin.url"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(normalize_remote(trimmed))
}

/// Normalize a git remote URL for hashing.
///
/// The goal is that equivalent remotes produce equal strings:
/// SSH and HTTPS URLs for the same repo, with or without trailing
/// `.git`, with or without an embedded credential, lowercased host.
pub fn normalize_remote(input: &str) -> String {
    let mut s = input.trim().to_string();

    // Strip an embedded credential like `https://user:tok@host/...`.
    if let Some(scheme_end) = s.find("://") {
        let after_scheme = scheme_end + 3;
        if let Some(at) = s[after_scheme..].find('@') {
            let cred_end = after_scheme + at + 1;
            s.replace_range(after_scheme..cred_end, "");
        }
    }

    // Strip trailing `.git`.
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }

    // Lowercase the host portion of `scheme://host/...` and `user@host:path`.
    if let Some(scheme_end) = s.find("://") {
        let after = scheme_end + 3;
        let rest_start = after;
        let rest_end = s[rest_start..]
            .find('/')
            .map(|p| rest_start + p)
            .unwrap_or(s.len());
        let host = s[rest_start..rest_end].to_ascii_lowercase();
        s.replace_range(rest_start..rest_end, &host);
    } else if let Some(at) = s.find('@') {
        if let Some(colon) = s[at + 1..].find(':') {
            let host_start = at + 1;
            let host_end = at + 1 + colon;
            let host = s[host_start..host_end].to_ascii_lowercase();
            s.replace_range(host_start..host_end, &host);
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_trailing_dot_git() {
        assert_eq!(
            normalize_remote("git@github.com:memryzed/memryzed.git"),
            "git@github.com:memryzed/memryzed"
        );
        assert_eq!(
            normalize_remote("https://github.com/memryzed/memryzed.git"),
            "https://github.com/memryzed/memryzed"
        );
    }

    #[test]
    fn normalize_strips_embedded_credentials() {
        assert_eq!(
            normalize_remote("https://user:tok@github.com/memryzed/memryzed.git"),
            "https://github.com/memryzed/memryzed"
        );
    }

    #[test]
    fn normalize_lowercases_host_only() {
        assert_eq!(
            normalize_remote("https://GitHub.COM/Memryzed/Memryzed.git"),
            "https://github.com/Memryzed/Memryzed"
        );
        assert_eq!(
            normalize_remote("git@GitHub.com:Memryzed/Memryzed.git"),
            "git@github.com:Memryzed/Memryzed"
        );
    }

    #[test]
    fn ssh_and_https_have_distinct_normalized_forms() {
        // We don't claim full equivalence between SSH and HTTPS in
        // v0.1.0; that requires parsing the path. They normalize to
        // distinct strings for now.
        let ssh = normalize_remote("git@github.com:memryzed/memryzed.git");
        let https = normalize_remote("https://github.com/memryzed/memryzed.git");
        assert_ne!(ssh, https);
    }

    #[test]
    fn compute_in_a_non_git_dir_uses_local_path() {
        let tmp = tempfile::tempdir().unwrap();
        let id = compute(tmp.path()).unwrap();
        assert!(id.git_remote.is_none(), "no remote in a fresh tempdir");
        assert!(
            id.id.starts_with("proj_local_"),
            "expected proj_local_ prefix, got {}",
            id.id
        );
        assert_eq!(id.absolute_path, tmp.path());
    }

    #[test]
    fn compute_is_deterministic_for_the_same_path() {
        let tmp = tempfile::tempdir().unwrap();
        let a = compute(tmp.path()).unwrap();
        let b = compute(tmp.path()).unwrap();
        assert_eq!(a.id, b.id);
    }
}
