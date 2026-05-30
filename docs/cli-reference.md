# CLI reference

This is the complete reference for the `memryzed` command-line tool.
Run `memryzed --help` for a short version of this information.

## Synopsis

    memryzed <command> [arguments] [flags]

Global flags accepted by every command:

    --config <path>      Path to config file. Defaults to ~/.memryzed/config.toml.
    --data-dir <path>    Path to data directory. Defaults to ~/.memryzed/.
    --json               Emit machine-readable JSON output where supported.
    --quiet              Suppress non-error output.
    --no-color           Disable colored output.
    --help               Show help for a command.
    --version            Print the version and exit.

## Setup commands

### `memryzed init`

Initialize Memryzed on this machine. Creates the data directory,
downloads the embedding model, and writes a default configuration.
Idempotent: running it on an already-initialized install reports the
current state and exits successfully.

Flags:

    --offline-bundle <path>   Use a pre-downloaded model bundle.
    --yes                     Do not prompt for confirmation.

### `memryzed install`

Detect MCP-aware clients on this machine and add Memryzed to each
client's MCP configuration. Backs up each modified config to
`<file>.memryzed.bak` before writing.

Flags:

    --client <name>     Target a specific client only.
    --print             Print the config block to stdout instead of writing.
    --yes               Do not prompt for confirmation.

Supported client names:

    claude-code, kiro, cursor, codex, copilot-cli, continue

### `memryzed uninstall`

Remove the Memryzed binary and the PATH entry. By default, leaves data
and MCP integrations alone.

Flags:

    --purge             Also delete ~/.memryzed/ and all stored data.
    --unwire            Also remove Memryzed from MCP client configs.
    --yes               Do not prompt for confirmation.

### `memryzed update`

Check GitHub Releases for a newer version and install it if one is
found.

Flags:

    --check             Only check, do not install.
    --yes               Do not prompt for confirmation.

## Server command

### `memryzed serve`

Run as an MCP server over stdio. This is the command MCP clients
invoke when they spawn Memryzed. Users do not normally run this
themselves.

Flags:

    --transport <kind>   stdio (default) or sse (reserved for future use).

If the data directory has not been initialized, `serve` performs a
silent default initialization and continues. The initialization is
logged to the audit log.

## Memory commands

### `memryzed list`

List memories.

Flags:

    --scope <kind>      global | project | session | all (default: all).
    --project <id>      Filter to a specific project.
    --status <status>   pending | approved | pinned | archived (default: approved + pinned).
    --kind <kind>       preference | fact | decision | todo.
    --limit <n>         Maximum number of results.
    --json              Output as JSON.

### `memryzed show <id>`

Show full detail for a single memory: content, scope, source turn,
timestamps, embedding model used, and audit history.

### `memryzed search <query>`

Run a hybrid search and print results, identical to what an agent
would see from `recall`.

Flags:

    --scope <kind>      Restrict to a scope.
    --limit <n>         Maximum number of results (default: 10).
    --json              Output as JSON.

### `memryzed remember <text>`

Add a memory directly. Goes to approved status, since the user is
explicitly in the loop.

Flags:

    --scope <kind>      global | project | session (required).
    --kind <kind>       preference | fact | decision | todo (default: fact).
    --pin               Mark as pinned (will not expire).
    --ttl-days <n>      Expire after this many days.

### `memryzed forget <id>`

Archive a memory. Archived memories are excluded from retrieval but
are kept for audit. Use `memryzed forget <id> --hard` to permanently
delete.

### `memryzed pin <id>`

Mark a memory as pinned. Pinned memories never expire and are sorted
first in `list` output.

### `memryzed edit <id>`

Open the memory's content in `$EDITOR` for editing.

### `memryzed review`

Open the TUI to triage pending memories. Use the keyboard to approve,
edit, reject, or pin each candidate. See the keybindings inside the
TUI for the full set.

## Session commands

### `memryzed sessions`

List sessions for the current project (the project of the working
directory).

Flags:

    --project <id>      List sessions for a different project.
    --status <status>   active | paused | completed | archived.
    --limit <n>         Maximum number of results.
    --json              Output as JSON.

### `memryzed resume [<id>]`

Print the state of a session. Without an ID, prints the most recent
session for the current project. Use this for inspection or to pipe
into other tools; agents normally call `resume` through the MCP tool.

Flags:

    --json              Output the full state blob as JSON.

### `memryzed end-session <id>`

Mark a session as completed and archive its state.

## Project commands

### `memryzed projects`

List known projects.

Flags:

    --json              Output as JSON.

### `memryzed project show <id>`

Show full detail for a project: identity, paths it has been seen at,
memories scoped to it, and recent sessions.

## Diagnostic commands

### `memryzed doctor`

Run a series of health checks: binary location, PATH presence,
database integrity, embedding model availability, MCP client
integrations, and recent activity. Prints a summary and exits with
status zero on success or non-zero if any check fails.

### `memryzed log`

Print recent entries from the audit log.

Flags:

    -f, --follow        Stream new entries as they are written.
    --tail <n>          Show the last n entries (default: 50).
    --since <duration>  Show entries from the last duration (for example, 1h).
    --client <name>     Filter to a single client.
    --json              Output as JSON.

### `memryzed config`

Show or edit the configuration.

    memryzed config              Print the active configuration.
    memryzed config get <key>    Print a single key.
    memryzed config set <key> <value>  Set a single key.
    memryzed config edit         Open ~/.memryzed/config.toml in $EDITOR.

## Data commands

### `memryzed mine`

Ingest existing agent conversation transcripts into Memryzed. Each
transcript becomes a session record, and its user turns are run
through the extractor to propose candidate memories. Idempotent: a
transcript already mined is skipped unless `--force` is given.

    memryzed mine [<path>] [--source auto|kiro|claude-code] [--dry-run] [--force]

If `<path>` is omitted, the default location for the source is used:

    kiro          ~/.kiro/sessions
    claude-code   ~/.claude/projects

Flags:

    --source <name>     Transcript format. Default: auto (detects from path).
    --dry-run           Parse and report without writing anything.
    --force             Re-mine transcripts even if seen before.

### `memryzed hooks install`

Generate the Claude Code auto-save hook scripts under
`~/.memryzed/hooks/` and wire them into `~/.claude/settings.json`.
Two hooks are installed: a periodic checkpoint and a pre-compaction
hook. Existing settings and hooks are preserved; the settings file
is backed up first.

Flags:

    --yes               Do not prompt for confirmation.

### `memryzed hooks uninstall`

Remove Memryzed's hooks from Claude Code. Only Memryzed's entries
are removed; other hooks and settings are left untouched. The
generated scripts are kept on disk.

### `memryzed export`

Export all data to JSON on stdout.

Flags:

    --scope <kind>      Restrict to a scope.
    --project <id>      Restrict to a project.
    --pretty            Pretty-print the output.

### `memryzed import <file>`

Import data from a JSON file produced by `memryzed export`. By
default, merges with existing data using last-write-wins. The import
is idempotent on stable IDs.

Flags:

    --replace           Delete all existing data before importing.
    --dry-run           Report what would be imported without writing.
    --yes               Do not prompt for confirmation.

## Exit codes

    0   Success.
    1   General error.
    2   Misuse: bad arguments or unknown command.
    3   Configuration error.
    4   Storage error: database or filesystem problem.
    5   Network error: model download or update check failed.
    6   Integration error: an MCP client config could not be read or written.

## Environment variables

    MEMRYZED_DATA_DIR    Override the data directory.
    MEMRYZED_CONFIG      Override the config file path.
    MEMRYZED_LOG_LEVEL   trace | debug | info | warn | error (default: info).
    MEMRYZED_NO_COLOR    Set to any value to disable colored output.
    NO_COLOR             Standard environment variable to disable colored output.
    EDITOR               Editor to launch for `edit` and `config edit`.
