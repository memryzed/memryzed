# Architecture

This document describes the internal architecture of Memryzed: the
processes, modules, data flows, and the contracts between them. It is
written for contributors and operators. For the user-level model of
how Memryzed behaves, see `docs/concepts.md`. For the precise on-disk
format, see `docs/data-model.md`.

## High-level picture

Memryzed is a single binary that runs as one of three personas
depending on how it is invoked:

- MCP server (`memryzed serve`). Spawned by an MCP-aware client over
  stdio. Long-running for the lifetime of the client session. Serves
  the eight MCP tools.
- CLI (`memryzed <command>`). Short-lived processes invoked by the
  user. Read and write the same database the server uses.
- TUI (`memryzed review`). A long-running interactive process for
  triaging the pending memory queue.

All three personas share the same core library and storage layer. The
SQLite database supports concurrent readers and a single writer; we
serialize writes through a process-local mutex inside each persona to
keep behavior predictable when multiple personas run at the same time.

## Process model

A typical user session has these processes running:

    +------------------------+     +------------------------+
    |   MCP-aware client     |     |   User's terminal      |
    |   (Claude Code, etc.)  |     |                        |
    +-----------+------------+     +-----------+------------+
                |                              |
                | stdio MCP                    | direct invocation
                v                              v
    +------------------------+     +------------------------+
    |  memryzed serve        |     |  memryzed list / etc.  |
    |  (long-running)        |     |  (short-lived)         |
    +-----------+------------+     +-----------+------------+
                |                              |
                +--------------+---------------+
                               |
                               v
                  +------------+------------+
                  |  ~/.memryzed/db.sqlite  |
                  |  ~/.memryzed/models/    |
                  |  ~/.memryzed/audit.log  |
                  +-------------------------+

The client may spawn multiple `memryzed serve` processes if the user
opens multiple instances. Each one connects to the same database and
operates independently.

## Crate layout

The Cargo workspace is organized into four crates:

    crates/
      memryzed-core/    Library. Storage, retrieval, sessions,
                        extractor, project identity, scopes.
                        No I/O outside of the storage layer.
      memryzed-mcp/     Library. MCP tool implementations on top of
                        memryzed-core. Translates MCP requests to
                        core calls and back.
      memryzed-cli/     Binary. The `memryzed` executable. Includes
                        the CLI command tree, the `serve` entry point
                        that wraps memryzed-mcp, and embeds the TUI.
      memryzed-tui/     Library. The ratatui review interface. Used
                        by `memryzed review` and any future
                        interactive surfaces.

The single shipped binary is `memryzed`, produced by `memryzed-cli`.
Splitting into crates keeps boundaries clear and makes the future
cloud server straightforward to build by reusing `memryzed-core` and
`memryzed-mcp` behind a different transport.

## Module breakdown inside `memryzed-core`

    storage/        Schema, migrations, SQLite access, sqlite-vec
                    integration, FTS5 setup. The only module that
                    touches the database. Returns plain Rust types
                    upstream.

    memory/         CRUD on memories. Validation, scope resolution,
                    confidence handling, status transitions.

    retrieval/      Hybrid search. Embedding lookup, FTS query, score
                    fusion, deduplication, recency boost. Returns
                    ranked results.

    embedder/       Wraps fastembed-rs. Loads and caches the
                    embedding model. Provides a synchronous
                    `embed(text) -> Vec<f32>` interface.

    sessions/       Session creation, checkpoint, resume, listing,
                    archiving. The state blob is opaque to this
                    module; only metadata is interpreted.

    projects/       Project identity. Computes the project ID from
                    the working directory: git remote SHA-256 first,
                    absolute path hash as fallback. Maintains the
                    `projects` table.

    extractor/      The fact-proposal pipeline. Two implementations:
                    rule-based (regex patterns) and Ollama-based
                    (HTTP). Both produce candidate memories that go
                    to the pending queue.

    audit/          Append-only log writer. Every recall, remember,
                    forget, and session change writes a structured
                    line to `audit.log`.

    config/         Loads `config.toml`, applies defaults, layers in
                    environment variables. Exposes a typed Config
                    struct.

    integration/    Detection and writing of MCP client config files.
                    One adapter per supported client.

    update/         GitHub Releases check, version comparison,
                    self-update via `self_update` crate.

    error/          Crate-wide error types. Maps to MCP error codes
                    in the MCP layer and to exit codes in the CLI
                    layer.

## The retrieval pipeline

When an MCP client calls `recall(query, scope)`:

1. The MCP layer parses and validates the request.
2. The retrieval module computes the query embedding through the
   embedder.
3. The retrieval module issues two SQL queries against storage:
   - A vector-similarity query against `memory_vectors` filtered to
     the requested scope.
   - A BM25 query against `memory_fts` filtered to the requested
     scope.
4. Results are merged using a weighted fusion of normalized scores
   (vector similarity, BM25, and a recency boost based on
   `created_at`).
5. The top N (capped by `limit` or `[retrieval] max_results`) are
   joined back to `memories` to fetch full content and metadata.
6. Results are wrapped with a `summary` line and returned.
7. The audit module logs the query and the IDs returned.

The pipeline runs synchronously. The query latency budget is under
50 ms p99 on a database with 10,000 memories.

## The extraction pipeline

The extractor runs in the MCP server process as a background task,
not on the request path. When a turn arrives:

1. The agent's request is observed (in v1, only via explicit
   `remember` calls or via the rule-based pattern matcher running
   over user-supplied content).
2. Candidate facts are produced. Each carries a confidence score.
3. Each candidate is written to the `memories` table with status
   `pending`.
4. If `confidence >= [memory] auto_approve_threshold`, the status is
   transitioned to `approved` immediately and the audit log records
   it. Otherwise, it remains pending until the user reviews.

The Ollama-based extractor (when enabled) runs the same way; it just
produces richer candidates with potentially lower confidence scores.

The extractor never blocks the agent's response. If extraction fails
(for example, Ollama is down), the failure is logged and the
conversation continues unaffected.

## Project identity

A project is a stable identifier for a working directory across
machines and across time. The algorithm:

1. Run `git config --get remote.origin.url` in the working directory.
2. If a remote URL is found, normalize it (strip credentials, lowercase
   the host, remove a trailing `.git`), then take SHA-256 truncated to
   12 hex chars. Prefix with `proj_`.
3. If no remote is found, hash the absolute path of the working
   directory and prefix with `proj_local_`.

Each project row stores:

    id              The computed identifier.
    git_remote      The original (normalized) remote URL, if any.
    local_paths     A JSON array of every absolute path the project
                    has been seen at. New paths are appended on each
                    serve start, so the same repo cloned to two
                    machines is unified by its remote.
    display_name    Human-readable name. Defaults to the basename of
                    the working directory.
    created_at      First time this project was seen.
    last_seen_at    Most recent time.

## Trust surfaces

Memryzed exposes three transparency surfaces, listed by reliability:

1. The audit log at `~/.memryzed/audit.log`. Always written. Read with
   `memryzed log` or `memryzed log -f`. Structured one-line-per-event,
   suitable for piping to `grep` and `jq`.
2. The MCP tool-call display in the client. Free; depends on the
   client. Most modern MCP clients show tool calls and their
   arguments and responses.
3. The `summary` field returned in every tool response. Best-effort;
   relies on the agent rendering it in its output.

The audit log is the only authoritative trust surface. The other two
are conveniences.

## Concurrency and locking

SQLite supports many concurrent readers but only one writer at a
time. We use:

- WAL journal mode for better concurrency.
- A process-local async mutex around the write path so concurrent
  tasks within `serve` serialize cleanly without spinning on
  `SQLITE_BUSY`.
- Short transactions. A single tool call is one transaction. Long
  background tasks (extraction, embedding regeneration) batch their
  writes.

When two Memryzed processes (for example, `serve` and an interactive
CLI) write at the same time, SQLite's busy timeout (set to five
seconds) handles brief contention. Operations that exceed it return
`storage_error` and are surfaced to the user.

## Error handling

`memryzed-core` uses a single error enum that wraps:

    Storage(sqlite, integrity, migration)
    Embedder(model_load, model_run)
    Extractor(parse, ollama)
    Integration(io, parse, unsupported_client)
    Config(parse, validation)
    Update(network, signature, write)
    Validation(invalid_argument, invalid_scope)
    NotFound(kind, id)

Each variant maps cleanly to:

- An MCP error code on the MCP boundary.
- An exit code on the CLI boundary.
- A user-visible message in the TUI.

## Telemetry

Off by default. When opted in, the client periodically emits a small
counter payload:

    {
      "version": "0.1.0",
      "os": "linux",
      "arch": "x86_64",
      "memory_count": 47,
      "session_count": 12,
      "client_seen": ["claude-code", "kiro"],
      "uptime_seconds": 3600
    }

No content. No queries. No file paths. The endpoint is documented at
`https://memryzed.com/telemetry` and the exact wire format is in the
source code under `memryzed-core/src/telemetry.rs`. Users can verify
by running `memryzed log` and watching for `telemetry_emit` events.

## Update flow

On `serve` startup, if `[updates] check_on_startup` is true and the
last check was more than 24 hours ago:

1. A background task fetches `https://api.github.com/repos/memryzed/memryzed/releases/latest`.
2. Compares the `tag_name` to the running version.
3. If newer, prints a notice on stderr and writes a `update_available`
   line to the audit log.
4. Never installs automatically (regardless of `auto_install` setting
   in v1).

`memryzed update` is the explicit upgrade command. It downloads the
appropriate binary for the platform, verifies its checksum, atomically
swaps the binary in place, and prints the new version.

## Future-proofing notes

The architecture is designed so that the future paid cloud sync can
be added without breaking changes:

- The schema is the same locally and remotely; sync is row-level
  copying with timestamp-based conflict resolution.
- All sensitive fields can be wrapped in a per-user encryption layer
  added at the storage boundary; the cloud server stores ciphertext.
- The MCP surface stays identical; cloud features are opt-in via
  configuration (a future `[cloud]` section).
- The crate split keeps `memryzed-core` and `memryzed-mcp` reusable
  for a server that exposes the same tools over a different
  transport.

For the full v1 design, see `docs/specs/v1.md`.
