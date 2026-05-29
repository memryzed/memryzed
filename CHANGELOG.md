# Changelog

All notable changes to Memryzed are documented in this file.

The format is based on Keep a Changelog (https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning (https://semver.org/spec/v2.0.0.html).

For the conventions used to write entries in this file, see
`docs/development/changelog-conventions.md`.

## [Unreleased]

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

[Unreleased]: https://github.com/memryzed/memryzed/compare/main...HEAD
