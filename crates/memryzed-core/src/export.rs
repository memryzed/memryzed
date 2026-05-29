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

//! Export and import.
//!
//! Versioned JSON dump of the user's memories and projects so the
//! same data can be moved between machines, version-controlled, or
//! shared with a teammate. Embeddings are not exported; they are
//! regenerated on import.
//!
//! Format described in `docs/data-model.md` under "Export format".

use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::clock::format_epoch_iso;
use crate::error::{Error, Result};
use crate::memory::{Kind, Memory, Scope, Status};
use crate::storage::Database;
use crate::version::VERSION;

/// Current export-format version.
///
/// Increment when the JSON schema changes in a backward-incompatible
/// way. Importers must check this and refuse files they cannot
/// process.
pub const EXPORT_VERSION: &str = "1";

/// Top-level export envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    /// Metadata about this export file.
    pub memryzed_export: ExportMeta,
    /// Every project referenced by the exported memories.
    pub projects: Vec<ExportedProject>,
    /// The exported memories.
    pub memories: Vec<ExportedMemory>,
}

/// Metadata about an export file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMeta {
    /// Export-format version. See [`EXPORT_VERSION`].
    pub version: String,
    /// ISO-8601 UTC time the export was produced.
    pub exported_at: String,
    /// Memryzed version that produced the export.
    pub source_version: String,
}

/// Project record serialization shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedProject {
    /// Stable project identifier.
    pub id: String,
    /// Normalized git remote URL, when known.
    pub git_remote: Option<String>,
    /// Absolute paths the project has been seen at.
    pub local_paths: Vec<String>,
    /// Human-readable display name.
    pub display_name: String,
    /// Unix epoch seconds the project was first seen.
    pub created_at: i64,
    /// Unix epoch seconds the project was most recently seen.
    pub last_seen_at: i64,
}

/// Memory record serialization shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMemory {
    /// Stable memory identifier.
    pub id: String,
    /// Scope kind string (global, project, session).
    pub scope_kind: String,
    /// Project or session id for non-global scopes.
    pub scope_id: Option<String>,
    /// The fact, verbatim.
    pub content: String,
    /// Kind string (preference, fact, decision, todo).
    pub kind: String,
    /// Status string (pending, approved, pinned, archived).
    pub status: String,
    /// Whether the memory is pinned.
    pub pinned: bool,
    /// Extractor confidence, when applicable.
    pub confidence: Option<f64>,
    /// Unix epoch seconds the memory was created.
    pub created_at: i64,
    /// Unix epoch seconds the memory was last updated.
    pub updated_at: i64,
    /// Unix epoch seconds the memory expires, if ever.
    pub expires_at: Option<i64>,
    /// Optional source conversation turn id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_turn_id: Option<String>,
    /// Optional originating client id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_client: Option<String>,
}

/// Build an export from the database.
pub fn build(db: &Database, now: i64) -> Result<Export> {
    let projects = export_projects(db)?;
    let memories = export_memories(db)?;
    Ok(Export {
        memryzed_export: ExportMeta {
            version: EXPORT_VERSION.to_string(),
            exported_at: format_epoch_iso(now),
            source_version: VERSION.to_string(),
        },
        projects,
        memories,
    })
}

/// Serialize an [`Export`] to pretty-printed JSON.
pub fn to_pretty_json(export: &Export) -> Result<String> {
    serde_json::to_string_pretty(export)
        .map_err(|e| Error::Validation(format!("failed to serialize export: {e}")))
}

/// Serialize an [`Export`] to compact JSON.
pub fn to_compact_json(export: &Export) -> Result<String> {
    serde_json::to_string(export)
        .map_err(|e| Error::Validation(format!("failed to serialize export: {e}")))
}

/// Read an export from a file on disk.
pub fn read_from_file(path: &Path) -> Result<Export> {
    let raw = std::fs::read_to_string(path)?;
    parse(&raw)
}

/// Parse an export from raw JSON.
pub fn parse(raw: &str) -> Result<Export> {
    let export: Export = serde_json::from_str(raw)
        .map_err(|e| Error::Validation(format!("invalid export file: {e}")))?;
    if export.memryzed_export.version != EXPORT_VERSION {
        return Err(Error::Validation(format!(
            "unsupported export version {:?}; this build expects {EXPORT_VERSION}",
            export.memryzed_export.version
        )));
    }
    Ok(export)
}

/// Result of an import operation.
#[derive(Debug, Clone, Default)]
pub struct ImportSummary {
    /// Projects inserted because they did not exist.
    pub projects_inserted: usize,
    /// Projects updated in place.
    pub projects_updated: usize,
    /// Memories inserted because they did not exist.
    pub memories_inserted: usize,
    /// Memories updated because the import had a newer version.
    pub memories_updated: usize,
    /// Memories skipped because the existing copy was newer or equal.
    pub memories_skipped: usize,
}

/// Apply an [`Export`] to the database.
///
/// Idempotent on stable IDs. Conflicts are resolved by `last_write_wins`
/// based on `updated_at`: incoming records replace existing ones only
/// when their `updated_at` is strictly newer.
pub fn apply(db: &mut Database, export: &Export) -> Result<ImportSummary> {
    let tx = db.conn_mut().transaction()?;
    let mut summary = ImportSummary::default();

    for p in &export.projects {
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM projects WHERE id = ?1",
                params![p.id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        let local_paths_json = serde_json::to_string(&p.local_paths)
            .map_err(|e| Error::Validation(format!("failed to serialize local_paths: {e}")))?;
        if exists {
            tx.execute(
                "UPDATE projects SET git_remote = ?1, local_paths = ?2, display_name = ?3,
                                     last_seen_at = MAX(last_seen_at, ?4)
                  WHERE id = ?5",
                params![
                    p.git_remote,
                    local_paths_json,
                    p.display_name,
                    p.last_seen_at,
                    p.id,
                ],
            )?;
            summary.projects_updated += 1;
        } else {
            tx.execute(
                "INSERT INTO projects (id, git_remote, local_paths, display_name,
                                       created_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    p.id,
                    p.git_remote,
                    local_paths_json,
                    p.display_name,
                    p.created_at,
                    p.last_seen_at,
                ],
            )?;
            summary.projects_inserted += 1;
        }
    }

    for m in &export.memories {
        let _scope: Scope = m.scope_kind.parse()?;
        let _kind: Kind = m.kind.parse()?;
        let _status: Status = m.status.parse()?;
        let existing_updated: Option<i64> = tx
            .query_row(
                "SELECT updated_at FROM memories WHERE id = ?1",
                params![m.id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(prev) = existing_updated {
            if m.updated_at <= prev {
                summary.memories_skipped += 1;
                continue;
            }
            tx.execute(
                "UPDATE memories SET scope_kind = ?1, scope_id = ?2, content = ?3,
                                     kind = ?4, status = ?5, pinned = ?6, confidence = ?7,
                                     updated_at = ?8, expires_at = ?9,
                                     source_turn_id = ?10, source_client = ?11
                  WHERE id = ?12",
                params![
                    m.scope_kind,
                    m.scope_id,
                    m.content,
                    m.kind,
                    m.status,
                    i64::from(m.pinned),
                    m.confidence,
                    m.updated_at,
                    m.expires_at,
                    m.source_turn_id,
                    m.source_client,
                    m.id,
                ],
            )?;
            summary.memories_updated += 1;
        } else {
            tx.execute(
                "INSERT INTO memories (id, scope_kind, scope_id, content, kind, source_turn_id,
                                       source_client, status, created_at, updated_at,
                                       expires_at, pinned, confidence, embedding_model)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL)",
                params![
                    m.id,
                    m.scope_kind,
                    m.scope_id,
                    m.content,
                    m.kind,
                    m.source_turn_id,
                    m.source_client,
                    m.status,
                    m.created_at,
                    m.updated_at,
                    m.expires_at,
                    i64::from(m.pinned),
                    m.confidence,
                ],
            )?;
            summary.memories_inserted += 1;
        }
    }

    tx.commit()?;
    Ok(summary)
}

fn export_projects(db: &Database) -> Result<Vec<ExportedProject>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, git_remote, local_paths, display_name, created_at, last_seen_at
           FROM projects ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (id, git_remote, paths_json, display_name, created_at, last_seen_at) = r?;
        let local_paths: Vec<String> = serde_json::from_str(&paths_json).map_err(|e| {
            Error::Validation(format!("invalid local_paths JSON in projects.{id}: {e}"))
        })?;
        out.push(ExportedProject {
            id,
            git_remote,
            local_paths,
            display_name,
            created_at,
            last_seen_at,
        });
    }
    Ok(out)
}

fn export_memories(db: &Database) -> Result<Vec<ExportedMemory>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, scope_kind, scope_id, content, kind, status, pinned, confidence,
                created_at, updated_at, expires_at, source_turn_id, source_client
           FROM memories ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Memory {
            id: row.get(0)?,
            scope: row.get::<_, String>(1)?.parse().unwrap_or(Scope::Global),
            scope_id: row.get(2)?,
            content: row.get(3)?,
            kind: row.get::<_, String>(4)?.parse().unwrap_or(Kind::Fact),
            status: row.get::<_, String>(5)?.parse().unwrap_or(Status::Approved),
            pinned: row.get::<_, i64>(6)? != 0,
            confidence: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            expires_at: row.get(10)?,
            source_turn_id: row.get(11)?,
            source_client: row.get(12)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        let m = r?;
        out.push(ExportedMemory {
            id: m.id,
            scope_kind: m.scope.as_db_str().to_string(),
            scope_id: m.scope_id,
            content: m.content,
            kind: m.kind.as_db_str().to_string(),
            status: m.status.as_db_str().to_string(),
            pinned: m.pinned,
            confidence: m.confidence,
            created_at: m.created_at,
            updated_at: m.updated_at,
            expires_at: m.expires_at,
            source_turn_id: m.source_turn_id,
            source_client: m.source_client,
        });
    }
    Ok(out)
}

// rusqlite::OptionalExtension brings the `.optional()` method into scope.
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{insert_with_embedder, NewMemory};
    use crate::NoopEmbedder;

    fn fresh_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn round_trip_export_then_import_preserves_memories() {
        let mut db1 = fresh_db();
        let m = insert_with_embedder(
            &mut db1,
            NewMemory::new(Scope::Global, "I prefer pnpm"),
            &NoopEmbedder,
            100,
        )
        .unwrap();
        let export = build(&db1, 200).unwrap();

        // Import into a fresh database.
        let mut db2 = fresh_db();
        let summary = apply(&mut db2, &export).unwrap();
        assert_eq!(summary.memories_inserted, 1);

        let recovered = crate::memory::get_by_id(&db2, &m.id).unwrap().unwrap();
        assert_eq!(recovered.content, "I prefer pnpm");
        assert_eq!(recovered.scope, Scope::Global);
    }

    #[test]
    fn idempotent_import_skips_existing() {
        let mut db = fresh_db();
        insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "x"),
            &NoopEmbedder,
            100,
        )
        .unwrap();
        let export = build(&db, 200).unwrap();
        let s1 = apply(&mut db, &export).unwrap();
        assert_eq!(s1.memories_skipped, 1);
        assert_eq!(s1.memories_inserted, 0);
    }

    #[test]
    fn newer_updated_at_overwrites() {
        let mut db = fresh_db();
        let m = insert_with_embedder(
            &mut db,
            NewMemory::new(Scope::Global, "old content"),
            &NoopEmbedder,
            100,
        )
        .unwrap();

        let mut export = build(&db, 200).unwrap();
        export.memories[0].content = "new content".into();
        export.memories[0].updated_at = 300;
        let summary = apply(&mut db, &export).unwrap();
        assert_eq!(summary.memories_updated, 1);

        let restored = crate::memory::get_by_id(&db, &m.id).unwrap().unwrap();
        assert_eq!(restored.content, "new content");
    }

    #[test]
    fn reject_unsupported_version() {
        let bad = r#"{
          "memryzed_export":{"version":"99","exported_at":"x","source_version":"x"},
          "projects":[],
          "memories":[]
        }"#;
        let err = parse(bad).unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn export_preserves_pinned_and_confidence() {
        let mut db = fresh_db();
        let mut new = NewMemory::new(Scope::Global, "important");
        new.pinned = true;
        new.confidence = Some(0.9);
        insert_with_embedder(&mut db, new, &NoopEmbedder, 100).unwrap();

        let export = build(&db, 200).unwrap();
        assert_eq!(export.memories.len(), 1);
        assert!(export.memories[0].pinned);
        assert_eq!(export.memories[0].confidence, Some(0.9));
        assert_eq!(export.memories[0].status, "pinned");
    }
}
