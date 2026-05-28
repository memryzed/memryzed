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

//! `memryzed list` implementation.

use anyhow::Result;

use memryzed_core::clock::format_epoch_iso;
use memryzed_core::memory::{list, ListFilter, Scope, Status};
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub struct Args {
    pub scope: Option<Scope>,
    pub project: Option<String>,
    pub statuses: Vec<Status>,
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

    let scope = args.scope.or_else(|| {
        if args.project.is_some() {
            Some(Scope::Project)
        } else {
            None
        }
    });

    let filter = ListFilter {
        scope,
        scope_id: args.project,
        statuses: if args.statuses.is_empty() {
            vec![Status::Approved, Status::Pinned]
        } else {
            args.statuses
        },
        limit: args.limit,
    };

    let memories = list(&db, &filter)?;

    if memories.is_empty() {
        if !ctx.quiet {
            println!("No memories match the filter.");
        }
        return Ok(());
    }

    if !ctx.quiet {
        for m in &memories {
            let pin = if m.pinned { "*" } else { " " };
            let scope_label = match (&m.scope, &m.scope_id) {
                (Scope::Global, _) => "global".to_string(),
                (Scope::Project, Some(id)) => format!("project:{id}"),
                (Scope::Session, Some(id)) => format!("session:{id}"),
                _ => "?".to_string(),
            };
            println!(
                "{pin} {id}  {scope:<20}  {created}  {content}",
                id = m.id,
                scope = scope_label,
                created = format_epoch_iso(m.created_at),
                content = truncate(&m.content, 80),
            );
        }
        println!();
        println!(
            "{} memor{}",
            memories.len(),
            if memories.len() == 1 { "y" } else { "ies" }
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
