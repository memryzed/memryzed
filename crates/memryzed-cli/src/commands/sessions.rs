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

//! `memryzed sessions`, `resume`, and `end-session` implementations.

use anyhow::Result;

use memryzed_core::clock::{format_epoch_iso, now_epoch_seconds};
use memryzed_core::Database;
use memryzed_core::{projects, sessions};

use crate::commands::Context;
use crate::exit;

fn open_db(ctx: &Context) -> Result<(Database, memryzed_core::DataDir)> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }
    let db = Database::open(&data_dir.db_file())?;
    Ok((db, data_dir))
}

fn current_project_id(db: &Database) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let project = projects::ensure_for_cwd(db, &cwd, now_epoch_seconds())?;
    Ok(project.id)
}

pub fn list(ctx: &Context, limit: Option<u32>) -> Result<()> {
    let (db, _dir) = open_db(ctx)?;
    let project_id = current_project_id(&db)?;
    let sessions = sessions::list(&db, &project_id, limit)?;

    if sessions.is_empty() {
        if !ctx.quiet {
            println!("No sessions for the current project.");
        }
        return Ok(());
    }
    if !ctx.quiet {
        for s in &sessions {
            let pin = if s.pinned { "*" } else { " " };
            println!(
                "{pin} {id}  {status:<10}  {updated}  {title}",
                id = s.id,
                status = s.status,
                updated = format_epoch_iso(s.updated_at),
                title = s.title.as_deref().unwrap_or("(untitled)"),
            );
        }
        println!();
        println!("{} session(s)", sessions.len());
    }
    Ok(())
}

pub fn resume(ctx: &Context, id: Option<String>, json: bool) -> Result<()> {
    let (db, _dir) = open_db(ctx)?;
    let session = match id {
        Some(id) => sessions::get_by_id(&db, &id)?,
        None => {
            let project_id = current_project_id(&db)?;
            sessions::resume_latest(&db, &project_id)?
        }
    };

    let Some(session) = session else {
        if !ctx.quiet {
            println!("No resumable session for the current project.");
        }
        return Ok(());
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&session.state)?);
        return Ok(());
    }

    if !ctx.quiet {
        println!("Session {}", session.id);
        println!(
            "  Title    {}",
            session.title.as_deref().unwrap_or("(untitled)")
        );
        println!("  Status   {}", session.status);
        println!("  Updated  {}", format_epoch_iso(session.updated_at));
        println!();
        println!("State:");
        println!("{}", serde_json::to_string_pretty(&session.state)?);
    }
    Ok(())
}

pub fn end_session(ctx: &Context, id: String) -> Result<()> {
    let (db, _dir) = open_db(ctx)?;
    let session = sessions::end(&db, &id, now_epoch_seconds())?;
    if !ctx.quiet {
        println!("Ended session {} ({})", session.id, session.status);
    }
    Ok(())
}
