# Development setup

This document describes how to get a working Memryzed development
environment from a clean machine.

## Prerequisites

- Rust stable, current. We follow Rust stable; the minimum supported
  version is documented in the workspace `Cargo.toml`.
- A C toolchain. Required for building some native dependencies.
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Linux: `gcc`, `pkg-config`, and the SQLite development headers
    (`libsqlite3-dev` on Debian/Ubuntu, `sqlite-devel` on Fedora).
  - Windows: Visual Studio Build Tools with the C++ workload.
- `git`.
- `cargo-dist`, installed via `cargo install cargo-dist`. Only needed
  for release work.
- `cargo-nextest`, recommended for fast test runs:
  `cargo install cargo-nextest`.

The full required toolchain matches what GitHub Actions uses for CI.
Anything that builds in CI should build on a developer machine with
the same prerequisites installed.

## Cloning

    git clone https://github.com/memryzed/memryzed.git
    cd memryzed

## First build

    cargo build

This compiles all crates in the workspace. The first build downloads
dependencies, which takes a few minutes.

To build the release binary:

    cargo build --release

The binary lands at `target/release/memryzed`.

## Running locally

You can run a development build of the CLI and server directly:

    cargo run -- --version
    cargo run -- init
    cargo run -- doctor

To use the development binary as the MCP server in your real agent
configuration, point your MCP client config at the absolute path of
the built binary:

    {
      "mcpServers": {
        "memryzed-dev": {
          "command": "/abs/path/to/memryzed/target/debug/memryzed",
          "args": ["serve"]
        }
      }
    }

We recommend using a separate name like `memryzed-dev` so the
development binary does not collide with an installed release.

## Coding conventions

- Format with `cargo fmt`. CI fails on unformatted code.
- Lint with `cargo clippy --workspace --all-targets`. CI fails on
  clippy warnings on the default profile.
- Names: snake_case for functions and variables, CamelCase for
  types, SCREAMING_SNAKE_CASE for constants.
- Errors: prefer `thiserror`-derived enums and propagate with `?`.
  No `unwrap` or `expect` in non-test code unless the failure is a
  programmer error that should panic.
- Logging: use `tracing` everywhere. Spans for boundaries, events
  inside.
- Async: `tokio`. Avoid blocking the executor; use `spawn_blocking`
  for blocking I/O if necessary.
- SQL: parameterized statements always. No string concatenation
  into SQL. Reviewers reject any direct interpolation of user input.

## Project layout

    Cargo.toml             Workspace root.
    crates/
      memryzed-core/       Library: storage, retrieval, sessions.
      memryzed-mcp/        Library: MCP tool layer.
      memryzed-cli/        Binary: the memryzed executable.
      memryzed-tui/        Library: ratatui interface.
    docs/                  Documentation.
    examples/              Reference MCP configurations and integration samples.
    scripts/               Local helper scripts.
    .github/
      workflows/           CI definitions.
    Cargo.lock             Locked dependencies.

The full architecture is in `../architecture.md`. The on-disk format
is in `../data-model.md`. The v1 specification is in `../specs/v1.md`.

## Running tests

    cargo nextest run                          # full test suite
    cargo nextest run -p memryzed-core         # one crate
    cargo nextest run --no-fail-fast           # keep going after failures

For coverage:

    cargo install cargo-llvm-cov
    cargo llvm-cov --workspace --html

Coverage reports land at `target/llvm-cov/html/index.html`. Coverage
is informational; we do not gate on a coverage threshold.

## Useful commands during development

    cargo check                             Fast type-check across the workspace.
    cargo build -p memryzed-cli --bin memryzed
    cargo run --bin memryzed -- doctor
    cargo doc --workspace --no-deps --open
    cargo udeps                             Find unused deps (requires nightly).

## Working with the database

A development database is created under `target/dev-data/` when you
run `cargo run -- init` from the workspace root. To inspect:

    sqlite3 target/dev-data/db.sqlite

Common queries:

    SELECT id, scope_kind, content FROM memories ORDER BY created_at DESC LIMIT 20;
    SELECT * FROM projects;
    SELECT * FROM sessions ORDER BY updated_at DESC LIMIT 5;
    PRAGMA user_version;

To reset the dev database, delete `target/dev-data/`.

## Debugging the MCP server

To trace what your client sends and what we respond:

    MEMRYZED_LOG_LEVEL=debug cargo run --bin memryzed -- serve

Then connect from your client. The debug log includes tool calls,
SQL queries above a threshold, and audit events.

For interactive testing without a client, you can pipe MCP messages
manually using a small helper script. The `examples/mcp-test.sh`
script does this end to end against a development build.

## Working on the install scripts

The install scripts live in a separate website repository so they
can be updated independently of release tags. Locally, you can
preview the generated install scripts after a `cargo dist build`:

    cargo dist build --installer shell --installer powershell

The output goes to `target/distrib/`.

## Working on documentation

Documentation is plain Markdown. There is no static site generator
in v1; the docs render correctly on GitHub.

Run a link check before pushing if you have changed cross-references:

    cargo install lychee
    lychee README.md docs/**/*.md

## Common pitfalls

- Forgetting to run `cargo fmt`. Set up your editor to run it on
  save.
- Adding a dependency that pulls a copyleft transitive dependency.
  Run `cargo deny check` after every dependency change.
- Writing SQL that does not use the schema's indexes. Check with
  `EXPLAIN QUERY PLAN` against a populated dev database.
- Returning `unwrap` errors to the user. Use the typed error enum.
- Making a backward-incompatible schema change without a migration.
  See `../data-model.md` for the migration system.
