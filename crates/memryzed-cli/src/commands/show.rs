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

//! `memryzed show` implementation.

use anyhow::Result;

use memryzed_core::clock::format_epoch_iso;
use memryzed_core::memory::get_by_id;
use memryzed_core::Database;

use crate::commands::Context;
use crate::exit;

pub fn run(ctx: &Context, id: String) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }
    let db = Database::open(&data_dir.db_file())?;

    let memory = match get_by_id(&db, &id)? {
        Some(m) => m,
        None => {
            return Err(
                exit::Coded::new(exit::GENERAL_ERROR, format!("memory {id} not found")).into(),
            );
        }
    };

    if !ctx.quiet {
        println!("Memory {}", memory.id);
        println!();
        println!("  Scope          {}", memory.scope);
        if let Some(scope_id) = &memory.scope_id {
            println!("  Scope ID       {scope_id}");
        }
        println!("  Kind           {}", memory.kind);
        println!("  Status         {}", memory.status);
        println!(
            "  Pinned         {}",
            if memory.pinned { "yes" } else { "no" }
        );
        if let Some(c) = memory.confidence {
            println!("  Confidence     {c:.2}");
        }
        println!("  Created        {}", format_epoch_iso(memory.created_at));
        println!("  Updated        {}", format_epoch_iso(memory.updated_at));
        if let Some(exp) = memory.expires_at {
            println!("  Expires        {}", format_epoch_iso(exp));
        }
        if let Some(turn) = &memory.source_turn_id {
            println!("  Source turn    {turn}");
        }
        if let Some(client) = &memory.source_client {
            println!("  Source client  {client}");
        }
        println!();
        println!("  {}", memory.content);
    }
    Ok(())
}
