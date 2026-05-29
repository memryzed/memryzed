# Project status

This document is the current snapshot of where Memryzed stands. It is
the first thing a contributor or a returning maintainer should read
to pick up cleanly. Update it whenever a milestone ships or a major
decision changes.

## Version

`0.4.0` (local git only; not yet tagged publicly).

## Where the code lives

- Local working tree: `/mnt/c/Users/HamzaArjah/Documents/Hamza/memryzed`
- Local git branch: `main`
- Remote: not yet created. The intended remote is
  `github.com/memryzed/memryzed`. The `update` command treats the
  missing remote gracefully and reports an `Unknown` status rather
  than failing.
- Release binary on this machine: `~/.memryzed/bin/memryzed`. May
  lag the latest committed version; rebuild and copy after every
  significant change.

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
| `memryzed update`             | done   | 0.4.0     |
| `memryzed review` (TUI)       | done   | 0.3.0     |
| `memryzed sessions`           | done   | 0.2.0     |
| `memryzed resume`             | done   | 0.2.0     |
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
| `checkpoint`     | done   | 0.2.0     |
| `resume`         | done   | 0.2.0     |
| `list_sessions`  | done   | 0.2.0     |
| `end_session`    | done   | 0.2.0     |
| `extract_from`   | done   | 0.3.0     |

### Storage

- SQLite via rusqlite (bundled), WAL mode, busy_timeout 5000 ms.
- Schema covering: `memories`, `memory_embeddings`, `memory_fts`,
  `projects`, `sessions`, `recall_log`, `meta`.
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
  22.04). Weak symbols, harmless on glibc 2.38+.

### Retrieval

- Hybrid: cosine similarity (vector) + BM25 (FTS5) + recency boost.
- Default weights 0.6 / 0.3 / 0.1 plus a 0.1 additive bonus for
  pinned memories.
- Pure Rust math; no sqlite-vec dependency.

### Extractor (0.3.0+)

- Rule-based pattern matcher implementing the patterns in the v1
  spec ("I prefer X over Y", "this repo uses X", "actually it is X
  not Y", direct "remember X" requests).
- Optional Ollama-based extractor at
  `crates/memryzed-core/src/extractor/ollama.rs` for richer
  candidates. Off by default; falls back to rule-based when Ollama
  is unreachable.
- Candidates flow into the pending queue with a confidence score.
  High-confidence candidates auto-approve; the rest go to
  `memryzed review` for triage.

### Sessions (0.2.0+)

- Per-project session records; `checkpoint` and `resume` MCP tools
  let agents save and restore working state per repository.
- Idle sessions transition to `paused` after 24 hours; archived
  after 30 days unless pinned.

### Update (0.4.0)

- `memryzed update --check` queries the GitHub Releases API at
  `https://api.github.com/repos/memryzed/memryzed/releases/latest`.
- Status enum: `UpToDate`, `Available`, `Unknown`. Unknown covers
  network failures, missing repository, and parse errors. Never
  surfaces as a hard error so the CLI exits zero on
  `update --check` even when offline.
- The actual binary swap is owned by the install script, not by
  the running process.

## Test coverage

150 tests pass plus 1 ignored real-model test:

- 21 CLI integration tests (`crates/memryzed-cli/tests/cli.rs`).
- 113 core unit tests across `audit`, `clock`, `embedder`, `error`,
  `export`, `extractor`, `id`, `integrations`, `memory`, `paths`,
  `projects`, `retrieval`, `sessions`, `storage`, `update`,
  `version`.
- 13 MCP tool tests in `memryzed-mcp` (8 tools).
- 3 additional small tests scattered across other modules.
- 1 `#[ignore]` test that downloads the real BGE-small model and
  embeds two strings; passes locally on demand.

CI gate enforced locally and in `.github/workflows/ci.yml`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -Dwarnings`
- `cargo test --workspace`
- Runs on Linux, macOS, Windows.

## Local git history (most recent first)

```
2bf2031 feat: v0.3.0 extractor, pending queue, extract_from tool, review TUI
7791c9a feat: v0.2.0 sessions per repo (checkpoint, resume, list_sessions, end_session)
bf42b70 release: 0.1.0
7654dbd build: v0.1.0-rc.1 release pipeline and install scripts
5c51881 feat: v0.1.0-beta.1 install/uninstall, audit log, config, export/import
56c0983 fix(docs): correct Kiro CLI MCP config path
87dd529 docs: add docs/development/status.md as the persistent project checkpoint
630517a feat: v0.1.0-alpha.5 MCP server (recall, remember, forget, list_memories)
ac0675b feat: v0.1.0-alpha.4 hybrid retrieval (vector + FTS + recency)
4700049 feat: v0.1.0-alpha.3 embeddings via fastembed-rs
357f63b feat: v0.1.0-alpha.2 storage + memory CRUD
acc0b69 chore: scaffold v0.1.0-alpha.1 (proof-of-life)
```

A v0.4.0 commit is pending and will land alongside the `update`
command, the Ollama extractor, and the doc refresh.

## Confirmed working with real agents

End-to-end exchange verified with Kiro CLI on 2026-05-28: the agent
called `memryzed.recall` with a natural-language query, got back
the stored fact, and answered correctly. The MCP transport,
embeddings, hybrid retrieval, and the four core tools all
round-trip correctly through a real client.

## Decisions still open

- GitHub org and repo creation. Name reserved as `memryzed` per the
  brand decisions, but no actual org/repo exists yet. Until that
  lands, `memryzed update` reports `Unknown`.
- `cargo-dist` setup. The release pipeline is documented in
  `docs/development/release-process.md`. Wired into CI in v0.1.0-rc.1
  but never exercised against a real remote.
- The website at `memryzed.com`. Domain decision recorded; not
  registered or deployed.
- Code-signing certs for macOS (Apple Developer Program) and
  Windows (EV Code Signing). Documented as needed; not budgeted.
- npm wrapper for `@memryzed/cli`. Listed as stretch for v1.0;
  not started.

## What is next on the roadmap

1. Finish v0.4.0 — Ollama extractor polish, `memryzed update`,
   smoke tests with the real agent.
2. v0.5.0+ — transcript mining for Claude Code JSONL session
   files; auto-save hooks to call `checkpoint` on a cadence;
   per-message recall via `memryzed sweep`.
3. Quality benchmarks publication per `docs/specs/benchmarks.md`
   (LongMemEval, LoCoMo, ConvoMem, MemBench).
4. Public release: GitHub org, push, tag v0.1.0, run cargo-dist,
   stand up `memryzed.com`.
5. Ongoing: more MCP client integrations, performance work,
   multilingual embeddings.

## Wiring Memryzed into the major MCP clients

| Client      | Config path                          |
|-------------|--------------------------------------|
| Claude Code | `~/.claude/mcp.json`                 |
| Kiro CLI    | `~/.kiro/settings/mcp.json`          |
| Cursor      | `~/.cursor/mcp.json`                 |
| Codex CLI   | `~/.codex/mcp.json`                  |
| Continue    | `~/.continue/config.json`            |

`memryzed install` handles auto-detection. For each, the entry is:

```json
{
  "mcpServers": {
    "memryzed": {
      "command": "/abs/path/to/memryzed",
      "args": ["serve"]
    }
  }
}
```

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

To drive the MCP server by hand (no embedder, fast):

```
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocolVersion":"2024-11-05","capabilities":{},
  "clientInfo":{"name":"smoke","version":"0"}}}' \
| MEMRYZED_DISABLE_EMBEDDING=1 memryzed serve
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
