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

//! `memryzed update` implementation.
//!
//! Checks GitHub Releases for a newer version. With `--check`, only
//! reports. Otherwise, prints the command to install the new
//! version (re-running the install script), since the binary swap
//! is owned by the installer rather than the running process.

use anyhow::Result;

use memryzed_core::update::{check, UpdateStatus};

use crate::commands::Context;

pub struct Args {
    pub check_only: bool,
}

pub fn run(ctx: &Context, args: Args) -> Result<()> {
    let status = check();
    match status {
        UpdateStatus::UpToDate { current } => {
            if !ctx.quiet {
                println!("Memryzed {current} is the latest version.");
            }
        }
        UpdateStatus::Available { current, latest } => {
            if !ctx.quiet {
                println!("Update available: {current} -> {latest}");
                println!(
                    "Release notes: https://github.com/memryzed/memryzed/releases/tag/v{latest}"
                );
                if args.check_only {
                    println!("Run `memryzed update` without --check to see how to install.");
                } else {
                    println!();
                    println!("To update, re-run the installer:");
                    println!("  curl -fsSL https://memryzed.com/install.sh | bash");
                    println!("Or on Windows PowerShell:");
                    println!("  irm https://memryzed.com/install.ps1 | iex");
                }
            }
        }
        UpdateStatus::Unknown { current, reason } => {
            if !ctx.quiet {
                println!("Memryzed {current}; could not check for updates: {reason}");
            }
        }
    }
    Ok(())
}
