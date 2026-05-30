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
                 pass a path or --source kiro|claude-code",
            )
        })?,
    };

    let mut db = Database::open(&data_dir.db_file())?;
    let embedder = make_default(&data_dir.models_dir())?;
    let opts = MineOptions {
        source,
        threshold: 0.85,
        dry_run: args.dry_run,
        force: args.force,
    };

    let report = mining::mine(
        &mut db,
        embedder.as_ref(),
        &path,
        &opts,
        now_epoch_seconds(),
    )?;

    if !ctx.quiet {
        let mode = if args.dry_run { " (dry run)" } else { "" };
        println!("Mined {}{}", path.display(), mode);
        println!("  transcripts found:   {}", report.files_found);
        println!("  transcripts mined:   {}", report.files_mined);
        println!("  already seen:        {}", report.files_skipped);
        println!("  sessions written:    {}", report.sessions_written);
        println!("  memories approved:   {}", report.memories_approved);
        println!("  memories pending:    {}", report.memories_pending);
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
    match source {
        Source::Kiro => Some(home.join(".kiro").join("sessions")),
        Source::ClaudeCode => Some(home.join(".claude").join("projects")),
        Source::Auto => None,
    }
}
