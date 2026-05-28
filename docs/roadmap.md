# Roadmap

This document captures what is planned next. The roadmap is a guide,
not a contract; priorities shift based on user feedback. Anything
listed here may change.

The `CHANGELOG.md` is the authoritative record of what has shipped.

## Now: v1.0

The v1.0 release is the focus of current work. It is fully specified
in `docs/specs/v1.md`. The scope is:

- Local MCP server with eight tools.
- Three-scope memory model (global, project, session).
- Per-repo session checkpoint and resume.
- SQLite storage with vector and full-text indexes.
- Local embedding model (BGE-small).
- Rule-based extractor with optional Ollama integration.
- TUI for review of pending memories.
- CLI with full command surface.
- Single-binary distribution via `cargo-dist`.
- Install scripts for macOS, Linux, WSL, Windows.
- Auto-detection and wiring of the major MCP clients.

Target ship: 4 to 6 weeks of focused work from the start of
implementation.

## Next: v1.x maintenance and polish

After v1.0 ships, the immediate focus combines real-world bug fixing
with three named features that close the largest gaps against
existing memory tools:

### Transcript mining

`memryzed mine <path>` ingests existing conversation transcripts
into Memryzed as session records and candidate memories. Initial
formats:

- Claude Code session JSONLs at `~/.claude/projects/`.
- Codex CLI session files.
- Cursor and Continue chat history where the format is documented.

Each ingested transcript becomes a session in the appropriate
project and feeds the extractor for candidate memories. Idempotent
on re-run. Solves the "I have months of agent history I want
indexed" problem on day one for new users.

### Auto-save hooks for Claude Code

A pair of hooks that run inside Claude Code to keep Memryzed up to
date without explicit user action:

- A periodic checkpoint hook that calls `checkpoint` every N
  conversation turns.
- A pre-compaction hook that calls `checkpoint` before Claude Code
  truncates context, so nothing is lost when context runs out.

Wired automatically by `memryzed install` for Claude Code.
Equivalents for other clients added as their hook surfaces
stabilize.

### Quality benchmarks publication

The first public quality numbers, run on the v1.0 implementation
against standard datasets (LongMemEval, LoCoMo, MemBench, ConvoMem)
with the methodology in `docs/specs/benchmarks.md`. Honest
reporting of wins, losses, and the conditions under which each
applies. Publishing benchmarks builds credibility and gives users
a concrete reason to choose Memryzed.

### Maintenance work

Alongside the named features:

- Bug fixes from real-world usage.
- More MCP clients auto-detected as their configurations stabilize.
- More robust extraction patterns based on what users actually say.
- Performance tuning on large memory stores.
- Better defaults in `config.toml` based on user feedback.
- Code signing for macOS and Windows binaries.
- Submission to homebrew-core and winget.

## Soon: cloud sync (paid)

The flagship paid feature. End-to-end encrypted sync of memories and
sessions across machines. Notable design points:

- Same schema locally and in the cloud.
- Per-user encryption keys held by the user; the server stores
  ciphertext for any sensitive field.
- Conflict resolution by last-write-wins on field, with a manual
  override surface.
- A web dashboard for review and search.

Pricing tier outline (subject to change):

- Free: local only. Always.
- Cloud individual: paid monthly. Sync, hosted backups, web
  dashboard.
- Team: paid per seat. Shared project memory, RBAC, audit export.
- Enterprise: paid annually. Self-hosted, SSO, on-prem embeddings,
  custom retention.

## Later: better extraction and broader reach

Replace and augment the v1 extractor and broaden the product's
reach:

- Frontier-model extraction (cloud-only, paid).
- Multilingual embedding models. Memryzed v1 ships English-tuned
  embeddings; expanding to multilingual support requires choosing a
  multilingual model, regenerating embeddings, and updating tests.
  Deferred deliberately: the v1 audience is primarily
  English-speaking developers, and shipping a tight English-only
  v1 is preferable to a thinner multilingual v1.
- Per-user fine-tuning of which extraction patterns to apply.
- Active suggestions: the extractor surfaces "did you mean to
  remember this?" prompts in the agent UI when a fact looks
  important but is below the threshold.

## Later: shared project memory

A team-tier feature that makes `project` scope shareable. Imagine
joining a project and inheriting 50 curated facts about its
conventions. Built on top of cloud sync. Requires careful design of
permissions and history.

## Later: self-hosted cloud server

For enterprises that cannot use SaaS. The same server that powers the
cloud product, packaged for self-hosting on Kubernetes or a single
VM. Same protocol, same client.

## Later: more languages and platforms

- First-class macOS notarization.
- A direct Windows MSI installer alongside the curl one-liner.
- Linux distribution packages (apt, dnf, pacman).

## Things we are explicitly not doing

- Building a chat UI of our own. The agents are the chat UI.
- Building integrations into IDEs as plugins. The MCP server is the
  integration. The IDE just needs to support MCP.
- Building a knowledge graph layer. Plain natural-language facts
  with retrieval are what users actually want; structured
  knowledge-graph schemas have not held up well in this category.
- Auto-pulling content from your filesystem. Memryzed only stores
  what an agent explicitly hands it via `remember` or what the
  extractor proposes from agent turns. We do not crawl your code.

## Influencing the roadmap

If something on this list is more or less important to you than it is
to us, say so. The fastest way is a GitHub issue or a discussion. We
take user feedback seriously, especially when it comes from people
who have integrated and use Memryzed daily.
