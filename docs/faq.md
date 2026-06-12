# Frequently asked questions

## What is Memryzed?

Memryzed is a local memory and session store for AI coding agents. It
runs as an MCP server and gives any MCP-aware client persistent
memory across sessions and resumable working state per repository.

## Which AI agents work with it?

Anything that supports the Model Context Protocol. Confirmed:

- Claude Code
- Kiro CLI
- Codex CLI
- Cursor
- Copilot CLI (via the gh-copilot extension's MCP support)
- Continue

Other MCP-aware tools should work without changes.

## Is it free?

Yes. Memryzed is fully open source under Apache-2.0, the server, the
CLI, and the documentation. There is no paid tier and no hosted
product; the entire project is free and runs on your machine.

## Where is my data stored?

In a single SQLite database at `~/.memryzed/db.sqlite` on Unix or
`%LOCALAPPDATA%\memryzed\db.sqlite` on Windows. Nothing is sent to
any server in v1. The optional Ollama extractor talks only to your
local Ollama instance.

## Does Memryzed require an internet connection?

Only at install time, to download the binary, and at first
initialization, to download the embedding model. After that, it
operates fully offline. The optional update check on `serve` startup
contacts GitHub once a day; you can disable it in `config.toml`.

## What languages does it support?

The embedding model in v1 is English-tuned. Memories in other
languages are stored verbatim and can be retrieved via FTS but vector
similarity will be weaker. Multi-lingual embedding models are on the
roadmap.

## Does it work in containers and CI?

Yes, on glibc-based images (Debian, Ubuntu, Amazon Linux, and most
others). The bundled embedding runtime needs glibc, so musl-only
images such as Alpine are not supported yet. For CI use, point
`MEMRYZED_DATA_DIR` at a writable location in your runner. CI is
generally not the place to build long-term memory, but it is useful
for testing integrations.

## How big does the database get?

A typical individual user with active use over six months has under
10 MB of database, plus the audit log (capped at about 100 MB total
because of rotation), plus the embedding model (about 130 MB).

## Can I use a different embedding model?

Not in v1. Switching models requires regenerating all embeddings,
which we do support, but only one model is shipped. We will expand
this in a future release.

## Can I share memories with my team?

Not currently. You can share through `memryzed export` and
`memryzed import` if you want to move a curated set of memories
between people or machines. Optional self-hostable sync is on the
roadmap.

## What about privacy of my conversations?

Memryzed never sends conversation content anywhere by default. The
extractor proposes facts that go into the pending queue for your
review. You decide what gets stored. The optional Ollama extractor
runs locally. Telemetry, when enabled, sends only counters; no
content.

When Memryzed captures conversation turns, it scans them for
high-confidence secrets, vendor API tokens (AWS, GitHub, Slack,
Stripe, OpenAI, Google, npm), private-key blocks, JWTs, and explicit
password/secret assignments, and replaces them with a
`[REDACTED:<kind>]` marker before anything is stored or embedded.
Redaction is deliberately conservative: it only touches text that is
unmistakably a credential, so ordinary code, hashes, and identifiers
are kept verbatim.

## Does Memryzed see my source code?

Memryzed sees only the strings the agent passes to its tools. If the
agent calls `remember("I changed the foo function")`, that string is
stored. Memryzed does not read your filesystem; the only files it
opens are its own database and configuration.

## How do I back up my data?

Run `memryzed export > backup.json` periodically. Restore with
`memryzed import backup.json`. The format is portable across
versions.

## Can I run Memryzed on a different machine and use it from this one?

Not currently. Memryzed is local-first: each machine has its own
store. You can copy the SQLite database between machines yourself if
you want to move your memory, since everything lives in that one file.

## How do I delete a memory permanently?

`memryzed forget <id>` archives a memory. To delete permanently, use
`memryzed forget <id> --hard`. Permanently deleted memories cannot be
recovered.

## Can the agent silently change my memories?

No. Calls to `remember` from an agent always result in stored
memories that you can inspect with `memryzed list`. The audit log
records every write. Your config controls whether agent-proposed
memories from the background extractor are auto-approved or sent to
the pending queue for review.

## Why is my agent not using a memory I added?

The most common reasons:

- The query the agent constructed did not match the memory well.
- The memory is in `pending` status and not yet approved.
- The memory was archived.

`memryzed list` shows your active memories. `memryzed search <query>`
shows what the agent would see for a given query.

## What happens when a memory becomes wrong?

You have several options:

- Edit it: `memryzed edit <id>`.
- Forget it: `memryzed forget <id>`.
- Tell the agent the correct fact in the conversation; the
  extractor's correction pattern will update or supersede the old
  fact.

## Why is there no GUI?

A web UI is on the roadmap. For v1 we focused on a great CLI and TUI
experience because that is what coding agent users live in.

## What happens if I delete `~/.memryzed/`?

Your data is gone. Memryzed will reinitialize on next use. There is
no automatic backup unless you have run `memryzed export`.

## What language is Memryzed written in?

Rust. The choice is documented in `docs/architecture.md`.

## Can I contribute?

Yes. See `CONTRIBUTING.md`.

## How do I report a security issue?

See `SECURITY.md`. Do not open a public issue.

## Is there an API I can call directly without an MCP client?

Memryzed is designed around the MCP transport. There is no separate
HTTP API in v1. You can use the CLI for scripting; every command
supports `--json` output.

## What is the relationship between Memryzed and existing memory tools (Letta, Mem0, Zep)?

Those tools target a different audience. They are designed for
developers building chat applications who need a memory backend.
Memryzed is designed for individual developers who use coding agents,
exposing a stable MCP surface so any client can use it. The
underlying retrieval techniques are similar; the product surface,
distribution model, and user is different.

## What about ChatGPT, Claude, and Gemini's built-in memory features?

Built-in memory is per-vendor and lives on the vendor's servers. If
you switch tools, you lose your memory. Memryzed is the inverse:
client-agnostic, local, and yours.
