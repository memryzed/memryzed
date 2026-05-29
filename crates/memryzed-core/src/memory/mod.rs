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
//! Covers the memory lifecycle: insert (direct user input is
//! `Approved`, or `Pinned` when pinned), pending-queue insert and
//! approval, lookup by ID, scoped listing, and archive (`forget`).

mod kind;
mod scope;
mod status;

pub use kind::Kind;
pub use scope::Scope;
pub use status::Status;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::embedder::Embedder;
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
/// extractor's pending-queue path uses `insert_pending` instead.
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

/// Insert a memory in `pending` status, awaiting user review.
///
/// Used by the extractor for proposed memories below the
/// auto-approve threshold. No embedding is written until the memory
/// is approved (see [`approve`]).
pub fn insert_pending(db: &Database, new: NewMemory, now: i64) -> Result<Memory> {
    validate_new(&new)?;
    let id = new_memory_id();
    db.conn().execute(
        "INSERT INTO memories (
            id, scope_kind, scope_id, content, kind, source_turn_id, source_client,
            status, created_at, updated_at, expires_at, pinned, confidence, embedding_model
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?8, ?9, 0, ?10, NULL)",
        params![
            id,
            new.scope.as_db_str(),
            new.scope_id,
            new.content,
            new.kind.as_db_str(),
            new.source_turn_id,
            new.source_client,
            now,
            new.expires_at,
            new.confidence,
        ],
    )?;
    get_by_id(db, &id)?.ok_or_else(|| Error::memory_not_found(id))
}

/// Approve a pending memory, computing and storing its embedding.
///
/// Transitions status to `approved` (or `pinned` when `pin` is set)
/// and writes the embedding in the same transaction. A no-op if the
/// memory is already approved.
pub fn approve(
    db: &mut Database,
    id: &str,
    pin: bool,
    embedder: &dyn crate::embedder::Embedder,
    now: i64,
) -> Result<Memory> {
    let memory = get_by_id(db, id)?.ok_or_else(|| Error::memory_not_found(id.to_string()))?;
    let status = if pin {
        Status::Pinned
    } else {
        Status::Approved
    };

    let embedding = embedder.embed(&[memory.content.as_str()])?;
    let embedding_vec = embedding.into_iter().next().unwrap_or_default();
    let store_embedding = !embedding_vec.is_empty() && embedder.is_active();

    let tx = db.conn_mut().transaction()?;
    tx.execute(
        "UPDATE memories SET status = ?1, pinned = ?2, updated_at = ?3, embedding_model = ?4
          WHERE id = ?5",
        params![
            status.as_db_str(),
            i64::from(pin),
            now,
            if store_embedding {
                Some(embedder.model_id())
            } else {
                None
            },
            id,
        ],
    )?;
    if store_embedding {
        let dim = embedding_vec.len() as i64;
        let bytes = embedding_to_bytes(&embedding_vec);
        tx.execute(
            "INSERT OR REPLACE INTO memory_embeddings (memory_id, model, dim, embedding)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, embedder.model_id(), dim, bytes],
        )?;
    }
    tx.commit()?;
    get_by_id(db, id)?.ok_or_else(|| Error::memory_not_found(id.to_string()))
}

/// List pending memories awaiting review, oldest first.
pub fn list_pending(db: &Database, limit: Option<u32>) -> Result<Vec<Memory>> {
    list(
        db,
        &ListFilter {
            scope: None,
            scope_id: None,
            statuses: vec![Status::Pending],
            limit,
        },
    )
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

/// Insert a memory together with its embedding in a single transaction.
///
/// The embedder is consulted for the content. If the returned vector
/// is empty (for example, a [`crate::NoopEmbedder`]), no embedding row
/// is written. The `embedding_model` column on `memories` is updated
/// to record which model produced the stored embedding.
pub fn insert_with_embedder(
    db: &mut Database,
    new: NewMemory,
    embedder: &dyn Embedder,
    now: i64,
) -> Result<Memory> {
    validate_new(&new)?;
    let id = new_memory_id();
    let status = if new.pinned {
        Status::Pinned
    } else {
        Status::Approved
    };

    let content_for_embed = new.content.clone();
    let embedding = embedder.embed(&[content_for_embed.as_str()])?;
    let embedding_vec = embedding.into_iter().next().unwrap_or_default();
    let store_embedding = !embedding_vec.is_empty() && embedder.is_active();
    let model_id = if store_embedding {
        Some(embedder.model_id().to_string())
    } else {
        None
    };

    let tx = db.conn_mut().transaction()?;
    tx.execute(
        "INSERT INTO memories (
            id, scope_kind, scope_id, content, kind, source_turn_id, source_client,
            status, created_at, updated_at, expires_at, pinned, confidence, embedding_model
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13)",
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
            model_id,
        ],
    )?;

    if store_embedding {
        let dim = embedding_vec.len() as i64;
        let bytes = embedding_to_bytes(&embedding_vec);
        tx.execute(
            "INSERT INTO memory_embeddings (memory_id, model, dim, embedding)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, embedder.model_id(), dim, bytes],
        )?;
    }
    tx.commit()?;

    get_by_id(db, &id)?.ok_or_else(|| Error::memory_not_found(id))
}

/// Look up the embedding for a memory.
pub fn get_embedding(db: &Database, memory_id: &str) -> Result<Option<StoredEmbedding>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT model, dim, embedding FROM memory_embeddings WHERE memory_id = ?1")?;
    let mut rows = stmt.query(params![memory_id])?;
    if let Some(row) = rows.next()? {
        let model: String = row.get(0)?;
        let dim: i64 = row.get(1)?;
        let bytes: Vec<u8> = row.get(2)?;
        let embedding = bytes_to_embedding(&bytes)?;
        if embedding.len() as i64 != dim {
            return Err(Error::Validation(format!(
                "stored embedding has {} floats but row records dim={dim}",
                embedding.len()
            )));
        }
        Ok(Some(StoredEmbedding {
            memory_id: memory_id.to_string(),
            model,
            embedding,
        }))
    } else {
        Ok(None)
    }
}

/// An embedding row joined with its memory id.
#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    /// Memory the embedding belongs to.
    pub memory_id: String,
    /// Model that produced the embedding.
    pub model: String,
    /// The vector itself.
    pub embedding: Vec<f32>,
}

fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(embedding.len() * 4);
    for f in embedding {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn bytes_to_embedding(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(Error::Validation(format!(
            "embedding bytes length {} is not a multiple of 4",
            bytes.len()
        )));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().expect("chunks_exact size 4");
        out.push(f32::from_le_bytes(arr));
    }
    Ok(out)
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

    /// Deterministic embedder used to test storage round-trip without
    /// loading a real model.
    struct DeterministicEmbedder;

    impl Embedder for DeterministicEmbedder {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0_f32; 4];
                    for (i, c) in t.chars().take(4).enumerate() {
                        v[i] = c as u32 as f32;
                    }
                    v
                })
                .collect())
        }
        fn dimension(&self) -> Option<usize> {
            Some(4)
        }
        fn model_id(&self) -> &str {
            "test-deterministic"
        }
    }

    #[test]
    fn insert_with_embedder_stores_embedding_atomically() {
        let mut db = fresh_db();
        let embedder = DeterministicEmbedder;
        let m = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "abcd"),
            &embedder,
            100,
        )
        .unwrap();

        let stored = get_embedding(&db, &m.id).unwrap().expect("embedding row");
        assert_eq!(stored.memory_id, m.id);
        assert_eq!(stored.model, "test-deterministic");
        assert_eq!(stored.embedding.len(), 4);
        assert_eq!(stored.embedding[0], 'a' as u32 as f32);
        assert_eq!(stored.embedding[1], 'b' as u32 as f32);
    }

    #[test]
    fn insert_with_noop_embedder_does_not_store_embedding() {
        let mut db = fresh_db();
        let embedder = crate::NoopEmbedder;
        let m = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "no embed"),
            &embedder,
            0,
        )
        .unwrap();
        assert!(get_embedding(&db, &m.id).unwrap().is_none());
    }

    #[test]
    fn get_embedding_returns_none_for_unknown_memory() {
        let db = fresh_db();
        assert!(get_embedding(&db, "mem_doesnotexist").unwrap().is_none());
    }

    #[test]
    fn deleting_memory_cascades_to_embedding() {
        let mut db = fresh_db();
        let embedder = DeterministicEmbedder;
        let m = insert_with_embedder(&mut db, NewMemory::new(Scope::Global, "abcd"), &embedder, 0)
            .unwrap();
        assert!(get_embedding(&db, &m.id).unwrap().is_some());
        delete(&db, &m.id).unwrap();
        assert!(get_embedding(&db, &m.id).unwrap().is_none());
    }

    #[test]
    fn embedding_byte_round_trip() {
        let original: Vec<f32> = vec![1.5, -0.25, 0.0, 1234.5];
        let bytes = embedding_to_bytes(&original);
        assert_eq!(bytes.len(), original.len() * 4);
        let restored = bytes_to_embedding(&bytes).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn insert_pending_creates_pending_memory_without_embedding() {
        let db = fresh_db();
        let mut new = NewMemory::new(Scope::Global, "Prefers pnpm over npm");
        new.kind = Kind::Preference;
        new.confidence = Some(0.95);
        let m = insert_pending(&db, new, 100).unwrap();
        assert_eq!(m.status, Status::Pending);
        assert!(get_embedding(&db, &m.id).unwrap().is_none());
    }

    #[test]
    fn pending_memories_are_excluded_from_retrieval_status_default() {
        let db = fresh_db();
        insert_pending(&db, NewMemory::new(Scope::Global, "pending one"), 100).unwrap();
        let approved = list(
            &db,
            &ListFilter {
                statuses: vec![Status::Approved, Status::Pinned],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(approved.is_empty());
        let pending = list_pending(&db, None).unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn approve_transitions_status_and_writes_embedding() {
        let mut db = fresh_db();
        let m = insert_pending(&db, NewMemory::new(Scope::Global, "abcd"), 100).unwrap();
        let approved = approve(&mut db, &m.id, false, &DeterministicEmbedder, 200).unwrap();
        assert_eq!(approved.status, Status::Approved);
        assert_eq!(approved.updated_at, 200);
        assert!(get_embedding(&db, &m.id).unwrap().is_some());
    }

    #[test]
    fn approve_with_pin_sets_pinned_status() {
        let mut db = fresh_db();
        let m = insert_pending(&db, NewMemory::new(Scope::Global, "abcd"), 100).unwrap();
        let approved = approve(&mut db, &m.id, true, &DeterministicEmbedder, 200).unwrap();
        assert_eq!(approved.status, Status::Pinned);
        assert!(approved.pinned);
    }

    #[test]
    fn approve_unknown_id_is_not_found() {
        let mut db = fresh_db();
        let err = approve(&mut db, "mem_nope", false, &crate::NoopEmbedder, 0).unwrap_err();
        assert!(matches!(err, Error::NotFound { kind: "memory", .. }));
    }
}
