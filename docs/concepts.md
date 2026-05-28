# Concepts

This document explains the model behind Memryzed. Read it once and the
rest of the documentation will make sense.

## The problem Memryzed solves

AI coding agents are stateless across sessions. Every time you close
your client and reopen it, the agent has forgotten everything about
you, your project, and what you were working on. You re-explain your
preferences, your build commands, your conventions, and the task you
were halfway through.

Memryzed is a persistent memory and session store that fixes both
problems. It runs locally as an MCP server, exposes a small set of
tools to any MCP-aware client, and stores everything in a single
SQLite database under your home directory.

## Two kinds of state

Memryzed distinguishes two things that are often confused:

- Memory: discrete facts that are useful for a long time. "I prefer
  pnpm." "This repo uses Vitest." "The deploy command is `make ship`."
  Memory is searchable, indexable, and small per item.

- Session state: the working state of a single task. The files you
  had open, the recent turns of the conversation, the last commands
  you ran, the working directory. Session state is large, opaque, and
  per task.

These have different lifetimes, different storage, and different
retrieval. Mixing them produces a system that is mediocre at both, so
Memryzed keeps them separate.

## Three scopes for memory

Every memory has a scope. Scopes are what make retrieval relevant.

- Global. Applies to you everywhere. Examples: "I prefer pnpm over
  npm." "I write commit messages in conventional format." "I am in
  UTC+3."

- Project. Applies to a specific repository, regardless of which
  machine you are on. Examples: "This repo uses Vitest." "The deploy
  command is `make ship`." "The auth module owner is Sara." A project
  is identified by the SHA-256 of its git remote URL when one is
  available, falling back to the absolute path of the working
  directory.

- Session. Applies to a single task. Mostly used for agent-internal
  bookkeeping during a long task. Sessions also carry the working
  state described above.

When an agent calls `recall`, by default it gets results from all three
scopes ranked together. It can also restrict the query to a single
scope.

## Sessions are per repo

A session is the unit of work in a single repository. When you open a
repo, you are either resuming the most recent session for that repo or
starting a new one. The agent can also explicitly end a session when a
task is complete.

A session contains:

- An ID and a title.
- The project it belongs to.
- A state blob: open files, recent turns, working directory, last
  commands, and any other state the agent considers worth saving.
- Status: active, paused, completed, or archived.
- Timestamps.

Sessions are how `resume` works. You ask for the last session in this
repo, and Memryzed gives you back enough context to pick up where you
left off.

## The trust loop

Memory is only useful if you trust it. Memryzed keeps you in the loop
through three mechanisms:

1. The audit log. Every recall query and every fact stored is logged
   to `~/.memryzed/audit.log`. Run `memryzed log` to see recent
   activity, or `memryzed log -f` to watch it live.

2. The pending queue. Facts that the extractor proposes go to a
   pending queue. They are not used in retrieval until you approve
   them. Run `memryzed review` to triage. High-confidence facts can be
   set to auto-approve in `config.toml`.

3. The transparency response. Every `recall` response includes a
   short summary line that the agent can render in its output, so you
   see "Memryzed: used 1 fact" alongside the agent's answer. Whether
   it is rendered depends on the agent.

You can always inspect, edit, pin, or delete any memory at any time.
There is no privileged state inside Memryzed that you cannot see.

## What gets remembered

The extractor is conservative by design. In v1, it captures memories
from a small set of explicit and high-signal patterns:

- Direct user requests: "remember that I prefer pnpm".
- Stated preferences: "I always use X", "I prefer X over Y".
- Project facts: "this repo uses X", "the build command is X".
- Corrections: "actually it is X, not Y" (updates an existing fact).

When Ollama is configured (off by default), an optional local LLM
extractor proposes richer facts from recent conversation. These also
go to the pending queue.

Anything the agent calls `remember` on directly bypasses the extractor
and goes straight to approved status, since the user is in the loop
when the agent calls `remember` explicitly.

## Storage

Everything Memryzed stores is in:

- `~/.memryzed/db.sqlite`: the main database. Memories, sessions,
  projects, and metadata.
- `~/.memryzed/models/`: the embedding model files.
- `~/.memryzed/config.toml`: user configuration.
- `~/.memryzed/audit.log`: the activity log.

There is no cloud component in v1. Nothing leaves your machine. The
optional Ollama extractor talks only to your local Ollama instance.

## Retrieval

When an agent calls `recall(query, scope)`, Memryzed runs a hybrid
search:

- Vector similarity over an embedding of the query.
- Full-text BM25 search over the memory contents.
- A recency boost so newer facts win on ties.

Results are merged, deduplicated, and ranked. The agent gets back a
small set of candidate memories, each with its scope, its content, its
confidence, and its ID for later inspection.

## Why local first

Memryzed is local first because:

- Developers do not paste proprietary code into other people's
  servers.
- The product is fast when there is no network on the path.
- Backup, export, version control, and audit are all simpler when the
  data is one file you own.
- A future cloud sync layer can be added without changing the local
  experience.

## Where to go from here

- For commands, see `docs/cli-reference.md`.
- For the MCP tool surface, see `docs/mcp-reference.md`.
- For configuration, see `docs/configuration.md`.
- For the system architecture, see `docs/architecture.md`.
- For the on-disk format, see `docs/data-model.md`.
- For the full v1 specification, see `docs/specs/v1.md`.
