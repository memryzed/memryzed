-- Memryzed migration 006: deduplicate episodes.
--
-- Episodes captured before this migration could contain exact
-- duplicates (the same turn mined from overlapping transcripts or
-- across repeated test runs). This removes existing duplicates,
-- keeping one row per (role, content), and adds a unique index so a
-- turn can never be stored twice. Inserts use INSERT OR IGNORE so a
-- duplicate is silently skipped rather than erroring.

-- Collapse exact duplicates, keeping the earliest-inserted row.
DELETE FROM episodes
 WHERE rowid NOT IN (
   SELECT min(rowid) FROM episodes GROUP BY role, content
 );

-- Keep the FTS index in sync after the bulk delete.
DELETE FROM episode_fts
 WHERE episode_id NOT IN (SELECT id FROM episodes);

-- Enforce uniqueness going forward.
CREATE UNIQUE INDEX episodes_role_content_idx ON episodes(role, content);
