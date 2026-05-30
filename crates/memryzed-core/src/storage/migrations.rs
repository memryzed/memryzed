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

//! Schema migration runner.
//!
//! Migrations are SQL files embedded into the binary and applied in
//! order. The current schema version is tracked via SQLite's
//! `PRAGMA user_version`. Each migration runs in a single transaction
//! and bumps the version on success.

use rusqlite::Connection;

use crate::error::{Error, Result};

/// One migration step.
struct Migration {
    /// Target version this migration upgrades to. Must be the index
    /// in [`MIGRATIONS`] plus one.
    target_version: i32,
    /// Display name for logs and error messages.
    name: &'static str,
    /// The SQL statements that compose this migration.
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        target_version: 1,
        name: "001_initial",
        sql: include_str!("../../migrations/001_initial.sql"),
    },
    Migration {
        target_version: 2,
        name: "002_embeddings",
        sql: include_str!("../../migrations/002_embeddings.sql"),
    },
    Migration {
        target_version: 3,
        name: "003_fts",
        sql: include_str!("../../migrations/003_fts.sql"),
    },
    Migration {
        target_version: 4,
        name: "004_sessions",
        sql: include_str!("../../migrations/004_sessions.sql"),
    },
    Migration {
        target_version: 5,
        name: "005_episodes",
        sql: include_str!("../../migrations/005_episodes.sql"),
    },
];

/// Highest schema version known to this binary.
pub fn current_schema_version() -> i32 {
    MIGRATIONS
        .last()
        .map(|m| m.target_version)
        .unwrap_or_default()
}

/// Apply every pending migration to `conn`.
///
/// Idempotent: a connection that is already at the latest version is
/// untouched.
pub fn run(conn: &mut Connection) -> Result<()> {
    let current: i32 = conn.pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))?;

    for migration in MIGRATIONS {
        if migration.target_version <= current {
            continue;
        }
        if migration.target_version != current_for_index(migration) {
            return Err(Error::Migration(format!(
                "migration {} is out of order; expected target_version {}",
                migration.name,
                current_for_index(migration),
            )));
        }
        apply(conn, migration)?;
    }

    Ok(())
}

fn current_for_index(migration: &Migration) -> i32 {
    // target_version equals the 1-based index of the migration.
    MIGRATIONS
        .iter()
        .position(|m| std::ptr::eq(m, migration))
        .map(|p| (p as i32) + 1)
        .unwrap_or(-1)
}

fn apply(conn: &mut Connection, migration: &Migration) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(migration.sql).map_err(|e| {
        Error::Migration(format!("migration {} failed to apply: {e}", migration.name))
    })?;
    tx.pragma_update(None, "user_version", migration.target_version)?;
    tx.commit()?;
    tracing::debug!(
        target: "memryzed::storage::migrations",
        migration = migration.name,
        target_version = migration.target_version,
        "applied migration",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn current_version_matches_migration_count() {
        let expected = MIGRATIONS.len() as i32;
        assert_eq!(current_schema_version(), expected);
    }

    #[test]
    fn run_applies_all_migrations_from_zero() {
        let mut conn = fresh_conn();
        run(&mut conn).unwrap();
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v, current_schema_version());
    }

    #[test]
    fn run_is_a_no_op_when_already_at_latest() {
        let mut conn = fresh_conn();
        run(&mut conn).unwrap();
        run(&mut conn).unwrap();
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v, current_schema_version());
    }
}
