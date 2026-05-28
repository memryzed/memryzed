# Testing

This document describes how Memryzed is tested: what kinds of tests
we have, where they live, what each one is responsible for, and how
to run them.

## Layers of testing

Memryzed has four layers of tests, in order of cost and signal:

1. Unit tests. Inside each crate, alongside the code. Test single
   functions and modules in isolation. Fast.
2. Integration tests. In each crate's `tests/` directory. Exercise
   public APIs against a real SQLite database in a temporary
   directory. Moderate cost.
3. End-to-end tests. In `crates/memryzed-cli/tests/`. Spawn the
   `memryzed` binary as a subprocess and exercise it through stdio
   (for `serve`) or as a CLI. Slow.
4. Smoke tests. A small set of platform-specific tests that run on
   every supported target in CI to catch packaging issues.

The unit and integration tests are the bulk of the suite. End-to-end
tests cover the integration layer between crates and the actual MCP
transport.

## Test organization

    crates/memryzed-core/
      src/                    Production code with `#[cfg(test)]` units.
      tests/                  Integration tests for the public API.
    crates/memryzed-mcp/
      src/                    Tool implementations with units.
      tests/                  Tool-level integration tests.
    crates/memryzed-cli/
      src/                    Binary code.
      tests/                  End-to-end tests spawning the binary.
    crates/memryzed-tui/
      src/                    TUI code with units (test view rendering).

## What each layer tests

### Unit tests

Inside `src/` next to the code. Examples of what belongs here:

- Pure functions: hash a remote URL, compute a recency boost, parse
  a regex pattern.
- Module-level invariants: a `Config` struct round-trips through
  TOML.
- Edge cases of small algorithms.

Avoid touching the database from unit tests. If a unit needs the
database, it probably belongs in integration tests.

### Integration tests

In each crate's `tests/` directory. Each test creates a temporary
data directory, runs migrations, exercises some public surface, and
asserts on the result. Examples:

- `memory::create_and_recall` round trip.
- `extractor::ollama_unreachable_falls_back`.
- `sessions::resume_returns_most_recent`.
- `migrations::migrate_from_v1_to_v3`.

A shared test helper crate, `crates/memryzed-test-utils`, provides:

- A `TempDataDir` RAII guard that creates and cleans up a data dir.
- A `seed_memories` helper for populating fixtures.
- A `mock_clock` for deterministic timestamps.
- A `mock_embedder` for deterministic embeddings, so retrieval tests
  do not depend on the real model.

### End-to-end tests

In `crates/memryzed-cli/tests/`. Each test spawns the built binary
and asserts on stdout, stderr, exit code, or the resulting database.

Examples:

- `cli_init_creates_directory` runs `memryzed init` in a clean
  TempDir and checks the layout.
- `cli_install_updates_config` runs `memryzed install` against a
  fake MCP client config and asserts on the rewritten file.
- `cli_serve_responds_to_recall` spawns `memryzed serve` and writes
  a recall request over stdin, then reads the response.

These tests use the `assert_cmd` and `predicates` crates.

### Smoke tests

A handful of tests gated by feature flags or environment variables
that are cheap to run on every CI target. Examples:

- The binary starts and prints `--version`.
- Migrations run cleanly on an empty database.
- The embedding model loads successfully (only on targets that have
  a cached model).

## Test data

The `tests/fixtures/` directory of `memryzed-core` holds:

- Sample conversation turns for extractor tests.
- Sample export files for import tests.
- Snapshots of the database schema at each historical version, for
  migration tests.

Fixtures are committed to the repository. They should be small
(typically under 10 KB each).

## Running tests

We use `cargo-nextest` for faster runs:

    cargo nextest run                          Full suite.
    cargo nextest run -p memryzed-core         One crate.
    cargo nextest run -E 'test(retrieval)'     Filter by name.
    cargo nextest run --no-fail-fast           Continue past failures.

Plain `cargo test` works too but is slower.

## Determinism

Tests must be deterministic. To enforce this:

- Never depend on wall-clock time. Use the `mock_clock` helper.
- Never depend on the real embedding model in unit or integration
  tests. Use the deterministic `mock_embedder`.
- Use `tempfile::TempDir` for any filesystem state. Never hard-code
  paths under `/tmp` or the home directory.
- Avoid network calls. The few tests that need network are gated
  behind a `network` feature flag and do not run in CI by default.

## Snapshot tests

For tests that assert on richer output (CLI help text, JSON
responses, formatted CLI output), we use `insta` snapshot testing.
Snapshots live under `tests/snapshots/` next to the test files.

To update snapshots after an intentional change:

    cargo install cargo-insta
    cargo insta review

Review every snapshot change before accepting it. Snapshot diffs
that look fine often hide real regressions.

## Performance benchmarks

A small set of benchmarks lives under `crates/memryzed-core/benches/`.
They use the `criterion` framework and target the operations called
out in `docs/specs/v1.md` section 18.

Run benchmarks locally with:

    cargo bench -p memryzed-core

CI runs benchmarks on a fixed runner and uploads the results to a
build artifact. Regressions over a configured threshold fail the
build.

## What CI runs

On every pull request:

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -D warnings`.
- `cargo nextest run --workspace`.
- `cargo deny check`.
- `cargo audit`.
- A small smoke test on macOS, Linux, and Windows.

On the `main` branch:

- The above, plus end-to-end tests on all platforms.
- Benchmark runs with regression checking.

On tag push (a release candidate):

- All of the above.
- Cross-compiled builds for every target listed in `docs/specs/v1.md`
  section 5.2.
- Installation tests using the generated install scripts on a
  matrix of fresh runner images.

## Contributing tests

When you fix a bug, add a test that fails before your fix and passes
after. When you add a feature, add tests that cover the happy path
and at least one failure mode.

When you add a public API, add an integration test that exercises
it the way a caller would.

When you change behavior intentionally, update the snapshots in the
same commit, with a CHANGELOG entry that explains what changed.
