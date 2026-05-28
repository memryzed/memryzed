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

//! Command dispatch and implementations.

mod doctor;
mod init;

use anyhow::Result;

use crate::cli::{Cli, Command};
use crate::exit;

/// Resolve subcommand and run it.
///
/// When no subcommand is supplied, prints help and returns success.
/// Subcommands not yet implemented in v0.1.0-alpha.1 return an error
/// of kind [`crate::exit::Coded`] with a clear message.
pub fn dispatch(cli: Cli) -> Result<()> {
    let Some(command) = cli.command else {
        // No subcommand: print help and exit zero.
        use clap::CommandFactory;
        Cli::command().print_help().ok();
        println!();
        return Ok(());
    };

    let context = Context {
        data_dir_override: cli.data_dir,
        quiet: cli.quiet,
        json: cli.json,
    };

    match command {
        Command::Init { yes } => init::run(&context, yes),
        Command::Doctor => doctor::run(&context),

        Command::Install { .. }
        | Command::Uninstall { .. }
        | Command::Update { .. }
        | Command::Serve
        | Command::List
        | Command::Show { .. }
        | Command::Search { .. }
        | Command::Remember { .. }
        | Command::Forget { .. }
        | Command::Review
        | Command::Sessions
        | Command::Resume { .. }
        | Command::Log { .. }
        | Command::Config
        | Command::Export
        | Command::Import { .. } => Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "this command is not yet implemented in v0.1.0-alpha.1",
        )
        .into()),
    }
}

/// Shared context passed to every command implementation.
pub struct Context {
    /// Optional override of the data directory from `--data-dir`.
    pub data_dir_override: Option<std::path::PathBuf>,
    /// True when `--quiet` is set.
    pub quiet: bool,
    /// True when `--json` is set.
    ///
    /// Reserved for the commands that produce machine-readable output
    /// (`list`, `search`, `show`, `log`, `export`). Those land in
    /// later alphas; for now the field exists so the CLI parses the
    /// flag and rejects misuse correctly.
    #[allow(dead_code)]
    pub json: bool,
}

impl Context {
    /// Resolve the data directory respecting the CLI override.
    pub fn data_dir(&self) -> Result<memryzed_core::DataDir> {
        if let Some(p) = &self.data_dir_override {
            return Ok(memryzed_core::DataDir::at(p));
        }
        memryzed_core::DataDir::resolve().map_err(Into::into)
    }
}
