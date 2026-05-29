# Memryzed

Persistent memory and session state for AI coding agents.

Memryzed is a local-first MCP server that gives any MCP-aware coding agent
(Claude Code, Kiro, Codex, Cursor, Copilot CLI, Continue, and others)
durable memory across sessions and resumable working state per repository.

Your agent stops forgetting. Your work stops restarting from zero.

## Status

Pre-1.0. Memryzed is feature-complete against the v1 specification
(`docs/specs/v1.md`): local memory store, hybrid retrieval,
per-repository sessions, the rule-based extractor with an optional
local-LLM extractor, and an MCP server exposing nine tools. The API
and on-disk format may still change before 1.0; see
`docs/development/versioning.md` for the compatibility policy and
`CHANGELOG.md` for what has shipped.

## Install

macOS, Linux, WSL:

```
curl -fsSL https://memryzed.com/install.sh | bash
```

Windows PowerShell:

```
irm https://memryzed.com/install.ps1 | iex
```

Windows Command Prompt:

```
curl -fsSL https://memryzed.com/install.cmd -o install.cmd && install.cmd
```

After install:

```
memryzed init        # one-time setup
memryzed install     # auto-wire into detected MCP clients
memryzed doctor      # verify everything is working
```

For detailed setup, see `docs/getting-started.md`.

## What Memryzed gives you

Three layers of persistent context, exposed to any MCP-aware agent through a
small, stable tool surface:

- Global memory. User-wide preferences and facts that follow you across every
  project and every machine.
- Project memory. Repository-scoped facts that persist across sessions in the
  same repository: build commands, conventions, ownership, decisions.
- Session state. Per-task working state that lets you resume exactly where you
  left off, including open files, recent conversation, and last commands.

All data lives on your machine in a single SQLite database. No accounts,
no telemetry by default, no network calls.

## How it works

Memryzed runs as a local MCP server that your agent talks to over stdio.
Agents call eight tools to recall facts, store facts, checkpoint sessions,
and resume them. A background extractor proposes facts from your
conversations; you approve, edit, or reject them through a CLI review queue.

For the full architecture, see `docs/architecture.md`.

## Documentation

User documentation:

- `docs/getting-started.md` - install and first use
- `docs/concepts.md` - the memory model and scopes
- `docs/cli-reference.md` - every command
- `docs/mcp-reference.md` - every MCP tool
- `docs/configuration.md` - configuration options
- `docs/troubleshooting.md` - when something is wrong
- `docs/faq.md` - common questions

For agent and client authors:

- `docs/for-agent-authors.md` - how to integrate Memryzed cleanly

For contributors and operators:

- `docs/architecture.md` - system architecture
- `docs/data-model.md` - on-disk format and schema
- `docs/specs/v1.md` - the full v1 specification
- `docs/development/` - development, release, and incident-response process
- `docs/roadmap.md` - what is planned next

## License

Apache-2.0. See `LICENSE` and `NOTICE`.

## Contributing

See `CONTRIBUTING.md` for the development setup, branching model, and how to
propose changes. By participating in this project you agree to the
`CODE_OF_CONDUCT.md`.

## Security

To report a vulnerability, follow the process in `SECURITY.md`. Do not open
public issues for security reports.
