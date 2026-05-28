# Troubleshooting

This guide covers the most common problems and how to resolve them.
For anything not listed here, run `memryzed doctor` first; its output
points to the right section of the documentation.

## Installation

### `memryzed: command not found` after install

The installer added Memryzed to your PATH but your current shell has
not loaded the change. Fix it by either:

- Opening a new terminal.
- Running `source ~/.bashrc` (or `~/.zshrc`, `~/.config/fish/config.fish`).
- On Windows, signing out and back in to refresh the user PATH.

If the binary is genuinely missing, reinstall:

    curl -fsSL https://memryzed.com/install.sh | bash

### Install script fails with a TLS or certificate error

Your network is intercepting HTTPS or your system's certificate store
is out of date. Try one of:

- Update your OS certificate store.
- Download the binary directly from
  `https://github.com/memryzed/memryzed/releases/latest` and place it
  in a directory on your PATH.

### Install on macOS warns "unidentified developer"

Until Memryzed is code-signed, macOS Gatekeeper will warn the first
time the binary runs. You can bypass it once with:

    xattr -d com.apple.quarantine ~/.memryzed/bin/memryzed

We are working on Apple notarization for a future release.

### `memryzed install` cannot find my agent's config

Run `memryzed install --client <name> --print` for your client. This
prints the configuration block to add manually. Then add the block to
your client's MCP configuration file. The standard config locations
are documented in `docs/mcp-reference.md`.

## Initialization and first run

### `memryzed init` fails to download the embedding model

The model is hosted on Hugging Face. Common causes and fixes:

- Behind a corporate proxy: set `HTTPS_PROXY` and try again.
- Restricted network: download the model bundle on another machine
  and pass `--offline-bundle <path>` to `memryzed init`.
- Disk space: the model is about 130 MB on disk, plus working space.
  Free up space and retry.

### `memryzed init` says the data directory already exists

This is fine. `init` is idempotent. It will not overwrite your data.
If you genuinely want to start fresh, run `memryzed uninstall --purge`
followed by a fresh install and `memryzed init`.

## The MCP server

### My agent does not see Memryzed tools

Verify in this order:

1. `memryzed doctor` shows the integration as configured for your
   client.
2. Your agent has been fully restarted since `memryzed install`.
3. The configuration file written by `memryzed install` is what your
   agent expects. Some clients use a slightly different config path
   per OS or per profile; consult your client's documentation.
4. `memryzed log -f` shows activity when you talk to your agent. If
   it stays silent, the server is not being spawned.

### The server keeps crashing or hanging

Run `memryzed serve` directly in a terminal and observe the output:

    MEMRYZED_LOG_LEVEL=debug memryzed serve < /dev/null

If you see a panic, please open a GitHub issue with the output and
your `memryzed --version` and OS.

### `storage_error` returns from tool calls

Causes and fixes:

- Database is locked because another Memryzed process is writing.
  Wait a few seconds and retry; the busy timeout should usually
  handle this.
- Disk is full. Check with `df -h ~/.memryzed`.
- Corruption. Run `sqlite3 ~/.memryzed/db.sqlite 'PRAGMA integrity_check;'`.
  If it returns anything other than `ok`, restore from your last
  `memryzed export` backup.

## Memory and retrieval

### Recall is returning irrelevant results

A few things to check:

- The query phrasing. Specific queries beat generic ones. See
  `docs/for-agent-authors.md` for query advice.
- The store has too few memories for the query. With fewer than
  about 20 memories, retrieval can be noisy. Add more or wait for
  the store to grow.
- The scope is wrong. If a fact is in `project` scope but you are
  recalling with `scope=global`, you will not find it.
- The fact is in the pending queue. Run `memryzed list --status
  pending` to confirm.

### Memories I expected to be remembered are not in the store

The extractor is conservative in v1. It captures explicit user
statements ("I prefer X", "this repo uses X", "remember X") but does
not infer broadly. Two options:

- Tell your agent to remember explicitly: "remember that I prefer X".
- Add the fact yourself: `memryzed remember "I prefer X" --scope global`.

### Pending queue keeps growing

Run `memryzed review` to triage. If the queue grows faster than you
can review, raise the auto-approve threshold:

    memryzed config set memory.auto_approve_threshold 0.7

The default is 0.85. Lower values auto-approve more, higher values
auto-approve fewer. Raise the value if you want a tighter queue.

## Sessions

### `resume()` returns no session in a project I have used before

Possible causes:

- The agent never called `checkpoint` for that project. Sessions are
  only created when an agent calls `checkpoint`.
- The project identity changed. Memryzed identifies a project by its
  git remote URL, falling back to the absolute path. If you cloned
  the same repo to a new path and the remote URL has changed (for
  example, you switched from HTTPS to SSH), Memryzed sees it as a
  new project. Inspect with `memryzed projects` and merge if needed.

### A session has the wrong title or stale state

Sessions are owned by your agent. The cleanest fix is to ask the
agent to checkpoint again, which updates the existing session.

To inspect or edit a session, use `memryzed resume <id> --json` and
work with the JSON. Direct SQL editing is supported but unsupported.

## Configuration

### My configuration changes do not take effect

The MCP server reads its configuration on startup. Running clients
will not see changes until they spawn a new server, which usually
means restarting the client.

CLI commands read configuration on every invocation, so they pick up
changes immediately.

## Updates

### `memryzed update` reports no update available, but I know there is one

The check uses the GitHub Releases API, which sometimes lags by a few
minutes after a tag. Retry in a minute. If still nothing, check
`https://github.com/memryzed/memryzed/releases` directly.

### After updating, things behave differently

Read the `CHANGELOG.md` for the version you upgraded to. User-visible
changes are documented there. If something feels like a regression,
open an issue with the version you came from, the version you went
to, and the symptom.

## Asking for help

When asking for help, include:

- `memryzed --version`
- Your operating system and architecture.
- The output of `memryzed doctor`.
- Recent entries from `memryzed log` if relevant.
- The MCP client you are using.

The fastest way to get help is a GitHub issue at
`https://github.com/memryzed/memryzed/issues`.
