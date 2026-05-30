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

//! Transcript mining.
//!
//! Ingests existing agent conversation transcripts into Memryzed.
//! Each transcript file becomes a session record for its project, and
//! the user turns in it are fed to the extractor to propose candidate
//! memories. Mining is idempotent: a transcript already mined (tracked
//! by a content hash in the `meta` table) is skipped on re-run.
//!
//! Supported sources:
//! - Kiro CLI session JSONL under `~/.kiro/sessions/`.
//! - Claude Code session JSONL under `~/.claude/projects/`.
//!
//! Both store one JSON object per line. The shapes differ, so each
//! source has a small adapter that normalizes a line into a [`Turn`].

mod source;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::embedder::Embedder;
use crate::error::{Error, Result};
use crate::extractor;
use crate::memory::{self, NewMemory, Scope};
use crate::sessions;
use crate::storage::Database;

pub use source::{detect_source, Source};

/// A single normalized conversation turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// "user" or "assistant".
    pub role: String,
    /// The message text.
    pub text: String,
}

/// Options controlling a mining run.
#[derive(Debug, Clone)]
pub struct MineOptions {
    /// Which source format the transcripts are in.
    pub source: Source,
    /// Auto-approve threshold for extracted candidates.
    pub threshold: f64,
    /// When true, parse and report but write nothing.
    pub dry_run: bool,
    /// When true, re-mine transcripts even if already seen.
    pub force: bool,
}

impl Default for MineOptions {
    fn default() -> Self {
        Self {
            source: Source::Auto,
            threshold: 0.85,
            dry_run: false,
            force: false,
        }
    }
}

/// Per-run summary returned to the caller.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MineReport {
    /// Transcript files discovered under the path.
    pub files_found: usize,
    /// Files actually parsed (not skipped as already-mined).
    pub files_mined: usize,
    /// Files skipped because they were mined before.
    pub files_skipped: usize,
    /// Sessions created or updated.
    pub sessions_written: usize,
    /// Candidate memories stored approved (>= threshold).
    pub memories_approved: usize,
    /// Candidate memories stored pending (< threshold).
    pub memories_pending: usize,
}

/// Mine every transcript under `path` into the database.
///
/// `path` may be a single transcript file or a directory that is
/// walked recursively for `.jsonl`/`.json` files. The embedder is
/// used when an approved candidate needs an embedding; pass a
/// [`crate::NoopEmbedder`] to skip embedding work.
pub fn mine(
    db: &mut Database,
    embedder: &dyn Embedder,
    path: &Path,
    opts: &MineOptions,
    now: i64,
) -> Result<MineReport> {
    let source = match opts.source {
        Source::Auto => source::detect_source(path).unwrap_or(Source::Kiro),
        explicit => explicit,
    };

    let files = discover(path)?;
    let mut report = MineReport {
        files_found: files.len(),
        ..Default::default()
    };

    for file in files {
        let raw = std::fs::read_to_string(&file)?;
        let hash = content_hash(&raw);
        let meta_key = format!("mined:{}", hash);

        if !opts.force && db.meta_get(&meta_key)?.is_some() {
            report.files_skipped += 1;
            continue;
        }

        let turns = source.parse(&raw);
        if turns.is_empty() {
            continue;
        }
        report.files_mined += 1;

        if !opts.dry_run {
            write_session(db, &file, &turns, now)?;
            report.sessions_written += 1;
        }

        let (approved, pending) =
            mine_candidates(db, embedder, &turns, opts.threshold, opts.dry_run, now)?;
        report.memories_approved += approved;
        report.memories_pending += pending;

        if !opts.dry_run {
            db.meta_set(&meta_key, &now.to_string())?;
        }
    }

    Ok(report)
}

/// Walk `path` for transcript files. A file path returns itself.
fn discover(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(Error::Validation(format!(
            "mine path does not exist: {}",
            path.display()
        )));
    }
    let mut out = Vec::new();
    walk(path, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else if matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("jsonl") | Some("json")
        ) {
            out.push(p);
        }
    }
    Ok(())
}

/// Create a session record for one transcript.
///
/// The session is scoped to a synthetic mining project so imported
/// history does not collide with live project sessions. The state
/// blob holds the recent turns and a source note.
fn write_session(db: &Database, file: &Path, turns: &[Turn], now: i64) -> Result<()> {
    let project = crate::projects::ensure_for_cwd(db, &mining_project_dir(), now)?;
    let title = file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| format!("Imported: {s}"))
        .unwrap_or_else(|| "Imported transcript".to_string());

    let recent: Vec<_> = turns
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|t| serde_json::json!({"role": t.role, "content": t.text}))
        .collect();
    let state = serde_json::json!({
        "source": "mined",
        "origin_file": file.to_string_lossy(),
        "turn_count": turns.len(),
        "recent_turns": recent,
    });

    sessions::checkpoint(db, &project.id, Some(title), state, now)?;
    Ok(())
}

/// Feed user turns to the extractor and store candidate memories.
fn mine_candidates(
    db: &mut Database,
    embedder: &dyn Embedder,
    turns: &[Turn],
    threshold: f64,
    dry_run: bool,
    now: i64,
) -> Result<(usize, usize)> {
    let mut approved = 0;
    let mut pending = 0;
    for turn in turns {
        if turn.role != "user" {
            continue;
        }
        for cand in extractor::extract(&turn.text) {
            // Imported candidates are never project-scoped: we cannot
            // know which repo a historical line referred to. Demote
            // project candidates to global.
            let scope = match cand.scope {
                Scope::Project | Scope::Session => Scope::Global,
                Scope::Global => Scope::Global,
            };
            if dry_run {
                if cand.confidence >= threshold {
                    approved += 1;
                } else {
                    pending += 1;
                }
                continue;
            }
            let mut new = NewMemory::new(scope, cand.content.clone());
            new.kind = cand.kind;
            new.confidence = Some(cand.confidence);
            new.source_client = Some("mine".to_string());
            if cand.confidence >= threshold {
                memory::insert_with_embedder(db, new, embedder, now)?;
                approved += 1;
            } else {
                memory::insert_pending(db, new, now)?;
                pending += 1;
            }
        }
    }
    Ok((approved, pending))
}

/// Synthetic working directory used to scope imported sessions.
fn mining_project_dir() -> PathBuf {
    PathBuf::from("memryzed://mined")
}

/// Stable content hash for idempotency. Uses the same SHA-256 helper
/// family as the id module but keeps the full 64-hex digest so two
/// distinct transcripts never collide.
fn content_hash(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopEmbedder;

    fn write_kiro_transcript(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let body = concat!(
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"please remember that I prefer uv over pip"}]}}"#,
            "\n",
            r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"text","data":"Noted."}]}}"#,
            "\n",
        );
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn mine_creates_session_and_candidate_then_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let file = write_kiro_transcript(tmp.path(), "s1.jsonl");
        let mut db = Database::open_in_memory().unwrap();
        let opts = MineOptions {
            source: Source::Kiro,
            ..Default::default()
        };

        let r1 = mine(&mut db, &NoopEmbedder, &file, &opts, 1_000).unwrap();
        assert_eq!(r1.files_found, 1);
        assert_eq!(r1.files_mined, 1);
        assert_eq!(r1.sessions_written, 1);
        // "remember that ..." is confidence 1.0 -> approved.
        assert_eq!(r1.memories_approved, 1);

        // Re-running the same file is a no-op: already-seen by hash.
        let r2 = mine(&mut db, &NoopEmbedder, &file, &opts, 2_000).unwrap();
        assert_eq!(r2.files_skipped, 1);
        assert_eq!(r2.files_mined, 0);
        assert_eq!(r2.sessions_written, 0);
    }

    #[test]
    fn force_remines_an_already_seen_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = write_kiro_transcript(tmp.path(), "s1.jsonl");
        let mut db = Database::open_in_memory().unwrap();
        let base = MineOptions {
            source: Source::Kiro,
            ..Default::default()
        };
        mine(&mut db, &NoopEmbedder, &file, &base, 1_000).unwrap();

        let forced = MineOptions {
            source: Source::Kiro,
            force: true,
            ..Default::default()
        };
        let r = mine(&mut db, &NoopEmbedder, &file, &forced, 2_000).unwrap();
        assert_eq!(r.files_mined, 1);
        assert_eq!(r.files_skipped, 0);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let file = write_kiro_transcript(tmp.path(), "s1.jsonl");
        let mut db = Database::open_in_memory().unwrap();
        let opts = MineOptions {
            source: Source::Kiro,
            dry_run: true,
            ..Default::default()
        };
        let r = mine(&mut db, &NoopEmbedder, &file, &opts, 1_000).unwrap();
        assert_eq!(r.files_mined, 1);
        assert_eq!(r.sessions_written, 0);
        // Counted but not stored.
        assert_eq!(r.memories_approved, 1);
        assert_eq!(
            crate::memory::list(&db, &Default::default()).unwrap().len(),
            0
        );

        // Dry run does not record the hash, so a later real run mines it.
        let real = MineOptions {
            source: Source::Kiro,
            ..Default::default()
        };
        let r2 = mine(&mut db, &NoopEmbedder, &file, &real, 2_000).unwrap();
        assert_eq!(r2.files_mined, 1);
    }

    #[test]
    fn directory_is_walked_for_transcripts() {
        let tmp = tempfile::tempdir().unwrap();
        write_kiro_transcript(tmp.path(), "a.jsonl");
        write_kiro_transcript(tmp.path(), "b.jsonl");
        std::fs::write(tmp.path().join("notes.txt"), "ignored").unwrap();
        let mut db = Database::open_in_memory().unwrap();
        let opts = MineOptions {
            source: Source::Kiro,
            ..Default::default()
        };
        let r = mine(&mut db, &NoopEmbedder, tmp.path(), &opts, 1_000).unwrap();
        // a.jsonl and b.jsonl have identical bodies, so the second
        // hashes the same and is skipped.
        assert_eq!(r.files_found, 2);
        assert_eq!(r.files_mined + r.files_skipped, 2);
    }

    #[test]
    fn missing_path_errors() {
        let mut db = Database::open_in_memory().unwrap();
        let opts = MineOptions::default();
        let err = mine(
            &mut db,
            &NoopEmbedder,
            Path::new("/no/such/path/xyz"),
            &opts,
            1,
        )
        .unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }
}
