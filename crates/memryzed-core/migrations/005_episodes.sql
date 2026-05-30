-- Memryzed migration 005: episodic memory.
--
-- Episodes are verbatim conversation turns captured from agent
-- transcripts (Kiro, Claude Code, Copilot CLI, ...). Unlike the
-- curated `memories` table, episodes are stored as-is and retrieved
-- semantically, so an agent can recall what was said in a prior
-- conversation with any agent. There is no review queue and no
-- status lifecycle: capture is automatic.
--
-- The embedding is a little-endian f32 BLOB, mirroring
-- memory_embeddings. A separate FTS5 table mirrors the text for the
-- hybrid retrieval's keyword leg.

CREATE TABLE episodes (
    id            TEXT PRIMARY KEY,        -- "epi_" + 12 hex
    role          TEXT NOT NULL,           -- 'user' | 'assistant'
    content       TEXT NOT NULL,
    source_agent  TEXT,                    -- 'kiro' | 'claude-code' | 'copilot-cli'
    session_ref   TEXT,                    -- originating transcript identifier
    project_id    TEXT,
    created_at    INTEGER NOT NULL,
    model         TEXT,                    -- embedding model id, NULL if none
    dim           INTEGER,                 -- embedding dimension, NULL if none
    embedding     BLOB                     -- LE f32 array, NULL if none
);

CREATE INDEX episodes_created_at_idx ON episodes(created_at);
CREATE INDEX episodes_agent_idx ON episodes(source_agent);
CREATE INDEX episodes_model_idx ON episodes(model);

CREATE VIRTUAL TABLE episode_fts USING fts5(
    episode_id UNINDEXED,
    content,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER episode_fts_after_insert
AFTER INSERT ON episodes BEGIN
    INSERT INTO episode_fts(episode_id, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER episode_fts_after_delete
AFTER DELETE ON episodes BEGIN
    DELETE FROM episode_fts WHERE episode_id = old.id;
END;
