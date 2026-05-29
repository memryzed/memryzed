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

//! `memryzed review` implementation.
//!
//! Opens the pending-memory review TUI. Thin wrapper over
//! `memryzed_tui::run`.

use anyhow::Result;

use crate::commands::Context;
use crate::exit;

pub fn run(ctx: &Context) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    if !data_dir.db_file().is_file() {
        return Err(exit::Coded::new(
            exit::GENERAL_ERROR,
            "data directory not initialized; run `memryzed init`",
        )
        .into());
    }
    let acted = memryzed_tui::run(&data_dir)?;
    if acted > 0 && !ctx.quiet {
        println!(
            "Reviewed {acted} pending memor{}.",
            if acted == 1 { "y" } else { "ies" }
        );
    }
    Ok(())
}
