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

//! Project records: identifying repositories.

pub mod identity;

use std::collections::BTreeSet;
use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::storage::Database;

pub use identity::{compute as compute_identity, ProjectIdentity};

/// A project as stored in the `projects` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Stable project identifier (`proj_*` or `proj_local_*`).
    pub id: String,
    /// Normalized git remote URL when one is known.
    pub git_remote: Option<String>,
    /// Every absolute path at which this project has been seen.
    pub local_paths: Vec<String>,
    /// Human-readable display name (defaults to the cwd's basename).
    pub display_name: String,
    /// First time the project was seen, Unix epoch seconds.
    pub created_at: i64,
    /// Most recent time the project was seen, Unix epoch seconds.
    pub last_seen_at: i64,
}

/// Get-or-create the project record for the given working directory.
///
/// Computes the project identity, inserts a new row if needed,
/// otherwise updates `last_seen_at` and merges in the new path.
pub fn ensure_for_cwd(db: &Database, cwd: &Path, now: i64) -> Result<Project> {
    let id = identity::compute(cwd)?;
    upsert(db, &id, now)
}

fn upsert(db: &Database, identity: &ProjectIdentity, now: i64) -> Result<Project> {
    let path_str = identity.absolute_path.to_string_lossy().to_string();
    if let Some(existing) = get_by_id(db, &identity.id)? {
        let mut paths: BTreeSet<String> = existing.local_paths.into_iter().collect();
        paths.insert(path_str.clone());
        let paths_vec: Vec<String> = paths.into_iter().collect();
        let serialized = serde_json::to_string(&paths_vec)
            .map_err(|e| Error::Validation(format!("failed to serialize local_paths: {e}")))?;
        db.conn().execute(
            "UPDATE projects
                SET local_paths = ?1, last_seen_at = ?2
              WHERE id = ?3",
            params![serialized, now, identity.id],
        )?;
        return get_by_id(db, &identity.id)?
            .ok_or_else(|| Error::project_not_found(identity.id.clone()));
    }

    let paths_vec = vec![path_str];
    let serialized = serde_json::to_string(&paths_vec)
        .map_err(|e| Error::Validation(format!("failed to serialize local_paths: {e}")))?;
    db.conn().execute(
        "INSERT INTO projects (id, git_remote, local_paths, display_name, created_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            identity.id,
            identity.git_remote,
            serialized,
            identity.display_name,
            now,
        ],
    )?;
    get_by_id(db, &identity.id)?.ok_or_else(|| Error::project_not_found(identity.id.clone()))
}

/// Look up a project by ID. Returns `None` when no row exists.
pub fn get_by_id(db: &Database, id: &str) -> Result<Option<Project>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, git_remote, local_paths, display_name, created_at, last_seen_at
           FROM projects
          WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_project(row)?))
    } else {
        Ok(None)
    }
}

/// List all known projects, most recently seen first.
pub fn list(db: &Database) -> Result<Vec<Project>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, git_remote, local_paths, display_name, created_at, last_seen_at
           FROM projects
          ORDER BY last_seen_at DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok(row_to_project(r)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

fn row_to_project(row: &rusqlite::Row<'_>) -> Result<Project> {
    let id: String = row.get(0)?;
    let git_remote: Option<String> = row.get(1)?;
    let local_paths_json: String = row.get(2)?;
    let display_name: String = row.get(3)?;
    let created_at: i64 = row.get(4)?;
    let last_seen_at: i64 = row.get(5)?;
    let local_paths: Vec<String> = serde_json::from_str(&local_paths_json)
        .map_err(|e| Error::Validation(format!("invalid local_paths JSON for {id:?}: {e}")))?;
    Ok(Project {
        id,
        git_remote,
        local_paths,
        display_name,
        created_at,
        last_seen_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn ensure_inserts_then_updates() {
        let db = fresh_db();
        let tmp = tempfile::tempdir().unwrap();

        let p1 = ensure_for_cwd(&db, tmp.path(), 1_000).unwrap();
        assert_eq!(p1.created_at, 1_000);
        assert_eq!(p1.last_seen_at, 1_000);
        assert_eq!(p1.local_paths.len(), 1);

        // Same path again, later timestamp.
        let p2 = ensure_for_cwd(&db, tmp.path(), 2_000).unwrap();
        assert_eq!(p2.id, p1.id);
        assert_eq!(p2.created_at, 1_000);
        assert_eq!(p2.last_seen_at, 2_000);
        assert_eq!(p2.local_paths.len(), 1, "same path should not duplicate");
    }

    #[test]
    fn list_orders_by_last_seen_desc() {
        let db = fresh_db();
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();

        ensure_for_cwd(&db, tmp_a.path(), 100).unwrap();
        ensure_for_cwd(&db, tmp_b.path(), 200).unwrap();

        let projects = list(&db).unwrap();
        assert_eq!(projects.len(), 2);
        assert!(projects[0].last_seen_at >= projects[1].last_seen_at);
    }

    #[test]
    fn get_by_id_returns_none_for_unknown() {
        let db = fresh_db();
        assert!(get_by_id(&db, "proj_doesnotexist").unwrap().is_none());
    }
}
