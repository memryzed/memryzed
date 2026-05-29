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

//! `memryzed export` and `memryzed import` implementations.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::export::{apply, build, read_from_file, to_compact_json, to_pretty_json};
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub struct ExportArgs {
    pub pretty: bool,
}

pub fn export(ctx: &Context, args: ExportArgs) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }
    let db = Database::open(&data_dir.db_file())?;
    let dump = build(&db, now_epoch_seconds())?;
    let json = if args.pretty {
        to_pretty_json(&dump)?
    } else {
        to_compact_json(&dump)?
    };
    // Export writes to stdout regardless of --quiet; it is the
    // command's entire purpose.
    println!("{json}");
    if !ctx.quiet {
        eprintln!(
            "Exported {} memories and {} projects.",
            dump.memories.len(),
            dump.projects.len()
        );
    }
    Ok(())
}

pub struct ImportArgs {
    pub file: PathBuf,
    pub dry_run: bool,
    pub yes: bool,
}

pub fn import(ctx: &Context, args: ImportArgs) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }
    let dump =
        read_from_file(&args.file).with_context(|| format!("reading {}", args.file.display()))?;

    if !ctx.quiet {
        println!(
            "Import from {}: {} memories, {} projects (export v{}, from {}).",
            args.file.display(),
            dump.memories.len(),
            dump.projects.len(),
            dump.memryzed_export.version,
            dump.memryzed_export.source_version,
        );
    }

    if args.dry_run {
        if !ctx.quiet {
            println!("Dry run: no changes written.");
        }
        return Ok(());
    }

    if !args.yes && !ctx.quiet {
        println!("Merging into existing data (newer records win). Re-run with --yes to confirm.");
        return Ok(());
    }

    let mut db = Database::open(&data_dir.db_file())?;
    let summary = apply(&mut db, &dump)?;

    if !ctx.quiet {
        println!(
            "Imported: {} new memories, {} updated, {} skipped; {} new projects, {} updated.",
            summary.memories_inserted,
            summary.memories_updated,
            summary.memories_skipped,
            summary.projects_inserted,
            summary.projects_updated,
        );
        println!("Note: embeddings are regenerated lazily; run a search to warm them.");
    }
    Ok(())
}
