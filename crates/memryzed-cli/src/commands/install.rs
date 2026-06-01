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

//! `memryzed install` and `memryzed uninstall`.
//!
//! Auto-detect MCP-aware clients on the user's machine and write the
//! Memryzed entry into each client's configuration. See
//! `memryzed-core::integrations` for the adapter list.

use std::env;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

use memryzed_core::integrations::{
    self, all, by_id, install_one, render_entry, uninstall_one, InstallOutcome, UninstallOutcome,
};

use crate::commands::Context;
use crate::exit;

pub struct InstallArgs {
    pub client: Option<String>,
    pub print: bool,
    pub yes: bool,
}

pub struct UninstallArgs {
    pub purge: bool,
    pub unwire: bool,
    pub yes: bool,
}

pub fn install(ctx: &Context, args: InstallArgs) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let binary_path = current_exe_path()?;
    let home = home_dir()?;

    if args.print {
        let target_id = args.client.as_deref();
        for adapter in selected_adapters(target_id)? {
            if !ctx.quiet {
                println!(
                    "# {} -> {}",
                    adapter.display_name(),
                    adapter.config_path(&home).display()
                );
                println!("{}", render_entry(&binary_path));
                println!();
            }
        }
        return Ok(());
    }

    let target_id = args.client.as_deref();
    let adapters = selected_adapters(target_id)?;

    if !ctx.quiet {
        println!("Memryzed integration");
        println!();
        println!("Scanning for MCP-aware clients...");
        for adapter in &adapters {
            let mark = if adapter.is_present(&home) {
                "yes"
            } else {
                "no "
            };
            println!(
                "  [{mark}] {:<14} {}",
                adapter.display_name(),
                adapter.config_path(&home).display()
            );
        }
        println!();
    }

    if !args.yes && !ctx.quiet {
        println!(
            "Will write Memryzed into the present clients above (existing configs are backed up)."
        );
        println!("Re-run with --yes to skip this message in scripts.");
        println!();
    }

    let mut wrote_any = false;
    for adapter in &adapters {
        let outcome = install_one(adapter.as_ref(), &home, &binary_path).with_context(|| {
            format!(
                "writing Memryzed into {}",
                adapter.config_path(&home).display()
            )
        })?;
        if !ctx.quiet {
            let label = match outcome {
                InstallOutcome::Added => "added",
                InstallOutcome::AlreadyPresent => "ok (already present)",
                InstallOutcome::Updated => "updated",
                InstallOutcome::NotPresent => "skip (not installed)",
            };
            println!("  {} -> {}", adapter.display_name(), label);
        }
        if matches!(outcome, InstallOutcome::Added | InstallOutcome::Updated) {
            wrote_any = true;
        }

        // Write the always-on steering rule so the agent uses
        // Memryzed proactively even if it ignores the MCP server's
        // instructions field. Only for present clients that support
        // a steering mechanism.
        if !matches!(outcome, InstallOutcome::NotPresent) {
            match integrations::write_steering(adapter.as_ref(), &home) {
                Ok(integrations::SteeringOutcome::Written)
                | Ok(integrations::SteeringOutcome::Updated) => {
                    if let Some(p) = adapter.steering_path(&home) {
                        if !ctx.quiet {
                            println!("      steering rule -> {}", p.display());
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    if !ctx.quiet {
                        println!("      steering rule skipped: {e}");
                    }
                }
            }

            // Auto-approve the Memryzed tools so the user is not
            // prompted on every call. Scoped to memryzed only.
            match integrations::write_auto_approve(adapter.as_ref(), &home) {
                Ok(integrations::AutoApproveOutcome::Written) => {
                    if !ctx.quiet {
                        println!("      auto-approve memryzed tools -> enabled");
                    }
                }
                Ok(integrations::AutoApproveOutcome::Unsupported) => {
                    if !ctx.quiet {
                        if let Some(hint) = auto_approve_hint(adapter.id()) {
                            println!("      auto-approve: {hint}");
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    if !ctx.quiet {
                        println!("      auto-approve skipped: {e}");
                    }
                }
            }

            // Claude Code: also wire the auto-save hooks so each turn
            // (and pre-compaction) is captured before Claude prunes
            // its transcripts. This is what makes Claude history
            // durable without a separate `hooks install` command.
            if adapter.id() == "claude-code" {
                match install_claude_hooks(&data_dir, &binary_path, &home) {
                    Ok(true) => {
                        if !ctx.quiet {
                            println!("      auto-save hooks -> enabled");
                        }
                    }
                    Ok(false) => {}
                    Err(e) => {
                        if !ctx.quiet {
                            println!("      auto-save hooks skipped: {e}");
                        }
                    }
                }
            }
        }
    }

    if !ctx.quiet {
        println!();
        if wrote_any {
            println!("Restart your agent for changes to take effect.");
        } else {
            println!("No changes were needed.");
        }
        println!("Verify with: memryzed doctor");
    }
    Ok(())
}

/// Wire Claude Code's auto-save hooks (periodic + pre-compaction) so
/// each turn is captured before Claude prunes or compacts its
/// transcripts. Returns whether settings were written. Reuses the
/// same logic as `memryzed hooks install`.
fn install_claude_hooks(
    data_dir: &memryzed_core::DataDir,
    binary_path: &std::path::Path,
    home: &std::path::Path,
) -> Result<bool> {
    use memryzed_core::hooks;
    hooks::write_scripts(data_dir.root(), binary_path)?;
    let settings_path = hooks::claude_settings_path(home);
    let settings = hooks::read_settings(&settings_path)?;
    let updated = hooks::merge_into_settings(settings, data_dir.root())?;
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if settings_path.is_file() {
        let backup = settings_path.with_extension("json.memryzed.bak");
        std::fs::copy(&settings_path, &backup)?;
    }
    let pretty = serde_json::to_string_pretty(&updated)?;
    std::fs::write(&settings_path, format!("{pretty}\n"))?;
    Ok(true)
}

/// One-line manual instruction for clients whose tool trust cannot be
/// set from a config file we own.
fn auto_approve_hint(id: &str) -> Option<&'static str> {
    match id {
        "codex" => Some("Codex approves per-session; run with --full-auto or trust the repo to skip prompts"),
        "continue" => Some("run with `cn --auto`, or choose \"don't ask again\" once to persist in ~/.continue/permissions.yaml"),
        _ => None,
    }
}

pub fn uninstall(ctx: &Context, args: UninstallArgs) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let home = home_dir()?;

    if !ctx.quiet {
        println!("Memryzed uninstall");
        println!();
    }

    if args.unwire {
        if !ctx.quiet {
            println!("Removing Memryzed from MCP client configs...");
        }
        for adapter in all() {
            let outcome = uninstall_one(adapter.as_ref(), &home).with_context(|| {
                format!(
                    "removing Memryzed from {}",
                    adapter.config_path(&home).display()
                )
            })?;
            if !ctx.quiet {
                let label = match outcome {
                    UninstallOutcome::Removed => "removed",
                    UninstallOutcome::NotPresent => "skip (not present)",
                };
                println!("  {} -> {}", adapter.display_name(), label);
            }
        }
    } else if !ctx.quiet {
        println!("MCP client configs left in place. Pass --unwire to remove them.");
    }

    if args.purge {
        if !ctx.quiet {
            println!();
            if !args.yes {
                println!(
                    "About to delete {}. Re-run with --yes to confirm.",
                    data_dir.root().display()
                );
                return Ok(());
            }
            println!("Deleting {}", data_dir.root().display());
        } else if !args.yes {
            return Err(exit::Coded::new(
                exit::MISUSE,
                "uninstall --purge requires --yes when running quietly",
            )
            .into());
        }
        if data_dir.root().exists() {
            std::fs::remove_dir_all(data_dir.root())
                .with_context(|| format!("removing {}", data_dir.root().display()))?;
        }
    } else if !ctx.quiet {
        println!();
        println!(
            "Data directory left at {}. Pass --purge to delete it.",
            data_dir.root().display()
        );
    }

    if !ctx.quiet {
        println!();
        println!(
            "Note: this command does not delete the binary at {}.",
            current_exe_path().unwrap_or_default().display()
        );
        println!("Remove it manually if you want a complete uninstall.");
    }
    Ok(())
}

fn selected_adapters(id: Option<&str>) -> Result<Vec<Box<dyn integrations::Adapter>>> {
    if let Some(id) = id {
        let adapter = by_id(id).ok_or_else(|| {
            exit::Coded::new(
                exit::MISUSE,
                format!(
                    "unknown client {id:?}; expected one of: claude-code, kiro, cursor, codex, continue"
                ),
            )
        })?;
        Ok(vec![adapter])
    } else {
        Ok(all())
    }
}

fn home_dir() -> Result<PathBuf> {
    memryzed_core::paths::home_dir().map_err(|e| {
        exit::Coded::new(
            exit::CONFIG_ERROR,
            format!("could not determine the user's home directory: {e}"),
        )
        .into()
    })
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe().map_err(|e| {
        exit::Coded::new(
            exit::CONFIG_ERROR,
            format!("could not determine current executable path: {e}"),
        )
        .into()
    })
}
