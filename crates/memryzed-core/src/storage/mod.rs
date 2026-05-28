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

//! SQLite storage layer.
//!
//! All database access flows through [`Database`]. WAL journaling,
//! the busy timeout, and the migration runner are configured here.
//!
//! Higher-level modules (`memory`, `projects`) take a `&Database`
//! and operate on it. They never open connections themselves.

mod migrations;

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{Error, Result};

pub use migrations::current_schema_version;

/// A handle to the Memryzed SQLite database.
///
/// Currently a thin wrapper around a single connection. Concurrency
/// for the MCP server is added when the server lands in alpha.5.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the database at `path`, run pending migrations,
    /// and configure pragmas (WAL, busy timeout).
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        let mut db = Self { conn };
        migrations::run(&mut db.conn)?;
        Ok(db)
    }

    /// Open an in-memory database. Useful for tests and ephemeral runs.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        let mut db = Self { conn };
        migrations::run(&mut db.conn)?;
        Ok(db)
    }

    fn configure(conn: &Connection) -> Result<()> {
        conn.busy_timeout(Duration::from_millis(5000))?;
        // WAL is meaningless on :memory: but harmless.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(())
    }

    /// Borrow the underlying connection. Library modules use this to
    /// run their own statements; callers outside the crate should not
    /// rely on it.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Mutable borrow, used for transactions.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Run an integrity check (`PRAGMA integrity_check`).
    ///
    /// Returns `Ok(())` when SQLite reports `ok` and an [`Error`]
    /// otherwise.
    pub fn integrity_check(&self) -> Result<()> {
        let mut stmt = self.conn.prepare("PRAGMA integrity_check")?;
        let result: String = stmt.query_row([], |row| row.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(Error::Migration(format!(
                "integrity_check returned {result:?}"
            )))
        }
    }

    /// Current schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i32> {
        let v: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_and_run_migrations() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), current_schema_version());
        db.integrity_check().unwrap();
    }

    #[test]
    fn open_creates_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("db.sqlite");
        let db = Database::open(&nested).unwrap();
        assert!(nested.exists());
        db.integrity_check().unwrap();
    }

    #[test]
    fn schema_has_expected_tables() {
        let db = Database::open_in_memory().unwrap();
        let mut stmt = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .filter(|n: &String| !n.starts_with("sqlite_"))
            .collect();
        for required in [
            "memories",
            "memory_embeddings",
            "projects",
            "recall_log",
            "meta",
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "table {required:?} missing; got {names:?}"
            );
        }
    }

    #[test]
    fn migrations_are_idempotent_across_opens() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("db.sqlite");

        // Open twice; second open should be a no-op for migrations.
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), current_schema_version());
        }
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), current_schema_version());
            db.integrity_check().unwrap();
        }
    }
}
