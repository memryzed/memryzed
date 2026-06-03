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

//! Episodic memory: verbatim conversation turns.
//!
//! Episodes are the cross-agent continuity layer. Each substantive
//! turn from an agent transcript is stored verbatim, embedded with
//! the local embedding model, and retrieved semantically. No LLM is
//! involved at any point: the embedder turns text into vectors and
//! the agent that calls `recall` does the understanding. This lets a
//! conversation held in one agent be recalled from another.
//!
//! Episodes deliberately have no review queue and no status: capture
//! is automatic, which is the whole point of "remember what I said".

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::embedder::Embedder;
use crate::error::{Error, Result};
use crate::id::new_episode_id;
use crate::retrieval::{cosine_similarity, sanitize_fts_query};
use crate::storage::Database;

/// Minimum content length (chars) for a turn to be worth storing.
/// Filters out "ok", "yes", "continue", and similar noise.
pub const MIN_EPISODE_CHARS: usize = 24;

/// How many turns to embed per call. Bounds memory and latency on
/// transcripts with thousands of turns while keeping the batching
/// speedup over one-at-a-time embedding.
pub const EMBED_BATCH: usize = 32;

/// A stored conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// Stable id (`epi_<12hex>`).
    pub id: String,
    /// "user" or "assistant".
    pub role: String,
    /// Verbatim turn text.
    pub content: String,
    /// Agent the turn came from (kiro, claude-code, copilot-cli).
    pub source_agent: Option<String>,
    /// Originating transcript identifier.
    pub session_ref: Option<String>,
    /// Unix epoch seconds.
    pub created_at: i64,
}

/// A new episode to capture.
#[derive(Debug, Clone)]
pub struct NewEpisode {
    /// "user" or "assistant".
    pub role: String,
    /// Verbatim turn text.
    pub content: String,
    /// Agent the turn came from.
    pub source_agent: Option<String>,
    /// Originating transcript identifier.
    pub session_ref: Option<String>,
    /// Original conversation time in Unix epoch seconds. `None` lets
    /// the caller assign a fallback (real file time or capture time).
    pub created_at: Option<i64>,
}

/// `true` if a turn is substantial enough to store. Trivial
/// acknowledgements and very short turns are skipped to keep
/// retrieval clean.
pub fn is_substantive(content: &str) -> bool {
    content.trim().chars().count() >= MIN_EPISODE_CHARS
}

/// Capture one episode, embedding its content. Returns the stored
/// episode. Callers should check [`is_substantive`] first; this
/// function stores whatever it is given.
pub fn insert(
    db: &mut Database,
    new: NewEpisode,
    embedder: &dyn Embedder,
    now: i64,
) -> Result<Episode> {
    if new.content.trim().is_empty() {
        return Err(Error::Validation(
            "episode content must not be empty".into(),
        ));
    }
    let id = new_episode_id();

    let embedding = embedder.embed(&[new.content.as_str()])?;
    let vec = embedding.into_iter().next().unwrap_or_default();
    let store = !vec.is_empty() && embedder.is_active();
    let (model, dim, bytes): (Option<&str>, Option<i64>, Option<Vec<u8>>) = if store {
        (
            Some(embedder.model_id()),
            Some(vec.len() as i64),
            Some(embedding_to_bytes(&vec)),
        )
    } else {
        (None, None, None)
    };

    db.conn().execute(
        "INSERT OR IGNORE INTO episodes
            (id, role, content, source_agent, session_ref, project_id, created_at, model, dim, embedding)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9)",
        params![
            id,
            new.role,
            new.content,
            new.source_agent,
            new.session_ref,
            new.created_at.unwrap_or(now),
            model,
            dim,
            bytes,
        ],
    )?;

    // With OR IGNORE, a duplicate (role, content) is skipped and our
    // generated id is never stored. Return the existing row in that
    // case so callers still get a valid episode.
    if let Some(ep) = get_by_id(db, &id)? {
        Ok(ep)
    } else {
        get_by_role_content(db, &new.role, &new.content)?.ok_or_else(|| Error::NotFound {
            kind: "episode",
            id,
        })
    }
}

/// Look up an episode by its exact (role, content), used to resolve
/// the existing row when an insert was skipped as a duplicate.
fn get_by_role_content(db: &Database, role: &str, content: &str) -> Result<Option<Episode>> {
    use rusqlite::OptionalExtension;
    db.conn()
        .query_row(
            "SELECT id, role, content, source_agent, session_ref, created_at
               FROM episodes WHERE role = ?1 AND content = ?2 LIMIT 1",
            params![role, content],
            row_to_episode,
        )
        .optional()
        .map_err(Into::into)
}

/// Capture many episodes at once, embedding them in fixed-size
/// batches. Batching is far faster than one-at-a-time, but an
/// unbounded batch (a transcript with thousands of turns) can
/// exhaust memory or stall the model, so turns are embedded in
/// chunks of [`EMBED_BATCH`]. All inserts share one transaction.
/// Returns the number stored.
///
/// `base_now` is the timestamp of the first episode; each subsequent
/// one is `base_now + index` so within-batch order is preserved.
pub fn insert_batch(
    db: &mut Database,
    new: &[NewEpisode],
    embedder: &dyn Embedder,
    base_now: i64,
) -> Result<usize> {
    if new.is_empty() {
        return Ok(0);
    }

    // Embed in bounded chunks, one vector per input (empty entries
    // when the embedder is inactive).
    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(new.len());
    if embedder.is_active() {
        for chunk in new.chunks(EMBED_BATCH) {
            let texts: Vec<&str> = chunk.iter().map(|e| e.content.as_str()).collect();
            let mut out = embedder.embed(&texts)?;
            out.resize(chunk.len(), Vec::new());
            embeddings.extend(out);
        }
    }
    let model = embedder.model_id();

    let tx = db.conn_mut().transaction()?;
    for (i, ep) in new.iter().enumerate() {
        let id = new_episode_id();
        let vec = embeddings.get(i).cloned().unwrap_or_default();
        let store = !vec.is_empty();
        let (m, dim, bytes): (Option<&str>, Option<i64>, Option<Vec<u8>>) = if store {
            (
                Some(model),
                Some(vec.len() as i64),
                Some(embedding_to_bytes(&vec)),
            )
        } else {
            (None, None, None)
        };
        tx.execute(
            "INSERT OR IGNORE INTO episodes
                (id, role, content, source_agent, session_ref, project_id, created_at, model, dim, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9)",
            params![
                id,
                ep.role,
                ep.content,
                ep.source_agent,
                ep.session_ref,
                ep.created_at.unwrap_or(base_now + i as i64),
                m,
                dim,
                bytes,
            ],
        )?;
    }
    tx.commit()?;
    Ok(new.len())
}

/// Capture many episodes without embedding them. Storing text is
/// effectively instant (thousands of rows in well under a second),
/// and the FTS index makes them keyword-searchable immediately. The
/// vector embeddings are filled in later by [`reindex_pending`],
/// run on a background thread by the server. This is what keeps
/// `init` and capture from ever blocking on the embedding model.
pub fn insert_batch_text_only(
    db: &mut Database,
    new: &[NewEpisode],
    base_now: i64,
) -> Result<usize> {
    if new.is_empty() {
        return Ok(0);
    }
    let tx = db.conn_mut().transaction()?;
    for (i, ep) in new.iter().enumerate() {
        let id = new_episode_id();
        tx.execute(
            "INSERT OR IGNORE INTO episodes
                (id, role, content, source_agent, session_ref, project_id, created_at, model, dim, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, NULL, NULL)",
            params![
                id,
                ep.role,
                ep.content,
                ep.source_agent,
                ep.session_ref,
                ep.created_at.unwrap_or(base_now + i as i64),
            ],
        )?;
    }
    tx.commit()?;
    Ok(new.len())
}

/// Number of episodes still awaiting an embedding under the active
/// model. Used by the background indexer to know if there is work.
pub fn pending_embedding_count(db: &Database, model: &str) -> Result<i64> {
    Ok(db.conn().query_row(
        "SELECT count(*) FROM episodes WHERE embedding IS NULL OR model IS NULL OR model != ?1",
        params![model],
        |r| r.get(0),
    )?)
}

/// Embed up to `limit` episodes that have no embedding yet, in a
/// single batched model call, and store the vectors. Returns the
/// number embedded. Designed to be called repeatedly until it
/// returns 0, so it is fully resumable and interruptible: a cancelled
/// run just leaves the remaining episodes for next time.
pub fn reindex_pending(db: &mut Database, embedder: &dyn Embedder, limit: usize) -> Result<usize> {
    if !embedder.is_active() || limit == 0 {
        return Ok(0);
    }
    let model = embedder.model_id().to_string();

    // Pull a batch of un-embedded ids and their text.
    let rows: Vec<(String, String)> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, content FROM episodes
              WHERE embedding IS NULL OR model IS NULL OR model != ?1
              ORDER BY created_at ASC
              LIMIT ?2",
        )?;
        let mapped = stmt.query_map(params![model, limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut v = Vec::new();
        for r in mapped {
            v.push(r?);
        }
        v
    };
    if rows.is_empty() {
        return Ok(0);
    }

    let texts: Vec<&str> = rows.iter().map(|(_, c)| c.as_str()).collect();
    let mut embeddings = embedder.embed(&texts)?;
    embeddings.resize(rows.len(), Vec::new());

    let tx = db.conn_mut().transaction()?;
    let mut done = 0;
    for ((id, _), vec) in rows.iter().zip(embeddings.iter()) {
        if vec.is_empty() {
            continue;
        }
        tx.execute(
            "UPDATE episodes SET model = ?1, dim = ?2, embedding = ?3 WHERE id = ?4",
            params![model, vec.len() as i64, embedding_to_bytes(vec), id],
        )?;
        done += 1;
    }
    tx.commit()?;
    Ok(done)
}

/// Look up an episode by id.
pub fn get_by_id(db: &Database, id: &str) -> Result<Option<Episode>> {
    use rusqlite::OptionalExtension;
    db.conn()
        .query_row(
            "SELECT id, role, content, source_agent, session_ref, created_at
               FROM episodes WHERE id = ?1",
            params![id],
            row_to_episode,
        )
        .optional()
        .map_err(Into::into)
}

/// Total number of stored episodes.
pub fn count(db: &Database) -> Result<i64> {
    Ok(db
        .conn()
        .query_row("SELECT count(*) FROM episodes", [], |r| r.get(0))?)
}

/// One ranked recall hit.
#[derive(Debug, Clone)]
pub struct EpisodeHit {
    /// The episode that matched the query.
    pub episode: Episode,
    /// Combined hybrid score.
    pub score: f32,
    /// The matched turn together with its neighbouring turns from the
    /// same conversation, in chronological order. A single turn is
    /// often too small to answer a query on its own; returning the
    /// surrounding window makes each hit usable and captures answers
    /// that live in adjacent turns. Includes `episode` itself.
    pub context: Vec<Episode>,
}

/// Neighbour turns to include on each side of a recall hit. The
/// answer to a query is frequently in the turn next to the one that
/// matched, so a small window markedly improves usable recall.
pub const CONTEXT_RADIUS: usize = 1;

// Hybrid recall scoring weights. Kept here as named constants so they
// are tuned in one place against the benchmark. The recency weight is
// deliberately small: it helps "what did we work on recently" but
// hurts topical recall ("what did we decide about X").
const W_VECTOR: f32 = 0.55;
const W_FTS: f32 = 0.25;
const W_LEXICAL: f32 = 0.15;
const W_RECENCY: f32 = 0.05;

/// Recall episodes relevant to a query using the same hybrid signals
/// as memory retrieval: vector cosine over embeddings plus an FTS
/// keyword leg, with a recency tilt. Returns the top `limit`.
pub fn recall(
    db: &Database,
    embedder: &dyn Embedder,
    query: &str,
    limit: usize,
    now: i64,
) -> Result<Vec<EpisodeHit>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(Error::Validation("recall query must not be empty".into()));
    }

    use std::collections::HashMap;
    let mut scores: HashMap<String, (f32, f32)> = HashMap::new(); // id -> (vec, fts)

    // Vector leg.
    if embedder.is_active() {
        let q = embedder
            .embed(&[trimmed])?
            .into_iter()
            .next()
            .unwrap_or_default();
        if !q.is_empty() {
            let mut stmt = db.conn().prepare(
                "SELECT id, embedding FROM episodes WHERE embedding IS NOT NULL AND model = ?1",
            )?;
            let rows = stmt.query_map(params![embedder.model_id()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?;
            for r in rows {
                let (id, bytes) = r?;
                let emb = bytes_to_embedding(&bytes)?;
                scores.entry(id).or_insert((0.0, 0.0)).0 = cosine_similarity(&q, &emb);
            }
        }
    }

    // FTS leg.
    let match_expr = sanitize_fts_query(trimmed);
    if !match_expr.is_empty() {
        let mut stmt = db.conn().prepare(
            "SELECT episode_id, bm25(episode_fts) FROM episode_fts WHERE episode_fts MATCH ?1",
        )?;
        let rows = stmt.query_map(params![match_expr], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        let raw: Vec<(String, f64)> = rows.filter_map(|r| r.ok()).collect();
        if let Some(best) = raw.iter().map(|(_, s)| s.abs()).fold(None, max_opt) {
            for (id, bm) in raw {
                let norm = if best == 0.0 {
                    0.0
                } else {
                    (bm.abs() / best) as f32
                };
                scores.entry(id).or_insert((0.0, 0.0)).1 = norm.clamp(0.0, 1.0);
            }
        }
    }

    if scores.is_empty() {
        return Ok(Vec::new());
    }

    // Lower-cased query terms for the lexical rerank: a hit whose text
    // contains the exact query words is boosted. This is a model-free
    // way to reward precise keyword overlap that the vector leg alone
    // can miss.
    let query_terms: Vec<String> = trimmed
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_lowercase())
        .collect();

    // Hydrate and combine.
    let mut hits = Vec::new();
    for (id, (vec_s, fts_s)) in scores {
        if let Some(ep) = get_by_id(db, &id)? {
            let age_days = ((now - ep.created_at).max(0) as f32) / 86_400.0;
            let recency = (-age_days / 30.0).exp().clamp(0.0, 1.0);
            let lexical = lexical_overlap(&ep.content, &query_terms);
            let score =
                (W_VECTOR * vec_s + W_FTS * fts_s + W_LEXICAL * lexical + W_RECENCY * recency)
                    .clamp(0.0, 1.0);
            hits.push(EpisodeHit {
                episode: ep,
                score,
                context: Vec::new(),
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Collapse duplicate content, keeping the first (highest-scored)
    // occurrence. Capture-time dedup prevents most duplicates, but
    // this guards any that predate the unique index and also merges
    // turns that differ only in trailing whitespace.
    let mut seen = std::collections::HashSet::new();
    hits.retain(|h| seen.insert(h.episode.content.trim().to_string()));
    hits.truncate(limit);

    // Attach the surrounding conversation window to each hit.
    for hit in &mut hits {
        hit.context = context_window(db, &hit.episode, CONTEXT_RADIUS)?;
    }
    Ok(hits)
}

/// Fraction of distinct query terms that appear in `content`. A
/// model-free precision signal in [0,1].
fn lexical_overlap(content: &str, query_terms: &[String]) -> f32 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let lc = content.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let hits = query_terms
        .iter()
        .filter(|t| seen.insert(*t) && lc.contains(t.as_str()))
        .count();
    let distinct: std::collections::HashSet<&String> = query_terms.iter().collect();
    hits as f32 / distinct.len() as f32
}

/// Return the matched episode together with up to `radius` neighbour
/// turns on each side from the same conversation, in chronological
/// order. Neighbours are the turns immediately adjacent in the same
/// `session_ref`, ordered by rowid (insertion = transcript order),
/// which is reliable even when all turns share one file-mtime
/// timestamp. Falls back to just the episode when it has no session.
fn context_window(db: &Database, ep: &Episode, radius: usize) -> Result<Vec<Episode>> {
    use rusqlite::OptionalExtension;
    let Some(session) = ep.session_ref.as_deref() else {
        return Ok(vec![ep.clone()]);
    };
    // The matched turn's rowid (insertion order = transcript order).
    let target: Option<i64> = db
        .conn()
        .query_row(
            "SELECT rowid FROM episodes WHERE id = ?1",
            params![ep.id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(target) = target else {
        return Ok(vec![ep.clone()]);
    };
    let r = radius as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, role, content, source_agent, session_ref, created_at
           FROM episodes
          WHERE session_ref = ?1 AND rowid >= ?2 AND rowid <= ?3
          ORDER BY rowid ASC",
    )?;
    let rows = stmt.query_map(params![session, target - r, target + r], row_to_episode)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    if out.is_empty() {
        out.push(ep.clone());
    }
    Ok(out)
}

/// Return the latest `limit` episodes by true conversation time,
/// most recent first. Answers "what did we last discuss" / "what were
/// we working on", which similarity-ranked recall cannot: recall
/// ranks by relevance, not time. Each result carries its
/// conversation window.
pub fn recent(db: &Database, limit: usize, now: i64) -> Result<Vec<EpisodeHit>> {
    let limit = limit.max(1);
    let mut stmt = db.conn().prepare(
        "SELECT id, role, content, source_agent, session_ref, created_at
           FROM episodes ORDER BY created_at DESC, rowid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], row_to_episode)?;
    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let ep = row?;
        if !seen.insert(ep.content.trim().to_string()) {
            continue;
        }
        let age_days = ((now - ep.created_at).max(0) as f32) / 86_400.0;
        let score = (-age_days / 30.0).exp().clamp(0.0, 1.0);
        let context = context_window(db, &ep, CONTEXT_RADIUS)?;
        hits.push(EpisodeHit {
            episode: ep,
            score,
            context,
        });
    }
    Ok(hits)
}

fn max_opt(acc: Option<f64>, x: f64) -> Option<f64> {
    Some(match acc {
        None => x,
        Some(p) => p.max(x),
    })
}

fn row_to_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<Episode> {
    Ok(Episode {
        id: row.get(0)?,
        role: row.get(1)?,
        content: row.get(2)?,
        source_agent: row.get(3)?,
        session_ref: row.get(4)?,
        created_at: row.get(5)?,
    })
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
        return Err(Error::Validation("episode embedding length not /4".into()));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopEmbedder;

    /// Deterministic 4-dim embedder: first char drives one dimension.
    struct DEmb;
    impl Embedder for DEmb {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0_f32; 8];
                    if let Some(c) = t.chars().find(|c| c.is_alphabetic()) {
                        v[(c.to_ascii_lowercase() as usize) % 8] = 1.0;
                    }
                    v
                })
                .collect())
        }
        fn dimension(&self) -> Option<usize> {
            Some(8)
        }
        fn model_id(&self) -> &str {
            "test-d"
        }
    }

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn substantive_filter() {
        assert!(!is_substantive("ok"));
        assert!(!is_substantive("   continue   "));
        assert!(is_substantive(
            "let's switch the deploy step to use eventbridge"
        ));
    }

    #[test]
    fn insert_and_recall_round_trip() {
        let mut d = db();
        insert(
            &mut d,
            NewEpisode {
                role: "user".into(),
                content: "we decided to use eventbridge for the init phase".into(),
                source_agent: Some("kiro".into()),
                session_ref: Some("s1".into()),
                created_at: None,
            },
            &DEmb,
            1_000,
        )
        .unwrap();
        insert(
            &mut d,
            NewEpisode {
                role: "user".into(),
                content: "the frontend is built with tailwind".into(),
                source_agent: Some("kiro".into()),
                session_ref: Some("s1".into()),
                created_at: None,
            },
            &DEmb,
            1_001,
        )
        .unwrap();

        assert_eq!(count(&d).unwrap(), 2);
        let hits = recall(&d, &DEmb, "eventbridge", 5, 2_000).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].episode.content.contains("eventbridge"));
    }

    #[test]
    fn recall_empty_query_errors() {
        let d = db();
        assert!(recall(&d, &NoopEmbedder, "  ", 5, 1).is_err());
    }

    #[test]
    fn recall_with_noop_embedder_uses_fts_only() {
        let mut d = db();
        insert(
            &mut d,
            NewEpisode {
                role: "user".into(),
                content: "remember the postgres connection string lives in API_URL".into(),
                source_agent: None,
                session_ref: None,
                created_at: None,
            },
            &NoopEmbedder,
            1_000,
        )
        .unwrap();
        let hits = recall(&d, &NoopEmbedder, "postgres connection", 5, 2_000).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn recall_returns_neighbour_turns_as_context() {
        let mut d = db();
        let turns = [
            "we are setting up the eks cluster for the dooh project",
            "use a spot node group to keep costs down on eks",
            "the spot group should scale from 2 to 10 nodes",
        ];
        let batch: Vec<NewEpisode> = turns
            .iter()
            .map(|t| NewEpisode {
                role: "user".into(),
                content: (*t).into(),
                source_agent: Some("kiro".into()),
                session_ref: Some("s1".into()),
                created_at: Some(1_000),
            })
            .collect();
        insert_batch_text_only(&mut d, &batch, 1_000).unwrap();

        let hits = recall(&d, &NoopEmbedder, "spot node group", 3, 2_000).unwrap();
        assert!(!hits.is_empty());
        let top = &hits[0];
        // The matched turn plus its neighbours are returned in order.
        assert!(top.context.len() >= 2, "expected a context window");
        assert!(top.context.iter().any(|e| e.id == top.episode.id));
        // Context is contiguous within the same session.
        assert!(top
            .context
            .iter()
            .all(|e| e.session_ref.as_deref() == Some("s1")));
    }

    #[test]
    fn recent_returns_latest_by_time() {
        let mut d = db();
        insert(
            &mut d,
            NewEpisode {
                role: "user".into(),
                content: "older turn about the billing pipeline design".into(),
                source_agent: Some("kiro".into()),
                session_ref: Some("s1".into()),
                created_at: Some(1_000),
            },
            &NoopEmbedder,
            1_000,
        )
        .unwrap();
        insert(
            &mut d,
            NewEpisode {
                role: "user".into(),
                content: "newer turn about the kubernetes upgrade plan".into(),
                source_agent: Some("kiro".into()),
                session_ref: Some("s2".into()),
                created_at: Some(9_000),
            },
            &NoopEmbedder,
            9_000,
        )
        .unwrap();
        let hits = recent(&d, 5, 10_000).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].episode.content.contains("kubernetes upgrade"));
    }
}
