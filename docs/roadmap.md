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

## Soon: optional sync between your machines

An optional, self-hostable sync so your memories and sessions can
follow you across your own machines. It is not required and not a
product, Memryzed stays fully functional and fully local without it.
Design points:

- Same schema locally and over the wire.
- Per-user encryption keys held by the user; only ciphertext is
  synced for sensitive fields.
- Conflict resolution by last-write-wins on field, with a manual
  override surface.

Everything here is open source under Apache-2.0, like the rest of the
project. There is no paid tier.

## Later: better extraction and broader reach

Replace and augment the v1 extractor and broaden the project's reach:

- Stronger extraction using the local LLM extractor, and optionally a
  user-supplied model, never a hosted/paid one.
- Multilingual embedding models. Memryzed v1 ships English-tuned
  embeddings; expanding to multilingual support requires choosing a
  multilingual model, regenerating embeddings, and updating tests.
  Deferred deliberately: the v1 audience is primarily
  English-speaking developers, and shipping a tight English-only
  v1 is preferable to a thinner multilingual v1.
- Active suggestions: the extractor surfaces "did you mean to
  remember this?" prompts when a fact looks important but is below
  the threshold.

## Later: shareable project memory

Make `project` scope shareable so a team can check curated project
facts into version control or a shared store and inherit them when
joining a repository. Built on the optional sync above. Requires
careful design of permissions and history. Open source like
everything else.

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
