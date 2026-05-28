-- Memryzed migration 002: per-memory embeddings.
--
-- The embedding is a little-endian f32 array stored as BLOB.
-- v0.1.0-alpha.3 stores embeddings here and v0.1.0-alpha.4 will
-- query them for hybrid retrieval. Sticking with a regular table
-- (rather than a sqlite-vec virtual table) keeps deployment simple
-- in early alphas; we revisit when the working set exceeds a few
-- thousand memories.

CREATE TABLE memory_embeddings (
    memory_id TEXT PRIMARY KEY,
    model     TEXT NOT NULL,
    dim       INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

CREATE INDEX memory_embeddings_model_idx ON memory_embeddings(model);
