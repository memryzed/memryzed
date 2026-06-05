// Copyright 2026 Memryzed contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Command-line interface tree for the `memryzed` binary.
//!
//! The command surface is documented in `docs/cli-reference.md`.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

const LONG_ABOUT: &str = "\
Memryzed: persistent memory and session state for AI coding agents.

Memryzed runs as a local MCP server. Any MCP-aware client (Claude Code,
Kiro, Codex, Cursor, Copilot CLI, Continue) can use it for durable
memory and resumable session state. See https://memryzed.com.";

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "memryzed",
    version,
    about = "Persistent memory and session state for AI coding agents.",
    long_about = LONG_ABOUT,
    propagate_version = true
)]
pub struct Cli {
    /// Path to the configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Path to the data directory. Defaults to ~/.memryzed/.
    #[arg(long = "data-dir", global = true, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// Emit machine-readable JSON output where supported.
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-error output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable colored output.
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,

    /// Subcommand to run. If absent, prints help and exits.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Every subcommand the CLI supports.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize the data directory and configuration.
    Init {
        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Detect MCP clients and wire Memryzed into their configurations.
    Install {
        /// Limit to a single client.
        #[arg(long, value_name = "NAME")]
        client: Option<String>,

        /// Print configuration block instead of writing it.
        #[arg(long)]
        print: bool,

        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Remove Memryzed binary and PATH entry.
    Uninstall {
        /// Also delete the data directory.
        #[arg(long)]
        purge: bool,

        /// Also remove Memryzed from MCP client configurations.
        #[arg(long)]
        unwire: bool,

        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Check for updates and install the latest release.
    Update {
        /// Only check; do not install.
        #[arg(long)]
        check: bool,

        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Run as the MCP server over stdio.
    Serve,

    /// List memories.
    List {
        /// Restrict to a scope: global, project, session.
        #[arg(long, value_name = "KIND")]
        scope: Option<String>,

        /// Restrict to a specific project ID. Implies --scope project.
        #[arg(long, value_name = "ID")]
        project: Option<String>,

        /// Show memories with the given status. Repeatable.
        #[arg(long, value_name = "STATUS")]
        status: Vec<String>,

        /// Maximum number of results.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Show full detail for a single memory.
    Show {
        /// Memory ID.
        id: String,
    },

    /// Search memories with the same hybrid retrieval as `recall`.
    Search {
        /// Query string.
        query: String,

        /// Restrict to a scope: global, project, session.
        #[arg(long, value_name = "KIND")]
        scope: Option<String>,

        /// Maximum number of results.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Add a memory directly.
    Remember {
        /// Memory content.
        text: String,

        /// Scope to insert into: global, project, or session.
        #[arg(long, value_name = "KIND", default_value = "global")]
        scope: String,

        /// Kind of memory: preference, fact, decision, or todo.
        #[arg(long, value_name = "KIND", default_value = "fact")]
        kind: String,

        /// Pin the memory so it never expires.
        #[arg(long)]
        pin: bool,

        /// Expire after this many days.
        #[arg(long, value_name = "DAYS")]
        ttl_days: Option<u32>,
    },

    /// Archive a memory.
    Forget {
        /// Memory ID.
        id: String,

        /// Permanently delete instead of archiving.
        #[arg(long)]
        hard: bool,
    },

    /// Open the review TUI for pending memories.
    Review,

    /// List sessions for the current project.
    /// List sessions for the current project.
    Sessions {
        /// Maximum number of results.
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },

    /// Print or load a session's state.
    Resume {
        /// Session ID. Defaults to the most recent session.
        id: Option<String>,

        /// Output the full state blob as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Mark a session completed.
    #[command(name = "end-session")]
    EndSession {
        /// Session ID to end.
        id: String,
    },

    /// Run health checks.
    Doctor,

    /// Re-embed stored episodes with the current model and settings.
    ///
    /// Clears existing embeddings so the background indexer recomputes
    /// them. Use after an upgrade that changes how episodes are
    /// embedded (for example the context-window change) so existing
    /// memory benefits, not just newly captured turns.
    Reindex {
        /// Re-embed every episode, even those already embedded under
        /// the current model. This is the default for this command.
        #[arg(long)]
        all: bool,
    },

    /// Print recent entries from the audit log.
    Log {
        /// Stream new entries as they are written.
        #[arg(short = 'f', long)]
        follow: bool,

        /// Show the last N entries.
        #[arg(long, value_name = "N", default_value_t = 50)]
        tail: usize,

        /// Filter to a single client.
        #[arg(long, value_name = "NAME")]
        client: Option<String>,
    },

    /// Show, get, set, or edit configuration.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Ingest existing agent conversation transcripts into Memryzed.
    Mine {
        /// Path to a transcript file or a directory of transcripts.
        /// Defaults to the detected source's standard location.
        path: Option<PathBuf>,

        /// Transcript source format: auto, kiro, claude-code, copilot-cli.
        #[arg(long, value_name = "SOURCE", default_value = "auto")]
        source: String,

        /// Mine every detected agent transcript directory.
        #[arg(long)]
        all: bool,

        /// Parse and report without writing anything.
        #[arg(long)]
        dry_run: bool,

        /// Re-mine transcripts even if seen before.
        #[arg(long)]
        force: bool,
    },

    /// Install or remove Claude Code auto-save hooks.
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },

    /// Continuously capture memories from all detected agents.
    Watch {
        /// Seconds between polls.
        #[arg(long, value_name = "SECONDS", default_value = "15")]
        interval: u64,

        /// Run a single pass and exit instead of looping.
        #[arg(long)]
        once: bool,
    },

    /// Export all data to JSON on stdout.
    Export {
        /// Pretty-print the JSON output.
        #[arg(long)]
        pretty: bool,
    },

    /// Import data from a JSON file produced by `memryzed export`.
    Import {
        /// Path to the export file.
        file: PathBuf,

        /// Report what would be imported without writing.
        #[arg(long)]
        dry_run: bool,

        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// Subcommands for `memryzed config`.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print a single key's value.
    Get {
        /// Dotted key, for example memory.auto_approve_threshold.
        key: String,
    },
    /// Set a single key's value.
    Set {
        /// Dotted key, for example memory.auto_approve_threshold.
        key: String,
        /// New value. Coerced to bool/int/float when possible.
        value: String,
    },
    /// Open the configuration file in $EDITOR.
    Edit,
}

/// Subcommands for `memryzed hooks`.
#[derive(Debug, Subcommand)]
pub enum HooksAction {
    /// Generate the hook scripts and wire them into Claude Code.
    Install {
        /// Do not prompt for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Remove Memryzed hooks from Claude Code (scripts are kept).
    Uninstall,
}
