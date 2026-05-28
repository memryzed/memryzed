# Getting started

This guide takes you from a clean machine to a working Memryzed install
wired into your AI agent. It should take five to ten minutes.

## Requirements

Memryzed is a single statically linked binary with no external runtime
dependencies. It works on:

- macOS (Apple Silicon and Intel)
- Linux x86_64 (glibc and musl)
- Linux aarch64
- Windows x86_64
- Windows on ARM
- WSL on Windows

You need at least 200 MB of free disk space (most of which is the
embedding model downloaded on first run) and an MCP-aware coding agent.
Memryzed has been tested with Claude Code, Kiro CLI, Codex CLI, Cursor,
Copilot CLI, and Continue. Any other MCP-aware client should work.

## Step 1: Install the binary

macOS, Linux, or WSL:

    curl -fsSL https://memryzed.com/install.sh | bash

Windows PowerShell:

    irm https://memryzed.com/install.ps1 | iex

Windows Command Prompt:

    curl -fsSL https://memryzed.com/install.cmd -o install.cmd && install.cmd

The installer downloads the correct binary for your platform from
GitHub Releases, verifies its checksum, places it at
`~/.memryzed/bin/memryzed`, and adds that directory to your shell PATH.
On Windows, the binary is placed at `%LOCALAPPDATA%\memryzed\bin\` and
added to the user PATH via the registry.

After the installer finishes, open a new shell or source your shell
profile, then verify:

    memryzed --version

## Step 2: Initialize

    memryzed init

This creates `~/.memryzed/`, downloads the embedding model (about
130 MB), initializes the SQLite database, and writes a default
`config.toml`. You will be asked to confirm before any downloads
happen.

## Step 3: Wire Memryzed into your agent

    memryzed install

This scans well-known locations for MCP-aware clients on your machine
(Claude Code, Kiro CLI, Cursor, Codex, Continue, and others) and adds
Memryzed to each detected client's MCP configuration. You will be asked
to confirm before any config files are modified.

If your client is not auto-detected, run:

    memryzed install --client <name> --print

This prints the configuration block to add manually to your client's
MCP configuration file. See `docs/mcp-reference.md` for the supported
client names and where each one keeps its configuration.

## Step 4: Restart your agent

MCP clients read their configuration on startup, so close your agent
fully and reopen it. From this point on, your agent has access to the
eight Memryzed tools (`recall`, `remember`, `forget`, `list_memories`,
`checkpoint`, `resume`, `list_sessions`, `end_session`).

## Step 5: Verify everything works

    memryzed doctor

`doctor` checks that the binary is in PATH, the database is
initialized, the embedding model is loaded, each integration is
present, and that recent activity has flowed through. If any check
fails, the output explains how to fix it.

To watch what your agent is doing in real time, in a separate terminal
run:

    memryzed log -f

Then talk to your agent. You should see entries appear as the agent
calls Memryzed.

## What to do next

- Read `docs/concepts.md` to understand the memory model.
- Read `docs/cli-reference.md` for every command and flag.
- Read `docs/configuration.md` to tune behavior.
- If your agent is misusing Memryzed, point your agent's author at
  `docs/for-agent-authors.md`.

## Uninstalling

If you want to remove Memryzed:

    memryzed uninstall

By default this removes the binary and the PATH entry but keeps your
data and the MCP client integrations. Pass `--purge` to remove the data
and `--unwire` to remove the MCP integrations as well.

## Getting help

If something does not work, see `docs/troubleshooting.md`. If you find
a bug or have a feature request, open an issue at
`https://github.com/memryzed/memryzed/issues`.
