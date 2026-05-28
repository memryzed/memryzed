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

//! `memryzed search` implementation.

use anyhow::Result;

use memryzed_core::clock::format_epoch_iso;
use memryzed_core::embedder::make_default;
use memryzed_core::memory::Scope;
use memryzed_core::retrieval::{search, SearchOptions};
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub struct Args {
    pub query: String,
    pub scope: Option<Scope>,
    pub limit: Option<u32>,
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
    let db = Database::open(&data_dir.db_file())?;
    let embedder = make_default(&data_dir.models_dir())?;

    let opts = SearchOptions {
        scope: args.scope,
        scope_id: None,
        limit: args
            .limit
            .map(|n| n as usize)
            .unwrap_or(memryzed_core::retrieval::DEFAULT_LIMIT),
        ..Default::default()
    };

    let results = search(&db, embedder.as_ref(), &args.query, &opts)?;

    if results.is_empty() {
        if !ctx.quiet {
            println!("No matches.");
        }
        return Ok(());
    }

    if !ctx.quiet {
        for r in &results {
            let pin = if r.memory.pinned { "*" } else { " " };
            let scope_label = match (&r.memory.scope, &r.memory.scope_id) {
                (Scope::Global, _) => "global".to_string(),
                (Scope::Project, Some(id)) => format!("project:{id}"),
                (Scope::Session, Some(id)) => format!("session:{id}"),
                _ => "?".to_string(),
            };
            let vec_part = r
                .vector_score
                .map(|s| format!("v{:.2}", s))
                .unwrap_or_else(|| "v-".into());
            let fts_part = r
                .fts_score
                .map(|s| format!("f{:.2}", s))
                .unwrap_or_else(|| "f-".into());
            println!(
                "{pin} {score:.2}  [{vec_part} {fts_part} r{rec:.2}]  {id}  {scope:<20}  {created}  {content}",
                score = r.score,
                rec = r.recency_score,
                id = r.memory.id,
                scope = scope_label,
                created = format_epoch_iso(r.memory.created_at),
                content = truncate(&r.memory.content, 60),
            );
        }
        println!();
        println!(
            "{} result{}",
            results.len(),
            if results.len() == 1 { "" } else { "s" }
        );
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 3).collect();
        out.push_str("...");
        out
    }
}
