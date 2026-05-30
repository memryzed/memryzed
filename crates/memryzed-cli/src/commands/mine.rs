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

//! `memryzed mine` implementation.
//!
//! Ingests existing conversation transcripts into Memryzed. With no
//! path, mines the detected source's default transcript location.
//! With `--all`, mines every detected agent transcript directory.

use std::path::PathBuf;

use anyhow::Result;

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::embedder::make_default;
use memryzed_core::mining::{self, MineOptions, Source};
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub struct Args {
    pub path: Option<PathBuf>,
    pub source: String,
    pub all: bool,
    pub dry_run: bool,
    pub force: bool,
}

pub fn run(ctx: &Context, args: Args) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }

    let mut db = Database::open(&data_dir.db_file())?;
    let embedder = make_default(&data_dir.models_dir())?;
    let now = now_epoch_seconds();

    // `--all` walks the registry of every detected agent dir.
    if args.all {
        let home = home_dir()?;
        let opts = MineOptions {
            source: Source::Auto,
            threshold: 0.85,
            dry_run: args.dry_run,
            force: args.force,
            incremental: false,
        };
        if !ctx.quiet && !args.dry_run {
            println!(
                "Mining all detected agents. The first run embeds your history and \
                 can take a few minutes; later runs are incremental and fast."
            );
        }
        let reports = mining::mine_all(&mut db, embedder.as_ref(), &home, &opts, now)?;
        if !ctx.quiet {
            if reports.is_empty() {
                println!("No agent transcript directories detected.");
            } else {
                let mode = if args.dry_run { " (dry run)" } else { "" };
                println!("Mined all detected agents{mode}");
                for (src, r) in &reports {
                    println!(
                        "  {:<14} found {:>4}  mined {:>4}  facts {:>3}  episodes {:>5}",
                        src.display_name(),
                        r.files_found,
                        r.files_mined,
                        r.memories_approved + r.memories_pending,
                        r.episodes_captured,
                    );
                }
            }
        }
        return Ok(());
    }

    let source: Source = args
        .source
        .parse()
        .map_err(|e: memryzed_core::Error| exit::Coded::new(exit::MISUSE, e.to_string()))?;

    let path = match args.path {
        Some(p) => p,
        None => default_path_for(source).ok_or_else(|| {
            exit::Coded::new(
                exit::MISUSE,
                "no path given and no default location for source 'auto'; \
                 pass a path, use --all, or set --source kiro|claude-code|copilot-cli",
            )
        })?,
    };

    let opts = MineOptions {
        source,
        threshold: 0.85,
        dry_run: args.dry_run,
        force: args.force,
        incremental: false,
    };

    let report = mining::mine(&mut db, embedder.as_ref(), &path, &opts, now)?;

    if !ctx.quiet {
        let mode = if args.dry_run { " (dry run)" } else { "" };
        println!("Mined {}{}", path.display(), mode);
        println!("  transcripts found:   {}", report.files_found);
        println!("  transcripts mined:   {}", report.files_mined);
        println!("  skipped (no new):    {}", report.files_skipped);
        println!("  sessions written:    {}", report.sessions_written);
        println!("  memories approved:   {}", report.memories_approved);
        println!("  memories pending:    {}", report.memories_pending);
        println!("  episodes captured:   {}", report.episodes_captured);
        if report.memories_pending > 0 {
            println!();
            println!("Review pending candidates with `memryzed review`.");
        }
    }
    Ok(())
}

/// Default transcript location for a source, when the user gives no
/// path. `Auto` has no default; the caller errors in that case.
fn default_path_for(source: Source) -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    source.default_dir(&home)
}

fn home_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or_else(|| {
            exit::Coded::new(exit::GENERAL_ERROR, "could not determine home directory").into()
        })
        .map_err(|e: anyhow::Error| e)
}
