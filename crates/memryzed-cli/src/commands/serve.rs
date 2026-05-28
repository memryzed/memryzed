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

//! `memryzed serve` implementation.
//!
//! Runs the MCP server over stdio. MCP-aware clients spawn this
//! process and exchange MCP frames on stdin/stdout. Long-running
//! for the duration of the client session.
//!
//! Logging goes to stderr so it does not interfere with the protocol
//! frames on stdout. The CLI's tracing setup (in `main.rs`) already
//! routes there.

use anyhow::{Context as _, Result};

use memryzed_mcp::{stdio, MemryzedServer};
use rmcp::ServiceExt;

use crate::commands::Context;

pub fn run(ctx: &Context) -> Result<()> {
    let data_dir = ctx.data_dir()?;

    // Ensure the data directory exists. If `memryzed init` has not
    // been run, perform a silent default initialization so an agent
    // never sees a noisy first-run prompt.
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir.root())
            .with_context(|| format!("creating data directory at {}", data_dir.root().display()))?;
        std::fs::create_dir_all(data_dir.bin_dir()).ok();
        std::fs::create_dir_all(data_dir.models_dir()).ok();
    }

    // Build a single-threaded current-thread runtime; the protocol
    // is request/response and we do not need a multi-threaded pool.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("constructing tokio runtime")?;

    runtime.block_on(async move {
        let server = MemryzedServer::open(&data_dir)?;
        tracing::info!(
            target: "memryzed::serve",
            data_dir = %data_dir.root().display(),
            "memryzed serve ready"
        );
        let service = server
            .serve(stdio())
            .await
            .context("starting MCP service over stdio")?;
        service
            .waiting()
            .await
            .context("MCP service exited unexpectedly")?;
        anyhow::Ok(())
    })?;

    Ok(())
}
