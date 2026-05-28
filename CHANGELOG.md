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

### Changed

- Roadmap v1.x section rewritten to call out three concrete priorities:
  transcript mining for existing Claude Code, Codex, and Cursor
  histories; auto-save hooks for Claude Code; and the first published
  quality benchmark numbers. Multilingual remains deferred to the
  Later section, with reasoning for the deferral made explicit.
- Section 18 of the v1 specification now distinguishes performance
  latency targets from retrieval-quality benchmarks, with the latter
  governed by the new benchmarks specification document.
- Workspace version bumped to `0.1.0-alpha.2`. The CLI's long help
  text reflects that storage and basic memory commands now work.

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
