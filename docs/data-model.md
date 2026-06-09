# Data model

This document defines the on-disk format and the database schema used
by Memryzed. It is the authoritative reference for what is stored,
where, and how.

## Filesystem layout

All Memryzed data lives under `~/.memryzed/` on Unix and
`%LOCALAPPDATA%\memryzed\` on Windows. The data directory can be
overridden by the `--data-dir` flag, the `MEMRYZED_DATA_DIR`
environment variable, or `[general] data_dir` in `config.toml`.

    ~/.memryzed/
      bin/
        memryzed              The binary itself, on Unix.
      config.toml             User configuration. See docs/configuration.md.
      db.sqlite               The main database. See "Database schema" below.
      db.sqlite-wal           SQLite write-ahead log. Managed by SQLite.
      db.sqlite-shm           SQLite shared memory file. Managed by SQLite.
      models/
        bge-small-en-v1.5/    The embedding model, downloaded on init.
          model.onnx
          tokenizer.json
          config.json
      audit.log               Append-only structured event log.
      audit.log.<n>           Rotated audit logs.

On Windows the layout is the same but rooted at `%LOCALAPPDATA%\memryzed\`.

## Database schema

The database is SQLite with the following extensions enabled:

- `sqlite-vec` for vector similarity search.
- `fts5` (built into SQLite) for full-text search.

WAL journal mode is enabled. The busy timeout is 5000 milliseconds.
The user_version pragma is used as the schema version number; it is
incremented on every migration.

### `memories`

The primary table for stored facts.

    CREATE TABLE memories (
      id              TEXT PRIMARY KEY,        -- "mem_" + 12 hex chars
      scope_kind      TEXT NOT NULL,           -- 'global' | 'project' | 'session'
      scope_id        TEXT,                    -- NULL for global; project_id; or session_id
      content         TEXT NOT NULL,           -- The fact in natural language
      kind            TEXT NOT NULL,           -- 'preference' | 'fact' | 'decision' | 'todo'
      source_turn_id  TEXT,                    -- Optional link to the source conversation turn
      source_client   TEXT,                    -- Which MCP client produced the source turn
      status          TEXT NOT NULL,           -- 'pending' | 'approved' | 'pinned' | 'archived'
      created_at      INTEGER NOT NULL,        -- Unix epoch seconds
      updated_at      INTEGER NOT NULL,
      expires_at      INTEGER,                 -- NULL for no expiration
      pinned          INTEGER NOT NULL DEFAULT 0,  -- 0 or 1
      confidence      REAL,                    -- 0.0 to 1.0; NULL for explicit user input
      embedding_model TEXT NOT NULL,           -- Model identifier, for migration
      CHECK (scope_kind IN ('global','project','session')),
      CHECK (status IN ('pending','approved','pinned','archived')),
      CHECK (kind IN ('preference','fact','decision','todo'))
    );

    CREATE INDEX memories_scope ON memories(scope_kind, scope_id);
    CREATE INDEX memories_status ON memories(status);
    CREATE INDEX memories_created_at ON memories(created_at);
    CREATE INDEX memories_expires_at ON memories(expires_at) WHERE expires_at IS NOT NULL;

### `memory_vectors`

The vector index, backed by sqlite-vec.

    CREATE VIRTUAL TABLE memory_vectors USING vec0(
      memory_id TEXT PRIMARY KEY,
      embedding FLOAT[384]   -- BGE-small-en-v1.5 dimension
    );

When the embedding model changes, this table is rebuilt during the
next start.

### `memory_fts`

The full-text index, backed by FTS5.

    CREATE VIRTUAL TABLE memory_fts USING fts5(
      memory_id UNINDEXED,
      content,
      tokenize = 'unicode61 remove_diacritics 2'
    );

Insertions, updates, and deletions on `memories.content` are mirrored
into this table by triggers.

### `projects`

    CREATE TABLE projects (
      id            TEXT PRIMARY KEY,          -- "proj_" or "proj_local_" + 12 hex chars
      git_remote    TEXT,                      -- Normalized git remote URL, if any
      local_paths   TEXT NOT NULL,             -- JSON array of absolute paths
      display_name  TEXT NOT NULL,
      created_at    INTEGER NOT NULL,
      last_seen_at  INTEGER NOT NULL
    );

    CREATE INDEX projects_last_seen ON projects(last_seen_at);

### `sessions`

    CREATE TABLE sessions (
      id          TEXT PRIMARY KEY,            -- "sess_" + 12 hex chars
      project_id  TEXT NOT NULL,
      title       TEXT,
      state_blob  TEXT NOT NULL,               -- JSON; opaque to Memryzed
      status      TEXT NOT NULL,               -- 'active' | 'paused' | 'completed' | 'archived'
      pinned      INTEGER NOT NULL DEFAULT 0,
      created_at  INTEGER NOT NULL,
      updated_at  INTEGER NOT NULL,
      FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
      CHECK (status IN ('active','paused','completed','archived'))
    );

    CREATE INDEX sessions_project ON sessions(project_id, updated_at);
    CREATE INDEX sessions_status ON sessions(status);

### `recall_log`

The structured event log table. Every recall, remember, forget, and
session change writes a row here. Rows older than 90 days are pruned
on a background sweep.

    CREATE TABLE recall_log (
      id          INTEGER PRIMARY KEY AUTOINCREMENT,
      kind        TEXT NOT NULL,               -- 'recall' | 'remember' | 'forget' | etc.
      query       TEXT,
      scope       TEXT,
      project_id  TEXT,
      memory_ids  TEXT,                        -- JSON array
      session_id  TEXT,
      client      TEXT,
      detail      TEXT,                        -- JSON for kind-specific fields
      at          INTEGER NOT NULL
    );

    CREATE INDEX recall_log_at ON recall_log(at);
    CREATE INDEX recall_log_kind ON recall_log(kind);

### `meta`

Single-row table for store-wide metadata.

    CREATE TABLE meta (
      key    TEXT PRIMARY KEY,
      value  TEXT
    );

Keys currently used:

    schema_version           Integer string. Mirrors PRAGMA user_version.
    embedding_model          The currently active embedding model identifier.
    last_update_check_at     Unix epoch seconds.
    install_id               Random opaque ID for telemetry, generated once.

## ID formats

All identifiers are stable, opaque strings. They are not derived from
content, so the same fact stored twice produces two different IDs.

    Memory:  mem_<12 hex>
    Project: proj_<12 hex>            (with git remote)
             proj_local_<12 hex>      (without git remote)
    Session: sess_<12 hex>

The hex portion is generated by hashing a per-row random value. The
same memory exported and re-imported preserves its ID.

## The audit log file

`audit.log` is a plain text file with one JSON object per line
(JSONL). Each line has at minimum:

    {
      "ts": "2026-05-28T17:32:14.123Z",
      "kind": "recall",
      "client": "claude-code",
      "detail": { /* kind-specific fields */ }
    }

Kinds in v1:

    init                 Memryzed initialized data dir.
    serve_start          MCP server started.
    serve_stop           MCP server stopped.
    recall               recall() called.
    remember             remember() called.
    forget               forget() called.
    list_memories        list_memories() called.
    checkpoint           checkpoint() called.
    resume               resume() called.
    list_sessions        list_sessions() called.
    end_session          end_session() called.
    extractor_propose    Background extractor proposed a memory.
    review_approve       User approved a pending memory.
    review_reject        User rejected a pending memory.
    integration_install  An MCP client config was modified.
    config_change        config.toml was modified.
    update_available     A newer version is available.
    update_install       memryzed update installed a new version.
    rate_limited         A client exceeded rate limits.
    telemetry_emit       A telemetry payload was sent.

The file is rotated when it exceeds 10 MB. The rotated files are kept
for 90 days.

## Migrations

Schema changes are managed by a versioned migration system. Each
migration is a single SQL file checked into the source tree under
`crates/memryzed-core/migrations/`. The filename pattern is:

    NNN_short_description.sql

where NNN is a zero-padded sequence starting at 001. On every server
start, the migration runner:

1. Reads `PRAGMA user_version`.
2. Applies every migration with a number greater than the current
   user_version, in order, in transactions.
3. Sets `PRAGMA user_version` to the highest number applied.

Migrations must be backward-compatible at the schema level so that an
older binary can still open a newer database for at least one minor
version. Destructive migrations (drops, type changes) require a major
version bump. See `docs/development/versioning.md`.

## Backups

The database is a single file. The standard SQLite backup techniques
apply:

- `memryzed export > backup.json` produces a portable, human-readable
  backup that can be re-imported into any version that supports the
  same export schema version.
- For a binary backup, copy `db.sqlite`, `db.sqlite-wal`, and
  `db.sqlite-shm` while Memryzed is not running, or use the SQLite
  online backup API by running `sqlite3 ~/.memryzed/db.sqlite ".backup
  /path/to/backup.sqlite"`.

The JSON export is the recommended backup format for users.

## Export format

`memryzed export` produces this JSON structure:

    {
      "memryzed_export": {
        "version": "1",
        "exported_at": "2026-05-28T17:34:17Z",
        "source_version": "v0.1.0"
      },
      "config": { /* config.toml as JSON */ },
      "projects": [ /* projects rows */ ],
      "memories": [ /* memories rows, without embeddings */ ],
      "sessions": [ /* sessions rows */ ]
    }

Embeddings are not exported. They are regenerated on import. This
keeps export files small (a typical export with several hundred
memories is well under 1 MB) and avoids embedding-model-version
mismatches across machines.

The export schema is itself versioned. The current version is `1`.
When the export schema changes in a backward-incompatible way, the
version is bumped and `memryzed import` includes a migration step
between versions.

## Constraints we keep

- All user-meaningful state is in `db.sqlite`. Removing it cleanly
  resets Memryzed.
- All transient state is in `db.sqlite-wal` and `db.sqlite-shm`, which
  SQLite manages.
- The audit log is the only file that grows without bound on the user
  side. It is rotated and the rotation is configurable.
- Models are downloaded artifacts; deleting `models/` triggers a
  re-download on next start.
- The schema is designed to stay the same shape if optional sync is
  added later. Any sync-only tables would live in a separate
  namespace so the local database stays clean.
