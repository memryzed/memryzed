# Changelog

All notable changes to Memryzed are documented in this file.

The format is based on Keep a Changelog (https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning (https://semver.org/spec/v2.0.0.html).

For the conventions used to write entries in this file, see
`docs/development/changelog-conventions.md`.

## [Unreleased]

### Added

- Recall context expansion. A recalled conversation turn is now
  returned together with its neighbouring turns from the same
  conversation, so each hit reads coherently and answers that live in
  an adjacent turn are captured. Every recall hit carries a rendered
  excerpt with the matched line marked.
- Chronological recall. `recall` accepts `order: "recent"` to return
  the latest conversations by true time, answering "what did we last
  discuss", which similarity ranking cannot.
- Lexical rerank leg in episode recall: a model-free boost for hits
  whose exact query terms appear, alongside the vector and full-text
  legs. Hybrid weights are now named constants tuned in one place.
- Self-describing recall results: the response summary names the
  source agents and whether results are relevance- or recency-ordered.
- Background capture-and-index engine. The MCP server (`memryzed
  serve`, which agents spawn automatically) now runs a background
  loop that captures new conversation from every detected agent and
  embeds it, with no user commands required beyond install. Capture
  is text-only and therefore instant; embeddings are filled in
  lazily on a background thread, so nothing ever blocks. `memryzed
  init` returns in well under a second even on a machine with a
  large agent history, and that history is imported and embedded in
  the background while the agent runs.
- `episodes::insert_batch_text_only` for instant capture and
  `episodes::reindex_pending` for resumable, interruptible
  background embedding (embeds episodes whose vector is missing or
  was produced by a different model).
- Episodic memory: verbatim conversation turns for cross-agent
  continuity. Mining now captures each substantive turn from a
  transcript as an embedded `episode`, stored in a new `episodes`
  table (migration 005) with its own FTS index. The `recall` MCP
  tool searches episodes alongside curated memories, so a
  conversation held in one agent (for example Kiro) can be recalled
  from another (for example Claude Code). No LLM is required: the
  bundled embedding model does the work and the calling agent does
  the understanding. Trivial turns (under 24 characters) are skipped.
- Batched embedding for capture. All substantive turns in a
  transcript are embedded in a single call, making `mine` and
  `watch` an order of magnitude faster than per-turn embedding.
- Universal cross-agent auto-capture. `memryzed watch` polls every
  detected agent transcript directory on an interval and mines new
  turns incrementally, capturing memories from Kiro CLI, Claude Code,
  and Copilot CLI without per-agent hooks. `memryzed mine --all`
  runs one pass across every detected agent.
- Copilot CLI transcript adapter (`user.message` / `assistant.message`
  lines with a `data.content` string) under
  `~/.copilot/session-state/`.
- A source registry (`Source::all`, `Source::default_dir`) mapping
  each supported agent to its standard transcript directory, so new
  agents are added in one place.
- Incremental mining mode: a per-file byte offset is tracked in the
  `meta` table so a growing live transcript yields only its newly
  appended turns on each pass. Truncated or rotated files reset to
  the start.

### Changed

(none)

### Deprecated

(none)

### Removed

(none)

### Fixed

(none)

### Security

(none)

## [0.5.0] - 2026-05-30

### Added

- Transcript mining. `memryzed mine <path>` ingests existing agent
  conversation transcripts into Memryzed: each transcript becomes a
  session record and its user turns are run through the extractor to
  propose candidate memories. Supports Kiro CLI session JSONL
  (`~/.kiro/sessions`) and Claude Code session JSONL
  (`~/.claude/projects`), with `--source auto|kiro|claude-code`,
  `--dry-run`, and `--force`. Idempotent across runs via a content
  hash recorded in the `meta` table.
- Claude Code auto-save hooks. `memryzed hooks install` generates two
  hook scripts (a periodic checkpoint on the Stop event and a
  pre-compaction hook) under `~/.memryzed/hooks/` and wires them into
  `~/.claude/settings.json`, preserving any existing hooks and
  settings. `memryzed hooks uninstall` removes only Memryzed's
  entries. The scripts mine the active transcript so recent turns
  become memories without the user asking.
- Quality benchmark harness (`benchmarks/`, the `memryzed-bench`
  binary). Loads a normalized dataset, stores every document in a
  fresh in-memory store, runs each question through hybrid retrieval,
  and reports recall at K as JSON. Datasets are not committed; the
  harness reads a normalized JSON format documented in
  `benchmarks/README.md`. Methodology and honesty rules are in
  `docs/specs/benchmarks.md`.
- `Database::meta_get` and `Database::meta_set` for reading and
  writing the `meta` key-value table, used by mining for idempotency.

## [0.4.0] - 2026-05-29

### Added

- `memryzed update` and `memryzed update --check`. Queries the
  GitHub Releases API for the latest tag and compares it to the
  running version. The check is non-fatal: network failures, a
  missing repository, and parse errors all collapse into an
  `Unknown` status so the command exits zero even when offline.
  The binary swap itself is owned by the install script.
- Optional Ollama-based extractor
  (`extractor/ollama.rs`). When enabled it proposes richer
  candidate memories from recent conversation turns through a local
  Ollama model, falling back to the rule-based extractor when
  Ollama is unreachable. Off by default.
- `integrations::is_configured`, a non-failing check for whether a
  client's MCP config already contains the Memryzed server entry.

### Fixed

- `memryzed doctor` now reports one row per known MCP client
  (installed and configured, installed but not configured, or not
  installed) instead of a stale placeholder line. Reads each
  client's config defensively so a malformed user file never fails
  the health report.

## [0.3.0] - 2026-05-29

### Added

- Rule-based extractor (`extractor` core module). Scans a user
  message for high-signal patterns and proposes candidate memories
  with a confidence score: explicit "remember that ..." and
  "don't forget ..." (confidence 1.0), "I prefer X over Y" (0.95),
  "I always/usually/never use X" (0.9), "this repo uses X" and
  "the deploy/build/test/lint command is X" (0.9, project scope).
- Pending queue. New `memory::insert_pending` stores a memory in
  `pending` status without an embedding; `memory::approve`
  transitions it to approved or pinned and writes the embedding in
  one transaction; `memory::list_pending` lists the queue.
- New `extract_from` MCP tool. Runs the extractor over a message,
  auto-approves candidates at or above the configured threshold
  (default 0.85), and queues the rest for review. Brings the MCP
  surface to nine tools.
- Review TUI (`memryzed-tui`, built on ratatui and crossterm) and
  the `memryzed review` command. Walks the pending queue with
  approve (a), approve and pin (p), reject (r), and navigation
  keys; shows per-candidate detail and a running status line.

### Changed

- The MCP server now exposes nine tools (added `extract_from`).

### Added

- Sessions: per-task working state scoped to a project. Migration
  004 adds the `sessions` table. New `sessions` core module with
  `checkpoint` (create or update the active session), `resume_latest`
  (most recent resumable session for a project), `get_by_id`, `list`,
  and `end` (mark completed).
- Four new MCP tools: `checkpoint`, `resume`, `list_sessions`, and
  `end_session`. The server resolves the project for its working
  directory once at startup, so session tools need no scope argument.
  `resume` with no id returns the most recent session; with an id it
  returns that specific one.
- Three new CLI commands: `memryzed sessions [--limit N]`,
  `memryzed resume [<id>] [--json]`, and `memryzed end-session <id>`.

### Changed

- Server tool count is now eight. The handshake instructions list
  both the memory tools and the session tools.

First non-prerelease. Accumulates everything built across the
0.1.0-alpha and 0.1.0-rc series into one release: a local MCP
server with memory storage, embeddings, hybrid retrieval, client
auto-install, audit log, configuration, and data export/import,
plus the release pipeline and install scripts.

### Added

- Initial project documentation set, including the v1 specification, user
  guide, configuration reference, MCP tool reference, agent author guide,
  and operations documentation.
- Quality benchmarks plan at `docs/specs/benchmarks.md`. Defines the
  datasets (LongMemEval, LoCoMo, ConvoMem, MemBench), the metrics
  reported (recall at K, recall by category, latency at percentiles,
  index size), the honesty principles for publication, and the
  conditions under which numbers are comparable to other tools.
- Cargo workspace scaffold with four member crates: `memryzed-core`,
  `memryzed-mcp`, `memryzed-cli`, `memryzed-tui`. The `memryzed`
  binary builds and prints `0.1.0-alpha.1` for `--version`.
- CLI command tree wired with clap for every subcommand documented in
  `docs/cli-reference.md`. Subcommands not yet implemented in this
  alpha return a clear "not yet implemented" error so users discover
  the surface through `--help`.
- `memryzed init` creates the data directory, the `bin/` subdirectory,
  and a default `config.toml` from an embedded asset. Idempotent on
  re-run.
- `memryzed doctor` runs sectioned checks (Installation, Data, Memory
  and integrations). Each check prints `[ok  ]`, `[fail]`, or `[skip]`,
  with skipped checks naming the alpha or beta version where the
  capability lands. Exits non-zero on any failure.
- GitHub Actions CI workflow at `.github/workflows/ci.yml` running
  `cargo fmt --check`, `cargo clippy --workspace --all-targets
  -Dwarnings`, and `cargo test` on Linux, macOS, and Windows.
- Unit tests in `memryzed-core` for the version constant and the
  `DataDir` paths layout. Integration tests in `memryzed-cli` covering
  `--version`, `--help`, `init`, `doctor`, and the unimplemented-
  subcommand error path. Eight CLI plus six unit tests pass cleanly.
- v0.1.0-alpha.2 storage layer: SQLite via `rusqlite` (bundled),
  embedded SQL migration system tracked through `PRAGMA user_version`,
  and migration `001_initial.sql` creating the `memories`, `projects`,
  `recall_log`, and `meta` tables with check constraints and indexes.
  WAL journaling, foreign keys, and a 5 second busy timeout are
  configured at open time.
- Domain types `Scope`, `Kind`, `Status` with database-string mapping
  and `FromStr` validation. Stable identifier helpers
  (`mem_<12hex>`, `sess_<12hex>`, deterministic `proj_*` and
  `proj_local_*`).
- Project identity computation from the current working directory.
  When a git remote is present (`git config --get remote.origin.url`),
  it is normalized (credentials stripped, trailing `.git` removed,
  host lowercased) and hashed; otherwise the absolute path is hashed.
- `Memory` and `Project` records with CRUD: `insert`, `get_by_id`,
  `list` with scope, status, and limit filters, `archive`, and hard
  `delete`. Validation rejects empty content, missing scope IDs for
  non-global memories, and confidence values outside `[0.0, 1.0]`.
- `memryzed init` now creates and migrates the SQLite database in
  addition to the data directory and configuration. `memryzed doctor`
  opens the database, runs `PRAGMA integrity_check`, and reports the
  schema version.
- New CLI commands operating on the database: `memryzed remember
  <text> --scope ...`, `memryzed list [--scope|--project|--status]`,
  `memryzed show <id>`, `memryzed forget <id> [--hard]`.
- 49 core unit tests and 14 CLI integration tests covering every new
  surface, including pinned-first ordering, scope filtering, status
  defaults, archive/delete semantics, and validation errors.
- v0.1.0-alpha.3 embedder: `fastembed-rs` integration with the
  BGE-small-en-v1.5 model (384-dim output). New `embedder` module
  with the `Embedder` trait, `FastembedEmbedder`, and a `NoopEmbedder`
  used by tests and when `MEMRYZED_DISABLE_EMBEDDING` is set.
- Migration 002 adds the `memory_embeddings` table (`memory_id` PK
  with FK + cascade delete on `memories.id`, plus `model`, `dim`,
  and a little-endian `f32` BLOB column). New
  `memory::insert_with_embedder` runs the memory insert and the
  embedding insert in a single transaction; `memory::get_embedding`
  reads the row back.
- `memryzed init` now creates `~/.memryzed/models/` and warm-loads
  the embedder, downloading the model on first run. The download
  step is skipped when `MEMRYZED_DISABLE_EMBEDDING` is set.
- `memryzed remember` now embeds the content as part of the same
  transaction that inserts the memory, and reports the model id and
  dimension in its output.
- `memryzed doctor` adds a real Embedding model check that loads
  the embedder and reports the model id and dimension; reports a
  skip with the environment-variable name when disabled.
- Cross-platform support: a small Linux-only build script compiles
  `c/glibc_compat.c` with weakly linked `__isoc23_strto*` shims so
  the prebuilt ONNX Runtime links cleanly on glibc < 2.38. On
  newer systems the strong glibc symbols take precedence and the
  shim is unused.
- v0.1.0-alpha.4 hybrid retrieval. Migration 003 adds the FTS5
  virtual table `memory_fts` and triggers that mirror inserts,
  updates, and deletes on `memories.content`. New `retrieval`
  module computes a hybrid score from cosine similarity over the
  stored embeddings, normalized BM25, and a recency boost
  (`exp(-age_days / 30)`), with a small additive bonus for pinned
  memories. The CLI gains `memryzed search <query>` printing
  ranked results with the per-signal score breakdown.
- v0.1.0-alpha.5 MCP server. New `memryzed-mcp` crate implements
  `MemryzedServer` with four MCP tools — `recall`, `remember`,
  `forget`, `list_memories` — built on `rmcp` 1.7 with
  `#[tool_router]` and `#[tool_handler]`. State is an
  `Arc<Inner>` carrying the database under a `tokio::sync::Mutex`
  and the embedder behind an `Arc<dyn Embedder>`. The server
  declares protocol version `2024-11-05`, exposes JSON Schemas
  generated by `schemars` for every argument struct, and maps
  core errors (`NotFound`, `Validation`, `Storage`, ...) to MCP
  error codes.
- New `memryzed serve` subcommand spins up a current-thread tokio
  runtime and runs the server over stdio via
  `rmcp::ServiceExt::serve(stdio())`. Auto-creates the data
  directory when invoked before `memryzed init` so the first call
  from an MCP client never fails noisily.
- 6 new MCP tool tests cover round-trip remember + list, archive
  via forget, empty-query rejection, unknown-id error, and invalid
  scope handling.
- v0.1.0-beta.1 client integration. New `integrations` module with
  adapters for Claude Code, Kiro CLI, Cursor, Codex CLI, and
  Continue. `memryzed install` auto-detects which clients are
  present and writes the Memryzed MCP entry into each, backing up
  the existing config to `<file>.memryzed.bak` first. `--client`
  targets one client, `--print` emits the config block without
  writing. `memryzed uninstall` removes the entry with `--unwire`
  and deletes the data directory with `--purge`.
- v0.1.0-beta.1 audit log. New `audit` module writes append-only
  JSONL to `~/.memryzed/audit.log`. `memryzed log` prints recent
  entries, `--follow` streams new ones, `--tail N` and `--client`
  filter the output.
- v0.1.0-beta.1 configuration commands. `memryzed config` prints
  the active config; `config get`, `config set`, and `config edit`
  read, write (with type coercion), and open the TOML in `$EDITOR`.
- v0.1.0-beta.1 data portability. New `export` module produces
  versioned JSON (schema version 1). `memryzed export` writes it to
  stdout (`--pretty` for indented output); `memryzed import`
  restores it with last-write-wins conflict resolution, `--dry-run`
  to preview, and `--yes` to confirm. Embeddings are regenerated on
  import rather than stored in the file.
- Corrected the documented Kiro CLI MCP config path to
  `~/.kiro/settings/mcp.json`.
- v0.1.0-rc.1 release engineering. Added cargo-dist configuration
  under `[workspace.metadata.dist]` in the root manifest (targets,
  installers, checksum, install path) and a `dist` build profile.
  Added `.github/workflows/release.yml` that builds every supported
  target on a version-tag push, packages each with a SHA-256
  sidecar, and publishes a GitHub Release. Added the canonical
  install scripts under `dist/` (`install.sh`, `install.ps1`,
  `install.cmd`) that download release archives, verify checksums,
  install to the standard location, and update PATH. These are the
  files served from memryzed.com.

### Changed

- Roadmap v1.x section rewritten to call out three concrete priorities:
  transcript mining for existing Claude Code, Codex, and Cursor
  histories; auto-save hooks for Claude Code; and the first published
  quality benchmark numbers. Multilingual remains deferred to the
  Later section, with reasoning for the deferral made explicit.
- Section 18 of the v1 specification now distinguishes performance
  latency targets from retrieval-quality benchmarks, with the latter
  governed by the new benchmarks specification document.
- Workspace version bumped to `0.1.0-alpha.3`. The CLI's long help
  text reflects that storage, embeddings, and basic memory commands
  now work.

### Deprecated

(none)

### Removed

(none)

### Fixed

(none)

### Security

(none)

---

## How to read this file

Each released version has its own section, ordered newest first. Within a
version, changes are grouped under the following headings:

- Added: new features or capabilities.
- Changed: changes to existing behavior.
- Deprecated: features that will be removed in a future release.
- Removed: features that have been removed.
- Fixed: bug fixes.
- Security: security-relevant changes.

Each entry is a short, user-visible description of the change. Internal
refactoring that does not affect users is not listed here.

The "Unreleased" section at the top accumulates entries between releases.
On release, the section is renamed to the new version, the date is added,
and a fresh empty "Unreleased" section is created.

[Unreleased]: https://github.com/memryzed/memryzed/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/memryzed/memryzed/releases/tag/v0.1.0
