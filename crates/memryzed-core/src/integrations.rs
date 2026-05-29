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

//! MCP client integrations.
//!
//! `memryzed install` walks the adapters here, finds which MCP-aware
//! clients are present on the user's machine, and writes the
//! Memryzed entry into each client's configuration file. Existing
//! configs are backed up to `<file>.memryzed.bak` before any write.
//!
//! Adapters are intentionally simple: each one knows where its
//! client stores its MCP config and how to merge a new server
//! entry into it. Adding a new client is one new adapter file.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{Error, Result};

/// Standard server name written into MCP client configs.
pub const SERVER_NAME: &str = "memryzed";

/// Standard suffix appended to backed-up config files.
pub const BACKUP_SUFFIX: &str = ".memryzed.bak";

/// What an adapter needs to do for a single client.
pub trait Adapter {
    /// Lower-case identifier used by `memryzed install --client <id>`.
    fn id(&self) -> &'static str;

    /// Display name shown in the CLI output.
    fn display_name(&self) -> &'static str;

    /// Path to the client's MCP config file under the user's home.
    fn config_path(&self, home: &Path) -> PathBuf;

    /// `true` if the client appears to be present on this machine.
    /// The default heuristic is "the parent directory of the config
    /// path exists" — meaning the client has at least been launched
    /// once. Adapters can override.
    fn is_present(&self, home: &Path) -> bool {
        if let Some(parent) = self.config_path(home).parent() {
            parent.is_dir()
        } else {
            false
        }
    }
}

/// All adapters known to this build, in stable order.
pub fn all() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(ClaudeCode),
        Box::new(KiroCli),
        Box::new(Cursor),
        Box::new(Codex),
        Box::new(Continue),
    ]
}

/// Look up a single adapter by its CLI id. Returns `None` for
/// unknown ids.
pub fn by_id(id: &str) -> Option<Box<dyn Adapter>> {
    all().into_iter().find(|a| a.id() == id)
}

// ----- adapters -----

/// Claude Code (`anthropic-ai/claude-code`).
pub struct ClaudeCode;
impl Adapter for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }
    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".claude").join("mcp.json")
    }
}

/// Kiro CLI.
pub struct KiroCli;
impl Adapter for KiroCli {
    fn id(&self) -> &'static str {
        "kiro"
    }
    fn display_name(&self) -> &'static str {
        "Kiro CLI"
    }
    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".kiro").join("settings").join("mcp.json")
    }
}

/// Cursor.
pub struct Cursor;
impl Adapter for Cursor {
    fn id(&self) -> &'static str {
        "cursor"
    }
    fn display_name(&self) -> &'static str {
        "Cursor"
    }
    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".cursor").join("mcp.json")
    }
}

/// Codex CLI.
pub struct Codex;
impl Adapter for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }
    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".codex").join("mcp.json")
    }
}

/// Continue.
pub struct Continue;
impl Adapter for Continue {
    fn id(&self) -> &'static str {
        "continue"
    }
    fn display_name(&self) -> &'static str {
        "Continue"
    }
    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".continue").join("config.json")
    }
}

/// Outcome of a single client install operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    /// The Memryzed entry was created where none existed.
    Added,
    /// An entry already existed and matched the target binary; no
    /// change written.
    AlreadyPresent,
    /// An entry existed but pointed elsewhere; we updated it.
    Updated,
    /// The client was not present on this machine.
    NotPresent,
}

/// Install Memryzed into one client's config.
///
/// `binary_path` is the absolute path to the `memryzed` executable
/// to register (typically `~/.memryzed/bin/memryzed`).
pub fn install_one(
    adapter: &dyn Adapter,
    home: &Path,
    binary_path: &Path,
) -> Result<InstallOutcome> {
    if !adapter.is_present(home) {
        return Ok(InstallOutcome::NotPresent);
    }
    let path = adapter.config_path(home);
    let entry = build_entry(binary_path);

    let mut existing = read_or_default(&path)?;
    let outcome = upsert_server(&mut existing, &entry);
    if outcome != InstallOutcome::AlreadyPresent {
        backup_if_exists(&path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let pretty = serde_json::to_string_pretty(&existing)
            .map_err(|e| Error::Validation(format!("failed to serialize config: {e}")))?;
        fs::write(&path, format!("{pretty}\n"))?;
    }
    Ok(outcome)
}

/// Outcome of a single client uninstall operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallOutcome {
    /// The Memryzed entry was found and removed.
    Removed,
    /// No entry existed; nothing was changed.
    NotPresent,
}

/// Remove the Memryzed entry from one client's config.
pub fn uninstall_one(adapter: &dyn Adapter, home: &Path) -> Result<UninstallOutcome> {
    let path = adapter.config_path(home);
    if !path.is_file() {
        return Ok(UninstallOutcome::NotPresent);
    }
    let mut existing = read_or_default(&path)?;
    if remove_server(&mut existing) {
        backup_if_exists(&path)?;
        let pretty = serde_json::to_string_pretty(&existing)
            .map_err(|e| Error::Validation(format!("failed to serialize config: {e}")))?;
        fs::write(&path, format!("{pretty}\n"))?;
        Ok(UninstallOutcome::Removed)
    } else {
        Ok(UninstallOutcome::NotPresent)
    }
}

/// Render the JSON entry that Memryzed registers, so the CLI can
/// also emit it via `memryzed install --print` for users on
/// unsupported clients.
pub fn render_entry(binary_path: &Path) -> String {
    let entry = build_entry(binary_path);
    let mut wrapper = Map::new();
    let mut servers = Map::new();
    servers.insert(SERVER_NAME.to_string(), Value::Object(entry));
    wrapper.insert("mcpServers".to_string(), Value::Object(servers));
    serde_json::to_string_pretty(&Value::Object(wrapper)).unwrap_or_default()
}

/// Whether a client's config currently contains a Memryzed server entry.
///
/// Returns `false` (without erroring) when the config file does not
/// exist, is empty, or contains malformed JSON. Used by
/// `memryzed doctor` to summarize integration state without ever
/// failing the overall health report on a malformed user file.
pub fn is_configured(adapter: &dyn Adapter, home: &Path) -> bool {
    let path = adapter.config_path(home);
    if !path.is_file() {
        return false;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let doc: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };
    doc.get("mcpServers")
        .and_then(|v| v.get(SERVER_NAME))
        .is_some()
}

fn build_entry(binary_path: &Path) -> Map<String, Value> {
    let mut entry = Map::new();
    entry.insert(
        "command".to_string(),
        Value::String(binary_path.to_string_lossy().into_owned()),
    );
    entry.insert(
        "args".to_string(),
        Value::Array(vec![Value::String("serve".to_string())]),
    );
    entry
}

fn read_or_default(path: &Path) -> Result<Value> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw)
        .map_err(|e| Error::Validation(format!("invalid JSON in {}: {e}", path.display())))
}

fn upsert_server(doc: &mut Value, entry: &Map<String, Value>) -> InstallOutcome {
    let root = match doc {
        Value::Object(map) => map,
        _ => {
            *doc = Value::Object(Map::new());
            doc.as_object_mut().expect("just set to object")
        }
    };
    let servers = root
        .entry("mcpServers".to_string())
        .or_insert(Value::Object(Map::new()));
    let servers_map = match servers {
        Value::Object(m) => m,
        _ => {
            *servers = Value::Object(Map::new());
            servers.as_object_mut().expect("just set to object")
        }
    };

    if let Some(existing) = servers_map.get(SERVER_NAME) {
        if existing == &Value::Object(entry.clone()) {
            return InstallOutcome::AlreadyPresent;
        }
        servers_map.insert(SERVER_NAME.to_string(), Value::Object(entry.clone()));
        return InstallOutcome::Updated;
    }
    servers_map.insert(SERVER_NAME.to_string(), Value::Object(entry.clone()));
    InstallOutcome::Added
}

fn remove_server(doc: &mut Value) -> bool {
    let Some(root) = doc.as_object_mut() else {
        return false;
    };
    let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return false;
    };
    servers.remove(SERVER_NAME).is_some()
}

fn backup_if_exists(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let backup = path.with_file_name(format!(
        "{}{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        BACKUP_SUFFIX
    ));
    fs::copy(path, &backup).map(|_| ()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_home() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn force_present_kiro(home: &Path) {
        std::fs::create_dir_all(home.join(".kiro").join("settings")).unwrap();
    }

    #[test]
    fn install_one_writes_entry_when_present() {
        let home = fixture_home();
        force_present_kiro(home.path());
        let bin = std::path::PathBuf::from("/opt/memryzed/bin/memryzed");
        let outcome = install_one(&KiroCli, home.path(), &bin).unwrap();
        assert_eq!(outcome, InstallOutcome::Added);

        let written = std::fs::read_to_string(KiroCli.config_path(home.path())).unwrap();
        let v: Value = serde_json::from_str(&written).unwrap();
        let entry = &v["mcpServers"]["memryzed"];
        assert_eq!(entry["command"], "/opt/memryzed/bin/memryzed");
        assert_eq!(entry["args"][0], "serve");
    }

    #[test]
    fn install_one_is_idempotent_when_already_present() {
        let home = fixture_home();
        force_present_kiro(home.path());
        let bin = std::path::PathBuf::from("/opt/memryzed/bin/memryzed");
        assert_eq!(
            install_one(&KiroCli, home.path(), &bin).unwrap(),
            InstallOutcome::Added
        );
        assert_eq!(
            install_one(&KiroCli, home.path(), &bin).unwrap(),
            InstallOutcome::AlreadyPresent
        );
    }

    #[test]
    fn install_one_updates_a_stale_entry_and_backs_it_up() {
        let home = fixture_home();
        force_present_kiro(home.path());
        let bin = std::path::PathBuf::from("/opt/memryzed/bin/memryzed");

        // First write a config pointing somewhere else.
        let path = KiroCli.config_path(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"mcpServers":{"memryzed":{"command":"/old/path","args":["serve"]}}}"#,
        )
        .unwrap();

        let outcome = install_one(&KiroCli, home.path(), &bin).unwrap();
        assert_eq!(outcome, InstallOutcome::Updated);

        // The stale path is preserved in the backup.
        let backup = path.with_file_name(format!(
            "{}{}",
            path.file_name().unwrap().to_string_lossy(),
            BACKUP_SUFFIX
        ));
        assert!(backup.is_file(), "backup must be created");
        let bak = std::fs::read_to_string(&backup).unwrap();
        assert!(bak.contains("/old/path"));
    }

    #[test]
    fn install_one_returns_not_present_for_missing_client() {
        let home = fixture_home();
        // Do NOT create the .kiro directory.
        let bin = std::path::PathBuf::from("/opt/memryzed/bin/memryzed");
        assert_eq!(
            install_one(&KiroCli, home.path(), &bin).unwrap(),
            InstallOutcome::NotPresent
        );
    }

    #[test]
    fn uninstall_one_removes_only_the_memryzed_entry() {
        let home = fixture_home();
        force_present_kiro(home.path());
        let path = KiroCli.config_path(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{
              "mcpServers": {
                "memryzed": {"command": "/x", "args": ["serve"]},
                "other": {"command": "/y"}
              }
            }"#,
        )
        .unwrap();

        let outcome = uninstall_one(&KiroCli, home.path()).unwrap();
        assert_eq!(outcome, UninstallOutcome::Removed);

        let written = std::fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&written).unwrap();
        assert!(v["mcpServers"]["memryzed"].is_null());
        assert!(v["mcpServers"]["other"].is_object(), "other must remain");
    }

    #[test]
    fn uninstall_one_no_op_when_no_entry_exists() {
        let home = fixture_home();
        force_present_kiro(home.path());
        let path = KiroCli.config_path(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"mcpServers":{}}"#).unwrap();

        let outcome = uninstall_one(&KiroCli, home.path()).unwrap();
        assert_eq!(outcome, UninstallOutcome::NotPresent);
    }

    #[test]
    fn render_entry_produces_valid_json_block() {
        let bin = std::path::PathBuf::from("/x/memryzed");
        let s = render_entry(&bin);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["mcpServers"]["memryzed"]["command"], "/x/memryzed");
    }

    #[test]
    fn by_id_finds_kiro() {
        let a = by_id("kiro").expect("kiro adapter");
        assert_eq!(a.id(), "kiro");
        assert_eq!(a.display_name(), "Kiro CLI");
    }

    #[test]
    fn by_id_returns_none_for_unknown() {
        assert!(by_id("doesnotexist").is_none());
    }
}
