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

Three named features that closed the largest gaps against existing
memory tools have shipped in 0.5.0:

### Transcript mining (shipped in 0.5.0)

`memryzed mine <path>` ingests existing conversation transcripts
into Memryzed as session records and candidate memories. Supports
Kiro CLI session JSONL (`~/.kiro/sessions`) and Claude Code session
JSONL (`~/.claude/projects`), with source auto-detection, dry-run,
and idempotent re-runs. Additional source formats (Codex, Cursor,
Continue) are added as their layouts are confirmed.

### Auto-save hooks for Claude Code (shipped in 0.5.0)

`memryzed hooks install` wires two hooks into Claude Code: a
periodic checkpoint and a pre-compaction hook, both of which mine
the active transcript so memory stays current without the user
asking. Equivalents for other clients are added as their hook
surfaces stabilize.

### Quality benchmarks harness (shipped in 0.5.0)

The `memryzed-bench` harness measures recall at K against a
normalized dataset. The first published numbers, run against
LongMemEval, LoCoMo, ConvoMem, and MemBench with the methodology in
`docs/specs/benchmarks.md`, are still to come: they require
converting each license-gated dataset to the normalized format and
running with the embedding model active.

### Maintenance work

Ongoing alongside the named features:

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
