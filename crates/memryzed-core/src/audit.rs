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

//! Audit log.
//!
//! Append-only JSONL at `<data_dir>/audit.log`. Every state-changing
//! operation writes one line. The format is documented in
//! `docs/data-model.md`. This module implements the writer and the
//! reader used by `memryzed log`.

use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::clock::format_epoch_iso;
use crate::error::Result;

/// One audit entry as serialized to JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// ISO-8601 UTC timestamp of the event.
    pub ts: String,
    /// Event kind. See `docs/data-model.md` for the list.
    pub kind: String,
    /// Originating client identifier when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// Kind-specific structured payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

impl AuditEntry {
    /// Construct a new entry with the current wall-clock time.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            ts: format_epoch_iso(crate::now_epoch_seconds()),
            kind: kind.into(),
            client: None,
            detail: None,
        }
    }

    /// Set the client identifier.
    pub fn with_client(mut self, client: impl Into<String>) -> Self {
        self.client = Some(client.into());
        self
    }

    /// Set the structured detail payload.
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// Append a single entry to the audit log.
///
/// Creates the parent directory if needed. Each entry is one line.
pub fn append(audit_path: &Path, entry: &AuditEntry) -> Result<()> {
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(entry)
        .map_err(|e| crate::Error::Validation(format!("failed to serialize audit entry: {e}")))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Read entries from the audit log.
///
/// Optionally limits to the most recent `tail` lines. Returns
/// entries in chronological order (oldest first). Lines that fail
/// to parse are skipped with a tracing warning.
pub fn read_recent(audit_path: &Path, tail: Option<usize>) -> Result<Vec<AuditEntry>> {
    if !audit_path.is_file() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(audit_path)?;
    let reader = BufReader::new(file);
    let mut all: Vec<AuditEntry> = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(target: "memryzed::audit", error = %e, "failed to read line");
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEntry>(&line) {
            Ok(entry) => all.push(entry),
            Err(e) => {
                tracing::warn!(target: "memryzed::audit", error = %e, "skipping malformed entry");
            }
        }
    }
    if let Some(n) = tail {
        if all.len() > n {
            let start = all.len() - n;
            return Ok(all.split_off(start));
        }
    }
    Ok(all)
}

/// Open the audit log for live tailing.
///
/// Returns a reader positioned at the end of the file. The caller
/// can read new bytes as they are appended (poll-based; rotate
/// detection is a future enhancement).
pub fn open_for_tail(audit_path: &Path) -> io::Result<std::fs::File> {
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(audit_path)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_audit() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("audit.log");
        (tmp, p)
    }

    #[test]
    fn append_then_read() {
        let (_g, path) = temp_audit();
        let e1 = AuditEntry::new("recall").with_client("kiro");
        let e2 = AuditEntry::new("remember")
            .with_client("kiro")
            .with_detail(serde_json::json!({"id": "mem_x"}));
        append(&path, &e1).unwrap();
        append(&path, &e2).unwrap();

        let read = read_recent(&path, None).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].kind, "recall");
        assert_eq!(read[1].kind, "remember");
        assert_eq!(read[1].client.as_deref(), Some("kiro"));
        assert_eq!(read[1].detail.as_ref().unwrap()["id"], "mem_x");
    }

    #[test]
    fn read_recent_with_tail_limits_results() {
        let (_g, path) = temp_audit();
        for i in 0..10 {
            append(&path, &AuditEntry::new(format!("kind_{i}"))).unwrap();
        }
        let read = read_recent(&path, Some(3)).unwrap();
        assert_eq!(read.len(), 3);
        assert_eq!(read[0].kind, "kind_7");
        assert_eq!(read[2].kind, "kind_9");
    }

    #[test]
    fn read_recent_returns_empty_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nonexistent.log");
        assert!(read_recent(&path, None).unwrap().is_empty());
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let (_g, path) = temp_audit();
        std::fs::write(&path, "not-json\n{\"ts\":\"x\",\"kind\":\"k\"}\n").unwrap();
        let read = read_recent(&path, None).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].kind, "k");
    }
}
