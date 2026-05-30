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

//! `memryzed hooks install` and `memryzed hooks uninstall`.
//!
//! Generates the Claude Code auto-save hook scripts and wires them
//! into `~/.claude/settings.json`. The settings file is backed up
//! before any write.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

use memryzed_core::hooks;

use crate::commands::Context;
use crate::exit;

pub fn install(ctx: &Context, yes: bool) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let binary_path = current_exe_path()?;
    let home = home_dir()?;
    let settings_path = hooks::claude_settings_path(&home);

    if !home.join(".claude").is_dir() {
        return Err(exit::Coded::new(
            exit::INTEGRATION_ERROR,
            "Claude Code not detected (~/.claude is missing); nothing to wire",
        )
        .into());
    }

    let (periodic, precompact) = hooks::write_scripts(data_dir.root(), &binary_path)?;

    if !yes && !ctx.quiet {
        println!("Memryzed will install two Claude Code hooks:");
        println!("  periodic checkpoint -> {}", periodic.display());
        println!("  pre-compaction      -> {}", precompact.display());
        println!("into {}", settings_path.display());
        println!("Existing settings are backed up. Re-run with --yes in scripts.");
        println!();
    }

    let settings = hooks::read_settings(&settings_path)?;
    let updated = hooks::merge_into_settings(settings, data_dir.root())?;
    backup_if_exists(&settings_path)?;
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(&updated)
        .map_err(|e| exit::Coded::new(exit::INTEGRATION_ERROR, e.to_string()))?;
    std::fs::write(&settings_path, format!("{pretty}\n"))
        .with_context(|| format!("writing {}", settings_path.display()))?;

    if !ctx.quiet {
        println!("Installed Claude Code auto-save hooks.");
        println!("Restart Claude Code for the hooks to take effect.");
    }
    Ok(())
}

pub fn uninstall(ctx: &Context) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let home = home_dir()?;
    let settings_path = hooks::claude_settings_path(&home);

    if !settings_path.is_file() {
        if !ctx.quiet {
            println!("No Claude Code settings found; nothing to remove.");
        }
        return Ok(());
    }

    let mut settings = hooks::read_settings(&settings_path)?;
    if hooks::remove_from_settings(&mut settings, data_dir.root()) {
        backup_if_exists(&settings_path)?;
        let pretty = serde_json::to_string_pretty(&settings)
            .map_err(|e| exit::Coded::new(exit::INTEGRATION_ERROR, e.to_string()))?;
        std::fs::write(&settings_path, format!("{pretty}\n"))?;
        if !ctx.quiet {
            println!("Removed Memryzed hooks from Claude Code.");
        }
    } else if !ctx.quiet {
        println!("No Memryzed hooks were present.");
    }
    Ok(())
}

fn backup_if_exists(path: &std::path::Path) -> Result<()> {
    if path.is_file() {
        let backup = path.with_extension("json.memryzed.bak");
        std::fs::copy(path, &backup)?;
    }
    Ok(())
}

fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("determining the memryzed executable path")
}

fn home_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or_else(|| {
            exit::Coded::new(exit::GENERAL_ERROR, "could not determine home directory").into()
        })
        .map_err(|e: anyhow::Error| e)
}
