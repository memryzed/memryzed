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

//! `memryzed reindex` implementation.
//!
//! Clears every episode's embedding and recomputes it with the
//! current model and embedding scheme. Use after an upgrade that
//! changes how episodes are embedded (such as the context-window
//! change) so existing memory benefits, not only newly captured
//! turns. The verbatim content is never touched.

use anyhow::Result;

use memryzed_core::embedder::make_default;
use memryzed_core::{episodes, Database};

use crate::commands::Context;
use crate::exit;

pub struct Args {
    /// Present for symmetry with the CLI flag; reindex always
    /// re-embeds every episode.
    pub all: bool,
}

pub fn run(ctx: &Context, args: Args) -> Result<()> {
    let _ = args.all;
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
    let profile = memryzed_core::engine::resolve_profile(&data_dir.config_file());
    let batch = profile.batch();

    if !embedder.is_active() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "embedding is disabled, so there is nothing to reindex",
        )
        .into());
    }

    let total = episodes::count(&db)?;
    if total == 0 {
        if !ctx.quiet {
            println!("No episodes to reindex.");
        }
        return Ok(());
    }

    let cleared = episodes::clear_embeddings(&mut db)?;
    if !ctx.quiet {
        println!(
            "Reindexing {cleared} episode{} with {} (context-window embedding). \
             This recomputes every vector and may take a few minutes.",
            if cleared == 1 { "" } else { "s" },
            embedder.model_id(),
        );
    }

    // Re-embed inline, in batches, until nothing is pending. Each
    // batch is its own transaction, so an interrupted run simply
    // leaves the rest for the background indexer or a later run.
    let mut done = 0usize;
    loop {
        let n = episodes::reindex_pending(&mut db, embedder.as_ref(), batch)?;
        if n == 0 {
            break;
        }
        done += n;
        if !ctx.quiet {
            print!("\r  embedded {done}/{cleared}");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }
    if !ctx.quiet {
        println!(
            "\rReindex complete: {done} episode{} embedded.        ",
            if done == 1 { "" } else { "s" }
        );
    }
    Ok(())
}
