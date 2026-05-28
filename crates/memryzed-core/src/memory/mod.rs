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

//! Memory records and CRUD.
//!
//! v0.1.0-alpha.2 supports the basic lifecycle: insert (always
//! `Approved` for explicit user input), look up by ID, list by
//! scope, and archive (`forget`). Embeddings, FTS, retrieval, and
//! the pending review queue land in subsequent alphas.

mod kind;
mod scope;
mod status;

pub use kind::Kind;
pub use scope::Scope;
pub use status::Status;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::id::new_memory_id;
use crate::storage::Database;

/// A single memory as stored in the `memories` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Stable identifier (`mem_<12hex>`).
    pub id: String,
    /// Where the memory applies.
    pub scope: Scope,
    /// Project or session ID for non-global scopes.
    pub scope_id: Option<String>,
    /// The fact in natural language.
    pub content: String,
    /// Kind of fact.
    pub kind: Kind,
    /// Lifecycle status.
    pub status: Status,
    /// `true` if the user has pinned the memory.
    pub pinned: bool,
    /// Confidence score from the extractor, when applicable.
    pub confidence: Option<f64>,
    /// Unix epoch seconds.
    pub created_at: i64,
    /// Unix epoch seconds.
    pub updated_at: i64,
    /// Unix epoch seconds. `None` means no expiration.
    pub expires_at: Option<i64>,
    /// Optional reference back to the conversation turn.
    pub source_turn_id: Option<String>,
    /// Optional MCP client identifier.
    pub source_client: Option<String>,
}

/// Parameters for inserting a new memory.
///
/// `id`, `created_at`, `updated_at`, and `status` are filled in
/// automatically by [`insert`].
#[derive(Debug, Clone)]
pub struct NewMemory {
    /// Where the memory applies.
    pub scope: Scope,
    /// Project or session ID for non-global scopes.
    pub scope_id: Option<String>,
    /// The fact in natural language.
    pub content: String,
    /// Kind of fact.
    pub kind: Kind,
    /// `true` if the memory should be pinned (immune to expiration).
    pub pinned: bool,
    /// Confidence score from the extractor, when applicable.
    pub confidence: Option<f64>,
    /// Unix epoch seconds at which the memory expires.
    pub expires_at: Option<i64>,
    /// Optional reference back to the conversation turn.
    pub source_turn_id: Option<String>,
    /// Optional MCP client identifier.
    pub source_client: Option<String>,
}

impl NewMemory {
    /// Construct a [`NewMemory`] with sensible defaults for `kind`,
    /// `pinned`, `confidence`, `expires_at`, and source tracking.
    pub fn new(scope: Scope, content: impl Into<String>) -> Self {
        Self {
            scope,
            scope_id: None,
            content: content.into(),
            kind: Kind::default(),
            pinned: false,
            confidence: None,
            expires_at: None,
            source_turn_id: None,
            source_client: None,
        }
    }
}

fn validate_new(new: &NewMemory) -> Result<()> {
    if new.content.trim().is_empty() {
        return Err(Error::Validation("memory content must not be empty".into()));
    }
    match new.scope {
        Scope::Global => {
            if new.scope_id.is_some() {
                return Err(Error::Validation(
                    "global memories must not have a scope_id".into(),
                ));
            }
        }
        Scope::Project | Scope::Session => {
            if new.scope_id.is_none() {
                return Err(Error::Validation(format!(
                    "{}-scoped memories require a scope_id",
                    new.scope
                )));
            }
        }
    }
    if let Some(c) = new.confidence {
        if !(0.0..=1.0).contains(&c) {
            return Err(Error::Validation(format!(
                "confidence must be in [0.0, 1.0], got {c}"
            )));
        }
    }
    Ok(())
}

/// Insert a memory.
///
/// Direct user inserts (this entry point) are stored with status
/// `Pinned` when `pinned` is true and `Approved` otherwise. The
/// extractor's pending-queue path will use a different entry point
/// in a later alpha.
pub fn insert(db: &Database, new: NewMemory, now: i64) -> Result<Memory> {
    validate_new(&new)?;
    let id = new_memory_id();
    let status = if new.pinned {
        Status::Pinned
    } else {
        Status::Approved
    };

    db.conn().execute(
        "INSERT INTO memories (
            id, scope_kind, scope_id, content, kind, source_turn_id, source_client,
            status, created_at, updated_at, expires_at, pinned, confidence, embedding_model
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, NULL)",
        params![
            id,
            new.scope.as_db_str(),
            new.scope_id,
            new.content,
            new.kind.as_db_str(),
            new.source_turn_id,
            new.source_client,
            status.as_db_str(),
            now,
            new.expires_at,
            i64::from(new.pinned),
            new.confidence,
        ],
    )?;

    get_by_id(db, &id)?.ok_or_else(|| Error::memory_not_found(id))
}

/// Look up a memory by ID. Returns `None` if no record exists.
pub fn get_by_id(db: &Database, id: &str) -> Result<Option<Memory>> {
    let mut stmt = db.conn().prepare(SELECT_COLUMNS)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_memory(row)?))
    } else {
        Ok(None)
    }
}

/// Filter for [`list`].
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    /// Restrict to a scope kind. `None` returns memories in every
    /// scope.
    pub scope: Option<Scope>,
    /// Restrict to a specific project or session ID. Only applied
    /// when `scope` is `Project` or `Session`.
    pub scope_id: Option<String>,
    /// Restrict to specific statuses. Empty means "no restriction".
    pub statuses: Vec<Status>,
    /// Maximum rows to return. `None` means no limit.
    pub limit: Option<u32>,
}

/// List memories matching the filter, newest first.
pub fn list(db: &Database, filter: &ListFilter) -> Result<Vec<Memory>> {
    let mut sql = String::from(
        "SELECT id, scope_kind, scope_id, content, kind, status, pinned, confidence,
                created_at, updated_at, expires_at, source_turn_id, source_client
           FROM memories",
    );
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    if let Some(scope) = filter.scope {
        conditions.push(format!("scope_kind = ?{}", binds.len() + 1));
        binds.push(Box::new(scope.as_db_str().to_string()));
        if let Some(id) = &filter.scope_id {
            conditions.push(format!("scope_id = ?{}", binds.len() + 1));
            binds.push(Box::new(id.clone()));
        }
    }

    if !filter.statuses.is_empty() {
        let placeholders: Vec<String> = (0..filter.statuses.len())
            .map(|i| format!("?{}", binds.len() + i + 1))
            .collect();
        conditions.push(format!("status IN ({})", placeholders.join(", ")));
        for s in &filter.statuses {
            binds.push(Box::new(s.as_db_str().to_string()));
        }
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY pinned DESC, created_at DESC");

    if let Some(limit) = filter.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }

    let mut stmt = db.conn().prepare(&sql)?;
    let bind_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(bind_refs.as_slice(), |row| Ok(row_to_memory(row)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

/// Archive a memory. The row remains in the database for audit but
/// is excluded from retrieval.
pub fn archive(db: &Database, id: &str, now: i64) -> Result<Memory> {
    let updated = db.conn().execute(
        "UPDATE memories SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![Status::Archived.as_db_str(), now, id],
    )?;
    if updated == 0 {
        return Err(Error::memory_not_found(id.to_string()));
    }
    get_by_id(db, id)?.ok_or_else(|| Error::memory_not_found(id.to_string()))
}

/// Permanently delete a memory. Used by `forget --hard` only.
pub fn delete(db: &Database, id: &str) -> Result<()> {
    let updated = db
        .conn()
        .execute("DELETE FROM memories WHERE id = ?1", params![id])?;
    if updated == 0 {
        return Err(Error::memory_not_found(id.to_string()));
    }
    Ok(())
}

const SELECT_COLUMNS: &str = "
    SELECT id, scope_kind, scope_id, content, kind, status, pinned, confidence,
           created_at, updated_at, expires_at, source_turn_id, source_client
      FROM memories
     WHERE id = ?1
";

fn row_to_memory(row: &rusqlite::Row<'_>) -> Result<Memory> {
    let id: String = row.get(0)?;
    let scope_str: String = row.get(1)?;
    let scope: Scope = scope_str.parse()?;
    let scope_id: Option<String> = row.get(2)?;
    let content: String = row.get(3)?;
    let kind_str: String = row.get(4)?;
    let kind: Kind = kind_str.parse()?;
    let status_str: String = row.get(5)?;
    let status: Status = status_str.parse()?;
    let pinned: i64 = row.get(6)?;
    let confidence: Option<f64> = row.get(7)?;
    let created_at: i64 = row.get(8)?;
    let updated_at: i64 = row.get(9)?;
    let expires_at: Option<i64> = row.get(10)?;
    let source_turn_id: Option<String> = row.get(11)?;
    let source_client: Option<String> = row.get(12)?;
    Ok(Memory {
        id,
        scope,
        scope_id,
        content,
        kind,
        status,
        pinned: pinned != 0,
        confidence,
        created_at,
        updated_at,
        expires_at,
        source_turn_id,
        source_client,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn insert_and_get_round_trip() {
        let db = fresh_db();
        let m = insert(&db, NewMemory::new(Scope::Global, "I prefer pnpm"), 100).unwrap();
        let fetched = get_by_id(&db, &m.id).unwrap().unwrap();
        assert_eq!(fetched.content, "I prefer pnpm");
        assert_eq!(fetched.scope, Scope::Global);
        assert_eq!(fetched.status, Status::Approved);
        assert_eq!(fetched.created_at, 100);
        assert_eq!(fetched.updated_at, 100);
        assert!(!fetched.pinned);
    }

    #[test]
    fn pinned_memory_has_pinned_status() {
        let db = fresh_db();
        let mut new = NewMemory::new(Scope::Global, "always use rebase");
        new.pinned = true;
        let m = insert(&db, new, 0).unwrap();
        assert_eq!(m.status, Status::Pinned);
        assert!(m.pinned);
    }

    #[test]
    fn empty_content_is_rejected() {
        let db = fresh_db();
        let err = insert(&db, NewMemory::new(Scope::Global, "  "), 0).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn project_scope_requires_scope_id() {
        let db = fresh_db();
        let err = insert(&db, NewMemory::new(Scope::Project, "uses pnpm"), 0).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn global_scope_rejects_scope_id() {
        let db = fresh_db();
        let mut new = NewMemory::new(Scope::Global, "x");
        new.scope_id = Some("proj_abc".into());
        let err = insert(&db, new, 0).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn confidence_out_of_range_is_rejected() {
        let db = fresh_db();
        let mut new = NewMemory::new(Scope::Global, "x");
        new.confidence = Some(2.0);
        let err = insert(&db, new, 0).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn list_filters_by_scope() {
        let db = fresh_db();
        insert(&db, NewMemory::new(Scope::Global, "g"), 0).unwrap();
        let mut p = NewMemory::new(Scope::Project, "p");
        p.scope_id = Some("proj_abc".into());
        insert(&db, p, 1).unwrap();

        let only_global = list(
            &db,
            &ListFilter {
                scope: Some(Scope::Global),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(only_global.len(), 1);
        assert_eq!(only_global[0].scope, Scope::Global);
    }

    #[test]
    fn list_filters_by_status() {
        let db = fresh_db();
        let m = insert(&db, NewMemory::new(Scope::Global, "x"), 0).unwrap();
        archive(&db, &m.id, 1).unwrap();

        let only_approved = list(
            &db,
            &ListFilter {
                statuses: vec![Status::Approved, Status::Pinned],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(only_approved.is_empty());

        let any = list(&db, &ListFilter::default()).unwrap();
        assert_eq!(any.len(), 1);
        assert_eq!(any[0].status, Status::Archived);
    }

    #[test]
    fn list_orders_pinned_first_then_newest() {
        let db = fresh_db();
        insert(&db, NewMemory::new(Scope::Global, "old"), 100).unwrap();
        insert(&db, NewMemory::new(Scope::Global, "new"), 200).unwrap();
        let mut pinned = NewMemory::new(Scope::Global, "pinned");
        pinned.pinned = true;
        insert(&db, pinned, 0).unwrap();

        let all = list(&db, &ListFilter::default()).unwrap();
        assert_eq!(all[0].content, "pinned", "pinned should be first");
        assert_eq!(all[1].content, "new", "newest non-pinned next");
        assert_eq!(all[2].content, "old");
    }

    #[test]
    fn archive_marks_status_and_updates_timestamp() {
        let db = fresh_db();
        let m = insert(&db, NewMemory::new(Scope::Global, "x"), 100).unwrap();
        let archived = archive(&db, &m.id, 200).unwrap();
        assert_eq!(archived.status, Status::Archived);
        assert_eq!(archived.updated_at, 200);
        assert_eq!(archived.created_at, 100);
    }

    #[test]
    fn archive_unknown_id_returns_not_found() {
        let db = fresh_db();
        let err = archive(&db, "mem_doesnotexist", 0).unwrap_err();
        assert!(matches!(err, Error::NotFound { kind: "memory", .. }));
    }

    #[test]
    fn delete_removes_the_row() {
        let db = fresh_db();
        let m = insert(&db, NewMemory::new(Scope::Global, "x"), 0).unwrap();
        delete(&db, &m.id).unwrap();
        assert!(get_by_id(&db, &m.id).unwrap().is_none());
    }
}
