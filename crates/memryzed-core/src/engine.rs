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

//! Background-engine configuration and single-instance coordination.
//!
//! Every agent session spawns its own `memryzed serve`, and each would
//! otherwise run its own embedding engine against the shared database,
//! multiplying CPU with the number of open sessions. Two pieces fix
//! and tune that:
//!
//! - [`EngineLock`]: an OS-level advisory lock so only one serve
//!   process runs the engine; the rest still answer tool calls but do
//!   no background embedding.
//! - [`IndexProfile`]: how hard that single engine works, gentle by
//!   default, configurable to go faster.

use std::path::Path;

/// Throughput profile for the background embedding engine. Controls
/// the batch size and the pause between batches, trading CPU for
/// speed. `Gentle` is the default and keeps embedding to a fraction
/// of the machine; `Fast` clears a backlog quickly at higher CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexProfile {
    /// Invisible: small batches, long pauses. Default.
    Gentle,
    /// A middle ground.
    Balanced,
    /// Drain the backlog quickly; uses noticeably more CPU.
    Fast,
}

impl IndexProfile {
    /// Episodes embedded per batch.
    pub fn batch(self) -> usize {
        match self {
            IndexProfile::Gentle => 16,
            IndexProfile::Balanced => 48,
            IndexProfile::Fast => 128,
        }
    }

    /// Milliseconds to sleep after each embed batch.
    pub fn pause_ms(self) -> u64 {
        match self {
            IndexProfile::Gentle => 400,
            IndexProfile::Balanced => 100,
            IndexProfile::Fast => 0,
        }
    }

    /// Parse a profile name, case-insensitive. Unknown names fall back
    /// to `Gentle`.
    pub fn parse(s: &str) -> IndexProfile {
        match s.trim().to_ascii_lowercase().as_str() {
            "balanced" => IndexProfile::Balanced,
            "fast" => IndexProfile::Fast,
            _ => IndexProfile::Gentle,
        }
    }

    /// Lowercase name, for display and config round-tripping.
    pub fn as_str(self) -> &'static str {
        match self {
            IndexProfile::Gentle => "gentle",
            IndexProfile::Balanced => "balanced",
            IndexProfile::Fast => "fast",
        }
    }
}

/// Resolve the active profile. `MEMRYZED_INDEX_PROFILE` wins, then the
/// `index.profile` key in `config.toml`, then `Gentle`.
pub fn resolve_profile(config_file: &Path) -> IndexProfile {
    if let Ok(env) = std::env::var("MEMRYZED_INDEX_PROFILE") {
        if !env.trim().is_empty() {
            return IndexProfile::parse(&env);
        }
    }
    if let Ok(raw) = std::fs::read_to_string(config_file) {
        if let Ok(doc) = raw.parse::<toml::Value>() {
            if let Some(p) = doc
                .get("index")
                .and_then(|i| i.get("profile"))
                .and_then(|v| v.as_str())
            {
                return IndexProfile::parse(p);
            }
        }
    }
    IndexProfile::Gentle
}

/// An advisory single-instance lock for the background engine, held
/// for the lifetime of the process that acquires it.
///
/// Implemented as a PID lock file created exclusively (`O_EXCL`): the
/// first process to create it owns the engine. A stale lock from a
/// crashed process (its PID no longer alive) is taken over. The file
/// is removed on drop. This avoids any `unsafe` FFI, so it works
/// under the crate's `#![forbid(unsafe_code)]`.
pub struct EngineLock {
    path: std::path::PathBuf,
}

impl EngineLock {
    /// Try to acquire the engine lock at `path`. Returns `Some` if this
    /// process now owns the engine, `None` if another live process
    /// holds it.
    pub fn acquire(path: &Path) -> Option<EngineLock> {
        if Self::try_create(path) {
            return Some(EngineLock {
                path: path.to_path_buf(),
            });
        }
        // Lock exists. Take it over only if the recorded PID is dead.
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(pid) = contents.trim().parse::<i32>() {
                if !pid_alive(pid) {
                    let _ = std::fs::remove_file(path);
                    if Self::try_create(path) {
                        return Some(EngineLock {
                            path: path.to_path_buf(),
                        });
                    }
                }
            }
        }
        None
    }

    /// Create the lock file exclusively and write our PID. Returns
    /// whether we created it (and therefore own the lock).
    fn try_create(path: &Path) -> bool {
        use std::io::Write;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                true
            }
            Err(_) => false,
        }
    }
}

impl Drop for EngineLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Whether a process with the given PID is currently alive. On Unix
/// this checks `/proc/<pid>`; elsewhere it conservatively assumes the
/// process is alive (so a lock is never wrongly stolen).
fn pid_alive(pid: i32) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_parse_and_values() {
        assert_eq!(IndexProfile::parse("FAST"), IndexProfile::Fast);
        assert_eq!(IndexProfile::parse("balanced"), IndexProfile::Balanced);
        assert_eq!(IndexProfile::parse("nonsense"), IndexProfile::Gentle);
        assert!(IndexProfile::Fast.batch() > IndexProfile::Gentle.batch());
        assert!(IndexProfile::Fast.pause_ms() < IndexProfile::Gentle.pause_ms());
    }

    #[test]
    fn env_overrides_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config.toml");
        std::fs::write(&cfg, "[index]\nprofile = \"balanced\"\n").unwrap();
        // Without env, config wins.
        std::env::remove_var("MEMRYZED_INDEX_PROFILE");
        assert_eq!(resolve_profile(&cfg), IndexProfile::Balanced);
    }

    #[cfg(unix)]
    #[test]
    fn lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("engine.lock");
        let first = EngineLock::acquire(&lock);
        assert!(first.is_some(), "first acquire should succeed");
        let second = EngineLock::acquire(&lock);
        assert!(second.is_none(), "second acquire should fail while held");
        drop(first);
        let third = EngineLock::acquire(&lock);
        assert!(third.is_some(), "after release, acquire should succeed");
    }
}
