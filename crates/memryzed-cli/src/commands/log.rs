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

//! `memryzed log` implementation.
//!
//! Prints recent audit-log entries. With `--follow`, polls the file
//! and streams new entries as they are appended.

use std::io::{Read, Seek, SeekFrom};
use std::thread;
use std::time::Duration;

use anyhow::Result;

use memryzed_core::audit::{read_recent, AuditEntry};

use crate::commands::Context;

pub struct Args {
    pub follow: bool,
    pub tail: usize,
    pub client: Option<String>,
}

pub fn run(ctx: &Context, args: Args) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let audit_path = data_dir.audit_log();

    let entries = read_recent(&audit_path, Some(args.tail))?;
    for entry in &entries {
        if matches(entry, args.client.as_deref()) {
            print_entry(entry);
        }
    }

    if !args.follow {
        return Ok(());
    }

    // Follow mode: poll the file for appended bytes.
    let mut file = memryzed_core::audit::open_for_tail(&audit_path)?;
    let mut pos = file.seek(SeekFrom::End(0))?;
    let mut buf = String::new();
    loop {
        thread::sleep(Duration::from_millis(500));
        let len = file.metadata()?.len();
        if len < pos {
            // File was rotated/truncated; restart from the top.
            pos = 0;
        }
        if len > pos {
            file.seek(SeekFrom::Start(pos))?;
            buf.clear();
            file.read_to_string(&mut buf)?;
            pos = len;
            for line in buf.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<AuditEntry>(line) {
                    if matches(&entry, args.client.as_deref()) {
                        print_entry(&entry);
                    }
                }
            }
        }
    }
}

fn matches(entry: &AuditEntry, client: Option<&str>) -> bool {
    match client {
        None => true,
        Some(c) => entry.client.as_deref() == Some(c),
    }
}

fn print_entry(entry: &AuditEntry) {
    let client = entry.client.as_deref().unwrap_or("-");
    let detail = entry
        .detail
        .as_ref()
        .map(|d| d.to_string())
        .unwrap_or_default();
    println!(
        "{ts}  {kind:<16}  {client:<12}  {detail}",
        ts = entry.ts,
        kind = entry.kind,
    );
}
