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

//! `memryzed watch` implementation.
//!
//! Continuously captures memories from every detected agent. On each
//! interval it mines all known transcript directories incrementally:
//! only the bytes appended since the previous pass are parsed, so a
//! growing live transcript yields just its new turns. This is the
//! universal, agent-agnostic auto-capture path; it relies only on the
//! transcript files agents already write to disk, not on per-agent
//! hooks.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::embedder::make_default;
use memryzed_core::mining::{self, MineOptions, Source};
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub struct Args {
    pub interval: u64,
    pub once: bool,
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
    let home = home_dir()?;

    // Incremental + Auto: mine_all overrides the source per directory,
    // and the incremental flag makes each pass cheap.
    let opts = MineOptions {
        source: Source::Auto,
        threshold: 0.85,
        dry_run: false,
        force: false,
        incremental: true,
    };

    if !ctx.quiet {
        let detected: Vec<&str> = Source::all()
            .iter()
            .filter(|s| s.default_dir(&home).map(|d| d.is_dir()).unwrap_or(false))
            .map(|s| s.display_name())
            .collect();
        if detected.is_empty() {
            println!("No agent transcript directories detected. Nothing to watch.");
            return Ok(());
        }
        println!("Watching: {}", detected.join(", "));
        if args.once {
            println!("Running a single pass.");
        } else {
            println!("Polling every {}s. Press Ctrl-C to stop.", args.interval);
        }
    }

    loop {
        let now = now_epoch_seconds();
        let reports = mining::mine_all(&mut db, embedder.as_ref(), &home, &opts, now)?;
        let captured: usize = reports
            .iter()
            .map(|(_, r)| r.memories_approved + r.memories_pending)
            .sum();
        if captured > 0 && !ctx.quiet {
            for (src, r) in &reports {
                let n = r.memories_approved + r.memories_pending;
                if n > 0 {
                    println!(
                        "[{}] {}: captured {} ({} approved, {} pending)",
                        now,
                        src.display_name(),
                        n,
                        r.memories_approved,
                        r.memories_pending,
                    );
                }
            }
        }

        if args.once {
            break;
        }
        std::thread::sleep(Duration::from_secs(args.interval.max(1)));
    }

    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or_else(|| {
            exit::Coded::new(exit::GENERAL_ERROR, "could not determine home directory").into()
        })
        .map_err(|e: anyhow::Error| e)
}
