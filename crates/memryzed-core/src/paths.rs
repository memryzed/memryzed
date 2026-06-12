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

//! Data directory layout for Memryzed.
//!
//! The data directory holds the database, the embedding model files,
//! the configuration, the audit log, and the binary itself when
//! installed via the install script.
//!
//! The default location is `~/.memryzed/` on Unix and
//! `%LOCALAPPDATA%\memryzed\` on Windows. Callers can override the
//! root via the `MEMRYZED_DATA_DIR` environment variable or the
//! `--data-dir` CLI flag.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

/// Environment variable that overrides the default data directory.
pub const ENV_DATA_DIR: &str = "MEMRYZED_DATA_DIR";

/// The root directory name appended under the user's home when no
/// override is supplied. Unix only; on Windows the data directory
/// lives under `%LOCALAPPDATA%\memryzed`.
#[cfg(unix)]
const DEFAULT_DIR_NAME: &str = ".memryzed";

/// Wrapper around the resolved data-directory root with helpers for
/// the well-known paths inside it.
///
/// All paths returned by these helpers are computed by joining the
/// root; they are not guaranteed to exist on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Construct a [`DataDir`] from an explicit root path.
    ///
    /// Used by tests and by the `--data-dir` flag.
    pub fn at<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }

    /// Resolve the data directory using, in order: the
    /// `MEMRYZED_DATA_DIR` environment variable, otherwise the
    /// platform default.
    ///
    /// On Unix the platform default is `$HOME/.memryzed`. On Windows
    /// it is `%LOCALAPPDATA%\memryzed\`.
    ///
    /// Returns an error only when neither the override nor a home
    /// directory can be determined.
    pub fn resolve() -> io::Result<Self> {
        if let Some(override_path) = env::var_os(ENV_DATA_DIR) {
            return Ok(Self::at(PathBuf::from(override_path)));
        }
        Ok(Self::at(default_data_dir()?))
    }

    /// The data-directory root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to the SQLite database file.
    pub fn db_file(&self) -> PathBuf {
        self.root.join("db.sqlite")
    }

    /// Path to the configuration file.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Path to the audit log file.
    pub fn audit_log(&self) -> PathBuf {
        self.root.join("audit.log")
    }

    /// Path to the background-engine single-instance lock file.
    pub fn engine_lock(&self) -> PathBuf {
        self.root.join("engine.lock")
    }

    /// Directory holding embedding model files.
    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Directory the install script places the binary in.
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// `true` if the data directory exists on disk.
    pub fn exists(&self) -> bool {
        self.root.is_dir()
    }
}

/// Resolve the user's home directory.
///
/// Returns an error if the home directory cannot be determined.
pub fn home_dir() -> io::Result<std::path::PathBuf> {
    if let Some(dirs) = directories::BaseDirs::new() {
        return Ok(dirs.home_dir().to_path_buf());
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not determine the user's home directory",
    ))
}

#[cfg(unix)]
fn default_data_dir() -> io::Result<PathBuf> {
    if let Some(home) = directories::BaseDirs::new() {
        return Ok(home.home_dir().join(DEFAULT_DIR_NAME));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not determine the user's home directory",
    ))
}

#[cfg(windows)]
fn default_data_dir() -> io::Result<PathBuf> {
    if let Some(dirs) = directories::BaseDirs::new() {
        return Ok(dirs.data_local_dir().join("memryzed"));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not determine LOCALAPPDATA",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn at_uses_the_supplied_root() {
        let d = DataDir::at("/tmp/memryzed-test");
        assert_eq!(d.root(), Path::new("/tmp/memryzed-test"));
    }

    #[test]
    fn well_known_paths_are_under_root() {
        let d = DataDir::at("/tmp/memryzed-test");
        assert!(d.db_file().starts_with("/tmp/memryzed-test"));
        assert!(d.config_file().starts_with("/tmp/memryzed-test"));
        assert!(d.audit_log().starts_with("/tmp/memryzed-test"));
        assert!(d.models_dir().starts_with("/tmp/memryzed-test"));
        assert!(d.bin_dir().starts_with("/tmp/memryzed-test"));
    }

    #[test]
    fn db_file_uses_the_documented_filename() {
        let d = DataDir::at("/tmp/memryzed-test");
        assert_eq!(d.db_file().file_name().unwrap(), "db.sqlite");
        assert_eq!(d.config_file().file_name().unwrap(), "config.toml");
        assert_eq!(d.audit_log().file_name().unwrap(), "audit.log");
    }

    #[test]
    fn exists_is_false_for_a_nonexistent_root() {
        let d = DataDir::at("/tmp/memryzed-does-not-exist-xyz");
        assert!(!d.exists());
    }
}
