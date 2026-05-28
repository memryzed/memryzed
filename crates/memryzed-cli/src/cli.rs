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
//! v0.1.0-alpha.1 wires the full command tree from `docs/cli-reference.md`
//! but most subcommands return a "not yet implemented" error. The
//! commands that are wired for real in this alpha are:
//!
//! - `memryzed --version`
//! - `memryzed --help`
//! - `memryzed init`
//! - `memryzed doctor`
//!
//! Subsequent alphas fill in the rest, one slice per release.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

const LONG_ABOUT: &str = "\
Memryzed: persistent memory and session state for AI coding agents.

Memryzed runs as a local MCP server. Any MCP-aware client (Claude Code,
Kiro, Codex, Cursor, Copilot CLI, Continue) can use it for durable
memory and resumable session state.

This is a pre-release build. Storage, embeddings, and CLI memory
commands work; hybrid retrieval and the MCP server arrive in
subsequent alphas. See https://memryzed.com for the roadmap.";

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
///
/// Subcommands not yet implemented in v0.1.0-alpha.1 return a clear
/// "not yet implemented" error. They are still wired here so users
/// discover them through `--help` and so the CLI tree shape is
/// stable from the start.
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
    Sessions,

    /// Print or load a session's state.
    Resume {
        /// Session ID. Defaults to the most recent session.
        id: Option<String>,
    },

    /// Run health checks.
    Doctor,

    /// Print recent entries from the audit log.
    Log {
        /// Stream new entries as they are written.
        #[arg(short = 'f', long)]
        follow: bool,
    },

    /// Show, get, set, or edit configuration.
    Config,

    /// Export all data to JSON on stdout.
    Export,

    /// Import data from a JSON file produced by `memryzed export`.
    Import {
        /// Path to the export file.
        file: PathBuf,
    },
}
