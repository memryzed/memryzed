-- Memryzed initial schema, migration 001.
--
-- This migration creates the base tables that v0.1.0-alpha.2 needs:
-- memories, projects, recall_log, and meta. Vector and full-text
-- index virtual tables, and the sessions table, land in subsequent
-- migrations as the corresponding features ship (alpha.3, alpha.4,
-- and the sessions release).

CREATE TABLE memories (
    id              TEXT PRIMARY KEY,
    scope_kind      TEXT NOT NULL,
    scope_id        TEXT,
    content         TEXT NOT NULL,
    kind            TEXT NOT NULL,
    source_turn_id  TEXT,
    source_client   TEXT,
    status          TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    pinned          INTEGER NOT NULL DEFAULT 0,
    confidence      REAL,
    embedding_model TEXT,
    CHECK (scope_kind IN ('global','project','session')),
    CHECK (status IN ('pending','approved','pinned','archived')),
    CHECK (kind IN ('preference','fact','decision','todo'))
);

CREATE INDEX memories_scope_idx ON memories(scope_kind, scope_id);
CREATE INDEX memories_status_idx ON memories(status);
CREATE INDEX memories_created_at_idx ON memories(created_at);
CREATE INDEX memories_expires_at_idx ON memories(expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE projects (
    id            TEXT PRIMARY KEY,
    git_remote    TEXT,
    local_paths   TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    last_seen_at  INTEGER NOT NULL
);

CREATE INDEX projects_last_seen_idx ON projects(last_seen_at);

CREATE TABLE recall_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL,
    query       TEXT,
    scope       TEXT,
    project_id  TEXT,
    memory_ids  TEXT,
    session_id  TEXT,
    client      TEXT,
    detail      TEXT,
    at          INTEGER NOT NULL
);

CREATE INDEX recall_log_at_idx ON recall_log(at);
CREATE INDEX recall_log_kind_idx ON recall_log(kind);

CREATE TABLE meta (
    key    TEXT PRIMARY KEY,
    value  TEXT
);
