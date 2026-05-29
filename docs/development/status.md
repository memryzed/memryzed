# Project status

This document is the current snapshot of where Memryzed stands. It is
the first thing a contributor or a returning maintainer should read
to pick up cleanly. Update it whenever a milestone ships or a major
decision changes.

## Version

`0.1.0-beta.1` (local git only; nothing has been pushed or tagged
publicly yet).

## Where the code lives

- Local working tree: `/mnt/c/Users/HamzaArjah/Documents/Hamza/memryzed`
- Local git branch: `main`
- Remote: not yet created. The intended remote is
  `github.com/memryzed/memryzed`.
- Release binary: `~/.memryzed/bin/memryzed`

## Capability matrix

The current binary supports the operations marked done. Everything
else is planned but not implemented.

### CLI

| Command                       | Status | Landed in |
|-------------------------------|:------:|-----------|
| `memryzed --version`          | done   | alpha.1   |
| `memryzed --help`             | done   | alpha.1   |
| `memryzed init`               | done   | alpha.1+ (DB in alpha.2, models in alpha.3) |
| `memryzed doctor`             | done   | alpha.1+  |
| `memryzed remember <text>`    | done   | alpha.2 (with embedding from alpha.3) |
| `memryzed list`               | done   | alpha.2   |
| `memryzed show <id>`          | done   | alpha.2   |
| `memryzed forget <id>`        | done   | alpha.2   |
| `memryzed search <query>`     | done   | alpha.4   |
| `memryzed serve` (MCP stdio)  | done   | alpha.5   |
| `memryzed install`            | done   | beta.1    |
| `memryzed uninstall`          | done   | beta.1    |
| `memryzed update`             | todo   | later     |
| `memryzed review` (TUI)       | todo   | 0.3.0     |
| `memryzed sessions`           | todo   | 0.2.0     |
| `memryzed resume`             | todo   | 0.2.0     |
| `memryzed log`                | done   | beta.1    |
| `memryzed config`             | done   | beta.1    |
| `memryzed export` / `import`  | done   | beta.1    |

### MCP tools (over stdio)

| Tool             | Status | Landed in |
|------------------|:------:|-----------|
| `recall`         | done   | alpha.5   |
| `remember`       | done   | alpha.5   |
| `forget`         | done   | alpha.5   |
| `list_memories`  | done   | alpha.5   |
| `checkpoint`     | todo   | 0.2.0     |
| `resume`         | todo   | 0.2.0     |
| `list_sessions`  | todo   | 0.2.0     |
| `end_session`    | todo   | 0.2.0     |

### Storage

- SQLite via rusqlite (bundled), WAL mode, busy_timeout 5000 ms.
- Schema at `user_version = 3`:
  - `001_initial.sql` — `memories`, `projects`, `recall_log`, `meta`.
  - `002_embeddings.sql` — `memory_embeddings` (BLOB f32 LE).
  - `003_fts.sql` — `memory_fts` (FTS5) plus content-sync triggers.
- sqlite-vec virtual table is **not** used yet; cosine similarity is
  computed in Rust over the BLOB column. Sufficient for the v1
  working set; revisit at scale.

### Embedding

- fastembed 5.14 with BGE-small-en-v1.5, 384 dimensions.
- Downloaded into `~/.memryzed/models/` on first `init`.
- `MEMRYZED_DISABLE_EMBEDDING=1` swaps in `NoopEmbedder` for tests
  and offline CI.
- Linux glibc shim at `crates/memryzed-core/c/glibc_compat.c` keeps
  the prebuilt ONNX Runtime linkable on glibc < 2.38 (e.g. Ubuntu
  22.04). Weak symbols, so it is harmless on glibc 2.38+.

### Retrieval

- Hybrid: cosine similarity (vector) + BM25 (FTS5) + recency boost.
- Default weights 0.6 / 0.3 / 0.1 plus a 0.1 additive bonus for
  pinned memories.
- Pure Rust math; no sqlite-vec dependency.

## Test coverage

109 tests pass plus 1 ignored real-model test:

- 20 CLI integration tests (`crates/memryzed-cli/tests/cli.rs`).
- 83 core unit tests across `audit`, `clock`, `embedder`, `error`,
  `export`, `id`, `integrations`, `memory`, `paths`, `projects`,
  `retrieval`, `storage`, `version`.
- 6 MCP tool tests in `memryzed-mcp`.
- 1 `#[ignore]` test that downloads the real BGE-small model and
  embeds two strings; passes locally.

CI gate enforced locally and in `.github/workflows/ci.yml`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -Dwarnings`
- `cargo test --workspace`
- Runs on Linux, macOS, Windows.

## Local git history

```
(beta.1) feat: v0.1.0-beta.1 client integrations, audit log, config, export/import
56c0983 fix(docs): correct Kiro CLI MCP config path
87dd529 docs: add docs/development/status.md as the persistent project checkpoint
630517a feat: v0.1.0-alpha.5 MCP server (recall, remember, forget, list_memories)
ac0675b feat: v0.1.0-alpha.4 hybrid retrieval (vector + FTS + recency)
4700049 feat: v0.1.0-alpha.3 embeddings via fastembed-rs
357f63b feat: v0.1.0-alpha.2 storage + memory CRUD
acc0b69 chore: scaffold v0.1.0-alpha.1 (proof-of-life)
```

Nothing has been pushed to a remote. There are no tags yet; tags
are produced by the release pipeline once `cargo-dist` is set up.

## Decisions still open

- GitHub org and repo creation. Name reserved as `memryzed` per the
  brand decisions, but no actual org/repo exists yet.
- `cargo-dist` setup. The release pipeline is documented in
  `docs/development/release-process.md` but not wired into CI.
- The website at `memryzed.com`. Domain decision recorded; not
  registered or deployed.
- Code-signing certs for macOS (Apple Developer Program) and
  Windows (EV Code Signing). Documented as needed; not budgeted.
- npm wrapper for `@memryzed/cli`. Listed as stretch for v1.0;
  not started.

## What is next on the roadmap

In planned order, smallest first:

1. `memryzed install` — auto-detect Claude Code, Kiro, Cursor,
   Codex, and Continue MCP configs and write the Memryzed entry
   automatically. Pairs with `uninstall` and `--print` flag.
2. `cargo-dist` integration and the install scripts at
   `memryzed.com/install.sh`, `install.ps1`, `install.cmd`.
3. The first published quality benchmark numbers per
   `docs/specs/benchmarks.md`.
4. `0.2.0` — sessions: `checkpoint`, `resume`, `list_sessions`,
   `end_session` plus the `sessions` table and the agent-side
   resume UX.
5. `0.3.0` — rule-based extractor with the pending review queue
   and the `memryzed review` TUI.
6. `0.4.0` — optional Ollama extractor; `memryzed update`; more
   MCP client integrations.
7. `0.5.0+` — polish, transcript mining, auto-save hooks, packaging
   (Homebrew, Scoop, winget), benchmarks publication.
8. `1.0.0` — full v1 spec met.

## Wiring Memryzed into Claude Code (current state)

Add this entry to `~/.claude/mcp.json`:

```json
{
  "mcpServers": {
    "memryzed": {
      "command": "/root/.memryzed/bin/memryzed",
      "args": ["serve"]
    }
  }
}
```

Restart Claude Code. The four tools (`recall`, `remember`,
`forget`, `list_memories`) appear in the client. Sessions are not
yet implemented; the agent will see clear errors if it asks for
them.

## Useful commands during development

From the workspace root:

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -Dwarnings
cargo fmt --all -- --check
cargo build --release --package memryzed-cli
cp target/release/memryzed ~/.memryzed/bin/memryzed
```

To run the real-model embedder test locally:

```
cargo test -p memryzed-core fastembed_real -- --ignored --nocapture
```

To drive the MCP server by hand:

```
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocolVersion":"2024-11-05","capabilities":{},
  "clientInfo":{"name":"smoke","version":"0"}}}' \
| memryzed serve
```

## What this document is not

- Not the user-facing roadmap. That is `docs/roadmap.md`.
- Not the release record. That is `CHANGELOG.md`.
- Not the spec. That is `docs/specs/v1.md` and
  `docs/specs/benchmarks.md`.
- Not architecture notes. Those are `docs/architecture.md` and
  `docs/data-model.md`.

Update this file at the end of every working session that changes
the state. Treat it as the persistent context that survives across
chat sessions and across contributors.
