# Configuration

Memryzed is configured through `~/.memryzed/config.toml`. This file is
created with defaults during `memryzed init` and can be edited at any
time. The server picks up changes on its next start; running clients
need to be restarted to see configuration changes.

You can also read or change settings without opening the file:

    memryzed config                  Print the active configuration.
    memryzed config get <key>        Print a single key.
    memryzed config set <key> <value>  Set a single key.
    memryzed config edit             Open the file in $EDITOR.

## Default configuration

The default `config.toml` after `memryzed init`:

    # Memryzed configuration.
    # See docs/configuration.md for the full reference.

    [general]
    data_dir = "~/.memryzed"
    log_level = "info"

    [server]
    rate_limit_recall_per_minute = 60
    rate_limit_remember_per_minute = 30
    rate_limit_other_per_minute = 60

    [memory]
    auto_approve_threshold = 0.85
    default_ttl_days = 0          # 0 means never expire by default
    pinned_count_limit = 0        # 0 means unlimited

    [retrieval]
    max_results = 10
    vector_weight = 0.6
    fts_weight = 0.3
    recency_weight = 0.1
    embedding_model = "bge-small-en-v1.5"

    [index]
    profile = "gentle"            # gentle | balanced | fast

    [extractor]
    enabled = true
    rule_based = true
    ollama_enabled = false
    ollama_url = "http://localhost:11434"
    ollama_model = "qwen2.5:3b"

    [updates]
    check_on_startup = true
    auto_install = false

    [telemetry]
    enabled = false

    [paths]
    db_file = "db.sqlite"
    models_dir = "models"
    audit_log = "audit.log"

## Sections in detail

### `[index]`

Controls how hard the background embedding engine works. Embedding is
the only CPU-intensive part of Memryzed; the profile trades CPU for how
quickly a backlog of conversation is embedded. Only one engine runs at
a time even with several agent sessions open (the first `serve` process
acquires a single-instance lock), so this is the one knob that governs
background CPU.

    profile    string. One of:
                 gentle   (default) small batches, long pauses. Stays a
                          fraction of one core; a large first-time
                          backlog takes longer but you never notice it.
                 balanced larger batches, short pauses.
                 fast     large batches, no pause. Embeds a backlog
                          quickly using noticeably more CPU.

               Override for one run with the MEMRYZED_INDEX_PROFILE
               environment variable. Set persistently with:
                 memryzed config set index.profile fast

               The active profile is shown by `memryzed doctor`.

### `[general]`

    data_dir       string. The data directory. Tilde is expanded. Can also
                   be overridden via the MEMRYZED_DATA_DIR environment
                   variable or the --data-dir flag.

    log_level      string. One of trace, debug, info, warn, error. Can also
                   be overridden via MEMRYZED_LOG_LEVEL.

### `[server]`

Soft rate limits applied per MCP client per minute. When exceeded, the
next call returns a `rate_limited` error and the event is logged. Set
any value to `0` to disable that limit, but note that this may allow
runaway loops to consume resources.

    rate_limit_recall_per_minute       integer. Default: 60.
    rate_limit_remember_per_minute     integer. Default: 30.
    rate_limit_other_per_minute        integer. Default: 60.

### `[memory]`

    auto_approve_threshold     float, 0.0 to 1.0. Memories the extractor
                               proposes with a confidence score at or above
                               this value are auto-approved. Below this
                               threshold, they go to the pending queue. Set
                               to 1.0 to disable auto-approval entirely.

    default_ttl_days           integer. Default time-to-live for new
                               memories in days. 0 means never expire.

    pinned_count_limit         integer. Maximum number of pinned memories
                               per scope. 0 means unlimited.

### `[retrieval]`

    max_results        integer. Default number of results returned by
                       `recall` when the client does not specify a limit.

    vector_weight      float. Weight applied to vector similarity in the
                       hybrid score.

    fts_weight         float. Weight applied to BM25 full-text scores.

    recency_weight     float. Weight applied to a recency boost. Higher
                       values prefer newer memories.

    embedding_model    string. Identifier of the embedding model to use.
                       Currently only "bge-small-en-v1.5" is shipped. The
                       choice of model is part of the database schema; if
                       you change models, existing embeddings need to be
                       regenerated. Memryzed does this automatically on
                       next start, but it can take time on a large store.

### `[extractor]`

Controls the background process that proposes memories from agent
turns.

    enabled           boolean. Master switch for the extractor.

    rule_based        boolean. Use the built-in pattern-based extractor.

    ollama_enabled    boolean. Use a local Ollama instance for richer
                      extraction. Off by default.

    ollama_url        string. URL of the Ollama HTTP API.

    ollama_model      string. Ollama model tag to use for extraction.
                      A small instruction-tuned model is recommended;
                      the default of qwen2.5:3b uses about 2 GB of memory.

When `ollama_enabled` is true and the Ollama API is unreachable, the
extractor falls back to rule-based extraction and logs the failure.

### `[updates]`

    check_on_startup   boolean. Check GitHub Releases on `serve` startup,
                       at most once per day, and print a notice if an
                       update is available. Does not block startup.

    auto_install       boolean. If true, attempt to install updates
                       automatically. Disabled by default for safety.

### `[telemetry]`

    enabled    boolean. Off by default. When enabled, Memryzed sends
               anonymized usage counters (number of recalls, number of
               memories, number of sessions, OS, version) to the project
               telemetry endpoint. No memory contents, queries, or
               filenames are ever sent. The exact payload shape is
               documented in `docs/architecture.md`.

### `[paths]`

Optional overrides for the locations of files within `data_dir`.
Useful only if you want to keep certain artifacts on a different disk.
Most users should leave these at their defaults.

## Per-machine vs. per-user

Memryzed has no system-wide configuration in v1. Every install is
per-user. If you want a different configuration on a different
machine, configure each one separately. Future cloud sync will provide
a way to share configuration across machines, but the local file
remains the source of truth.

## Environment variables

The following environment variables override configuration values at
runtime. Each one takes precedence over `config.toml`:

    MEMRYZED_DATA_DIR       Overrides general.data_dir.
    MEMRYZED_CONFIG         Path to a different config file.
    MEMRYZED_LOG_LEVEL      Overrides general.log_level.
    MEMRYZED_NO_COLOR       Disables colored output. NO_COLOR is also honored.

Command-line flags override environment variables. The precedence
order from lowest to highest is:

    defaults < config.toml < environment variables < command-line flags

## Migrating configuration between releases

When a new release adds a configuration option, the upgrade path is:

1. The new release reads your existing `config.toml` and applies new
   options at their default values.
2. `memryzed config edit` will show the active configuration. Any new
   options that have not been written to disk are visible there.
3. You can persist the new defaults by running `memryzed config set
   <key> <value>` for each new key, or by manually editing the file.

When a release removes or renames an option, that change is documented
in the `Changed`, `Deprecated`, or `Removed` section of the
`CHANGELOG.md` for that release. Memryzed will not silently rename
keys.

## Resetting to defaults

To reset configuration to defaults:

    memryzed config edit
    # Delete the file or replace its contents.
    # On next start, defaults are restored.

To reset all data and configuration:

    memryzed uninstall --purge
    memryzed init
