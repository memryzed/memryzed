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

//! Sessions: per-task working state scoped to a project.
//!
//! A session captures the state of one task so an agent can resume
//! exactly where it left off. The `state` blob is opaque to Memryzed;
//! agents own its shape (see `docs/for-agent-authors.md`).
//!
//! Lifecycle:
//! - `checkpoint` creates the active session for a project if none
//!   exists, otherwise updates it in place.
//! - `resume` returns the most recent non-archived session for a
//!   project, or a specific session by id.
//! - `end` marks a session completed.

use std::fmt;
use std::str::FromStr;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::id::new_session_id;
use crate::storage::Database;

/// Lifecycle state of a session.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Currently being worked on.
    Active,
    /// Idle but resumable.
    Paused,
    /// Marked complete.
    Completed,
    /// Excluded from normal listings.
    Archived,
}

impl SessionStatus {
    /// Database string.
    pub fn as_db_str(self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Paused => "paused",
            SessionStatus::Completed => "completed",
            SessionStatus::Archived => "archived",
        }
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl FromStr for SessionStatus {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(SessionStatus::Active),
            "paused" => Ok(SessionStatus::Paused),
            "completed" => Ok(SessionStatus::Completed),
            "archived" => Ok(SessionStatus::Archived),
            other => Err(Error::Validation(format!(
                "unknown session status {other:?}"
            ))),
        }
    }
}

/// A session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Stable identifier (`sess_<12hex>`).
    pub id: String,
    /// Project the session belongs to.
    pub project_id: String,
    /// Human-readable title.
    pub title: Option<String>,
    /// Opaque agent-owned state, parsed as JSON.
    pub state: Value,
    /// Lifecycle status.
    pub status: SessionStatus,
    /// Whether the session is pinned (immune to auto-archive).
    pub pinned: bool,
    /// Unix epoch seconds.
    pub created_at: i64,
    /// Unix epoch seconds.
    pub updated_at: i64,
}

/// Create or update the active session for a project.
///
/// If an `active` session exists for `project_id`, its title and
/// state are updated and `updated_at` is bumped. Otherwise a new
/// active session is created. Returns the resulting session.
pub fn checkpoint(
    db: &Database,
    project_id: &str,
    title: Option<String>,
    state: Value,
    now: i64,
) -> Result<Session> {
    let state_json = serde_json::to_string(&state)
        .map_err(|e| Error::Validation(format!("failed to serialize session state: {e}")))?;

    let existing_id: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM sessions
              WHERE project_id = ?1 AND status = 'active'
              ORDER BY updated_at DESC LIMIT 1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(id) = existing_id {
        // Update title only when a new one is supplied.
        if let Some(t) = &title {
            db.conn().execute(
                "UPDATE sessions SET title = ?1, state_blob = ?2, updated_at = ?3 WHERE id = ?4",
                params![t, state_json, now, id],
            )?;
        } else {
            db.conn().execute(
                "UPDATE sessions SET state_blob = ?1, updated_at = ?2 WHERE id = ?3",
                params![state_json, now, id],
            )?;
        }
        return get_by_id(db, &id)?.ok_or_else(|| Error::NotFound {
            kind: "session",
            id,
        });
    }

    let id = new_session_id();
    db.conn().execute(
        "INSERT INTO sessions (id, project_id, title, state_blob, status, pinned, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'active', 0, ?5, ?5)",
        params![id, project_id, title, state_json, now],
    )?;
    get_by_id(db, &id)?.ok_or_else(|| Error::NotFound {
        kind: "session",
        id,
    })
}

/// Return the most recent resumable session for a project.
///
/// Resumable means status is `active` or `paused`. Returns `None`
/// when the project has no resumable session.
pub fn resume_latest(db: &Database, project_id: &str) -> Result<Option<Session>> {
    let id: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM sessions
              WHERE project_id = ?1 AND status IN ('active','paused')
              ORDER BY updated_at DESC LIMIT 1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => get_by_id(db, &id),
        None => Ok(None),
    }
}

/// Look up a session by id.
pub fn get_by_id(db: &Database, id: &str) -> Result<Option<Session>> {
    db.conn()
        .query_row(
            "SELECT id, project_id, title, state_blob, status, pinned, created_at, updated_at
               FROM sessions WHERE id = ?1",
            params![id],
            row_to_session,
        )
        .optional()
        .map_err(Into::into)
}

/// List sessions for a project, most recently updated first.
pub fn list(db: &Database, project_id: &str, limit: Option<u32>) -> Result<Vec<Session>> {
    let mut sql = String::from(
        "SELECT id, project_id, title, state_blob, status, pinned, created_at, updated_at
           FROM sessions WHERE project_id = ?1 ORDER BY updated_at DESC",
    );
    if let Some(n) = limit {
        sql.push_str(&format!(" LIMIT {n}"));
    }
    let mut stmt = db.conn().prepare(&sql)?;
    let rows = stmt.query_map(params![project_id], row_to_session)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Mark a session completed.
pub fn end(db: &Database, id: &str, now: i64) -> Result<Session> {
    let updated = db.conn().execute(
        "UPDATE sessions SET status = 'completed', updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if updated == 0 {
        return Err(Error::NotFound {
            kind: "session",
            id: id.to_string(),
        });
    }
    get_by_id(db, id)?.ok_or_else(|| Error::NotFound {
        kind: "session",
        id: id.to_string(),
    })
}

fn row_to_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let state_json: String = row.get(3)?;
    let status_str: String = row.get(4)?;
    let pinned: i64 = row.get(5)?;
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        state: serde_json::from_str(&state_json).unwrap_or(Value::Null),
        status: status_str.parse().unwrap_or(SessionStatus::Active),
        pinned: pinned != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects;

    fn fresh_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn a_project(db: &Database) -> String {
        let tmp = tempfile::tempdir().unwrap();
        projects::ensure_for_cwd(db, tmp.path(), 100).unwrap().id
    }

    #[test]
    fn checkpoint_creates_then_updates_active_session() {
        let db = fresh_db();
        let pid = a_project(&db);

        let s1 = checkpoint(
            &db,
            &pid,
            Some("Refactor payments".into()),
            serde_json::json!({"open_files": ["a.rs"]}),
            1000,
        )
        .unwrap();
        assert_eq!(s1.status, SessionStatus::Active);
        assert_eq!(s1.title.as_deref(), Some("Refactor payments"));

        let s2 = checkpoint(
            &db,
            &pid,
            None,
            serde_json::json!({"open_files": ["a.rs", "b.rs"]}),
            2000,
        )
        .unwrap();
        assert_eq!(s2.id, s1.id, "same active session is updated in place");
        assert_eq!(s2.created_at, 1000);
        assert_eq!(s2.updated_at, 2000);
        assert_eq!(
            s2.title.as_deref(),
            Some("Refactor payments"),
            "title preserved"
        );
        assert_eq!(s2.state["open_files"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn resume_latest_returns_most_recent() {
        let db = fresh_db();
        let pid = a_project(&db);
        checkpoint(&db, &pid, Some("first".into()), Value::Null, 1000).unwrap();
        // End the first, then create a second.
        let first = resume_latest(&db, &pid).unwrap().unwrap();
        end(&db, &first.id, 1500).unwrap();
        let second = checkpoint(&db, &pid, Some("second".into()), Value::Null, 2000).unwrap();

        let resumed = resume_latest(&db, &pid).unwrap().unwrap();
        assert_eq!(resumed.id, second.id);
        assert_eq!(resumed.title.as_deref(), Some("second"));
    }

    #[test]
    fn resume_latest_none_for_empty_project() {
        let db = fresh_db();
        let pid = a_project(&db);
        assert!(resume_latest(&db, &pid).unwrap().is_none());
    }

    #[test]
    fn end_marks_completed_and_excludes_from_resume() {
        let db = fresh_db();
        let pid = a_project(&db);
        let s = checkpoint(&db, &pid, None, Value::Null, 1000).unwrap();
        let ended = end(&db, &s.id, 1100).unwrap();
        assert_eq!(ended.status, SessionStatus::Completed);
        assert!(resume_latest(&db, &pid).unwrap().is_none());
    }

    #[test]
    fn end_unknown_id_is_not_found() {
        let db = fresh_db();
        let err = end(&db, "sess_nope", 0).unwrap_err();
        assert!(matches!(
            err,
            Error::NotFound {
                kind: "session",
                ..
            }
        ));
    }

    #[test]
    fn list_orders_by_updated_desc_and_respects_limit() {
        let db = fresh_db();
        let pid = a_project(&db);
        let a = checkpoint(&db, &pid, Some("a".into()), Value::Null, 1000).unwrap();
        end(&db, &a.id, 1000).unwrap();
        let b = checkpoint(&db, &pid, Some("b".into()), Value::Null, 2000).unwrap();
        end(&db, &b.id, 2000).unwrap();
        let c = checkpoint(&db, &pid, Some("c".into()), Value::Null, 3000).unwrap();

        let all = list(&db, &pid, None).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, c.id);

        let limited = list(&db, &pid, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }
}
