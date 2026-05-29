-- Memryzed migration 004: sessions.
--
-- A session is a per-task working-state record scoped to a project.
-- The state_blob is opaque JSON owned by the agent; Memryzed only
-- interprets the metadata columns.

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL,
    title       TEXT,
    state_blob  TEXT NOT NULL,
    status      TEXT NOT NULL,
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    CHECK (status IN ('active','paused','completed','archived'))
);

CREATE INDEX sessions_project_idx ON sessions(project_id, updated_at);
CREATE INDEX sessions_status_idx ON sessions(status);
