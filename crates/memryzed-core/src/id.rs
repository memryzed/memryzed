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

//! Stable, opaque identifier generation.
//!
//! All identifiers are 12 hex characters of randomness with a kind
//! prefix. Memory IDs look like `mem_a1b2c3d4e5f6`. Project IDs are
//! either `proj_<12hex>` (when computed from a git remote, deterministic)
//! or `proj_local_<12hex>` (when computed from an absolute path,
//! deterministic). Session IDs are `sess_<12hex>` (random).

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Length of the hex portion of every ID.
pub const ID_HEX_LEN: usize = 12;

/// Prefix for memory IDs.
pub const MEM_PREFIX: &str = "mem_";
/// Prefix for project IDs derived from a git remote URL.
pub const PROJ_PREFIX: &str = "proj_";
/// Prefix for project IDs derived from a local absolute path.
pub const PROJ_LOCAL_PREFIX: &str = "proj_local_";
/// Prefix for session IDs.
pub const SESS_PREFIX: &str = "sess_";

/// Generate a fresh memory ID using the OS RNG.
pub fn new_memory_id() -> String {
    format!("{MEM_PREFIX}{}", random_hex())
}

/// Generate a fresh session ID using the OS RNG.
pub fn new_session_id() -> String {
    format!("{SESS_PREFIX}{}", random_hex())
}

/// Compute a deterministic project ID from a normalized git remote URL.
pub fn project_id_from_remote(remote: &str) -> String {
    format!("{PROJ_PREFIX}{}", hash_hex(remote))
}

/// Compute a deterministic local-only project ID from an absolute path.
pub fn project_id_from_local_path(absolute_path: &str) -> String {
    format!("{PROJ_LOCAL_PREFIX}{}", hash_hex(absolute_path))
}

fn random_hex() -> String {
    let mut bytes = [0u8; ID_HEX_LEN / 2];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let digest = h.finalize();
    let truncated = &digest[..ID_HEX_LEN / 2];
    hex::encode(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_id_format() {
        let id = new_memory_id();
        assert!(id.starts_with(MEM_PREFIX));
        assert_eq!(id.len(), MEM_PREFIX.len() + ID_HEX_LEN);
        let hex = &id[MEM_PREFIX.len()..];
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn session_id_format() {
        let id = new_session_id();
        assert!(id.starts_with(SESS_PREFIX));
        assert_eq!(id.len(), SESS_PREFIX.len() + ID_HEX_LEN);
    }

    #[test]
    fn project_id_from_remote_is_deterministic() {
        let a = project_id_from_remote("git@github.com:memryzed/memryzed.git");
        let b = project_id_from_remote("git@github.com:memryzed/memryzed.git");
        assert_eq!(a, b);
        assert!(a.starts_with(PROJ_PREFIX));
        assert_eq!(a.len(), PROJ_PREFIX.len() + ID_HEX_LEN);
    }

    #[test]
    fn project_id_from_local_path_is_deterministic() {
        let a = project_id_from_local_path("/home/me/projects/repo");
        let b = project_id_from_local_path("/home/me/projects/repo");
        assert_eq!(a, b);
        assert!(a.starts_with(PROJ_LOCAL_PREFIX));
    }

    #[test]
    fn distinct_inputs_produce_distinct_ids() {
        let a = project_id_from_remote("git@github.com:a/b.git");
        let b = project_id_from_remote("git@github.com:a/c.git");
        assert_ne!(a, b);
    }

    #[test]
    fn random_ids_do_not_collide_in_a_hundred_iterations() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..100 {
            ids.insert(new_memory_id());
        }
        assert_eq!(ids.len(), 100);
    }
}
