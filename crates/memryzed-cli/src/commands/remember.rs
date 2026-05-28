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

//! `memryzed remember` implementation.
//!
//! Inserts a memory directly. Approved on insert.
//! Project-scoped memories trigger get-or-create on the project
//! record for the current working directory. Every insert also
//! computes and stores an embedding when an active embedder is
//! configured.

use anyhow::Result;

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::embedder::make_default;
use memryzed_core::memory::{insert_with_embedder, Kind, NewMemory, Scope};
use memryzed_core::projects;
use memryzed_core::Database;

use crate::commands::Context;

pub struct Args {
    pub text: String,
    pub scope: Scope,
    pub kind: Kind,
    pub pin: bool,
    pub ttl_days: Option<u32>,
}

pub fn run(ctx: &Context, args: Args) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let mut db = Database::open(&data_dir.db_file())?;
    let now = now_epoch_seconds();

    let scope_id = match args.scope {
        Scope::Global => None,
        Scope::Project => {
            let cwd = std::env::current_dir()?;
            let project = projects::ensure_for_cwd(&db, &cwd, now)?;
            Some(project.id)
        }
        Scope::Session => {
            return Err(crate::exit::Coded::new(
                crate::exit::GENERAL_ERROR,
                "session-scoped memories require an active session; sessions land in v0.2.0",
            )
            .into());
        }
    };

    let mut new = NewMemory::new(args.scope, args.text);
    new.scope_id = scope_id;
    new.kind = args.kind;
    new.pinned = args.pin;
    new.expires_at = args.ttl_days.map(|d| now + i64::from(d) * 86_400);

    let embedder = make_default(&data_dir.models_dir())?;
    let memory = insert_with_embedder(&mut db, new, embedder.as_ref(), now)?;

    if !ctx.quiet {
        println!("Stored memory {} ({})", memory.id, memory.scope);
        println!("  {}", memory.content);
        if embedder.is_active() {
            let dim = embedder
                .dimension()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "?".into());
            println!("  embedding: {} (dim={dim})", embedder.model_id());
        } else {
            println!("  embedding: skipped (no active embedder)");
        }
    }
    Ok(())
}
