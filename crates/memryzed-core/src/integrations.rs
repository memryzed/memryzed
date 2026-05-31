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

/// Marker lines bracketing the Memryzed steering block in a shared
/// rules file (such as Claude's CLAUDE.md), so it can be updated or
/// removed without touching the user's own content.
pub const STEERING_BEGIN: &str = "<!-- BEGIN MEMRYZED -->";
/// Closing marker for the steering block.
pub const STEERING_END: &str = "<!-- END MEMRYZED -->";

/// The always-on instruction written into each client's rules file.
/// This is the guaranteed fallback for agents that do not act on the
/// MCP server's instructions field: it tells the agent, in its own
/// rules, to use Memryzed proactively.
pub const STEERING_BODY: &str = "\
## Memory (Memryzed)

You have persistent memory of the user's past conversations and facts,
across this and every other agent they use, through the Memryzed MCP
server. Use it proactively, without being asked:

- Before answering anything that may depend on earlier work, prior
  decisions, past sessions, or \"what we discussed\", call the
  `recall` tool first and use what it returns.
- At the start of a new task, call `recall` to load relevant context.
- When the user states a durable preference, decision, or fact, call
  `remember` to persist it.

Treat checking memory as a normal first step, like reading a file.";

/// What an adapter needs to do for a single client.
pub trait Adapter {
    /// Lower-case identifier used by `memryzed install --client <id>`.
    fn id(&self) -> &'static str;

    /// Display name shown in the CLI output.
    fn display_name(&self) -> &'static str;

    /// Path to the client's MCP config file under the user's home.
    fn config_path(&self, home: &Path) -> PathBuf;

    /// Path to a per-client "always-on" rules/steering file where a
    /// one-time instruction can be written so the agent reliably uses
    /// Memryzed even if it does not honor the MCP server's
    /// instructions field. `None` for clients with no such mechanism.
    fn steering_path(&self, home: &Path) -> Option<PathBuf> {
        let _ = home;
        None
    }

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
    fn steering_path(&self, home: &Path) -> Option<PathBuf> {
        // Claude Code reads project/user memory from CLAUDE.md.
        Some(home.join(".claude").join("CLAUDE.md"))
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
    fn steering_path(&self, home: &Path) -> Option<PathBuf> {
        // Kiro reads always-on steering rules from ~/.kiro/steering/.
        Some(home.join(".kiro").join("steering").join("memryzed.md"))
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

/// Outcome of writing a steering rule for one client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteeringOutcome {
    /// The client has no steering mechanism; nothing written.
    Unsupported,
    /// A new steering rule file (or block) was written.
    Written,
    /// The steering rule was already present and current.
    AlreadyPresent,
    /// An existing Memryzed block was refreshed.
    Updated,
}

/// Write (or refresh) the always-on Memryzed steering rule for a
/// client, the guaranteed fallback for agents that ignore the MCP
/// server's instructions field.
///
/// For a dedicated steering file (Kiro's `steering/memryzed.md`) the
/// file is owned entirely by Memryzed and written verbatim. For a
/// shared rules file (Claude's `CLAUDE.md`) the Memryzed block is
/// merged between markers, preserving any surrounding user content.
pub fn write_steering(adapter: &dyn Adapter, home: &Path) -> Result<SteeringOutcome> {
    let Some(path) = adapter.steering_path(home) else {
        return Ok(SteeringOutcome::Unsupported);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let dedicated = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("memryzed.md"))
        .unwrap_or(false);

    if dedicated {
        // Memryzed owns this file entirely.
        let desired = format!("{STEERING_BODY}\n");
        if path.is_file()
            && fs::read_to_string(&path)
                .map(|c| c == desired)
                .unwrap_or(false)
        {
            return Ok(SteeringOutcome::AlreadyPresent);
        }
        let existed = path.is_file();
        fs::write(&path, desired)?;
        return Ok(if existed {
            SteeringOutcome::Updated
        } else {
            SteeringOutcome::Written
        });
    }

    // Shared file: merge a marked block, preserving other content.
    let block = format!("{STEERING_BEGIN}\n{STEERING_BODY}\n{STEERING_END}\n");
    let existing = if path.is_file() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };

    if let (Some(start), Some(end)) = (existing.find(STEERING_BEGIN), existing.find(STEERING_END)) {
        let end = end + STEERING_END.len();
        let current = &existing[start..end];
        let want = format!("{STEERING_BEGIN}\n{STEERING_BODY}\n{STEERING_END}");
        if current == want {
            return Ok(SteeringOutcome::AlreadyPresent);
        }
        let mut updated = String::with_capacity(existing.len());
        updated.push_str(&existing[..start]);
        updated.push_str(&want);
        updated.push_str(&existing[end..]);
        backup_if_exists(&path)?;
        fs::write(&path, updated)?;
        return Ok(SteeringOutcome::Updated);
    }

    // No existing block: append (with a separating blank line).
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&block);
    if path.is_file() {
        backup_if_exists(&path)?;
    }
    fs::write(&path, out)?;
    Ok(SteeringOutcome::Written)
}

/// Outcome of writing an auto-approve / tool-trust rule for one client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoApproveOutcome {
    /// The client has no reliable per-server auto-approve mechanism,
    /// or there was nothing to write into (e.g. no editable agent).
    Unsupported,
    /// A trust rule was written.
    Written,
    /// The trust rule was already present.
    AlreadyPresent,
}

/// Auto-approve the Memryzed MCP tools for a client so the user is not
/// prompted on every `recall`/`remember` call. Scoped to the Memryzed
/// server only; it never enables blanket trust of other tools, which
/// would be the user's decision to make, not ours.
///
/// Mechanisms differ per client (verified against current docs):
/// - Claude Code: a `mcp__memryzed` rule in `permissions.allow`.
/// - Kiro: `@memryzed` added to each existing agent's `allowedTools`
///   (the built-in default agent cannot be edited and is left alone).
/// - Cursor: `autoApprove` on the Memryzed server entry.
///
/// Codex (global-only approval) and Continue (no documented per-MCP
/// rule) return `Unsupported`; install prints guidance instead.
pub fn write_auto_approve(adapter: &dyn Adapter, home: &Path) -> Result<AutoApproveOutcome> {
    match adapter.id() {
        "claude-code" => add_to_json_string_array(
            &home.join(".claude").join("settings.json"),
            &["permissions", "allow"],
            "mcp__memryzed",
        ),
        "kiro" => kiro_auto_approve(home),
        "cursor" => cursor_auto_approve(home),
        _ => Ok(AutoApproveOutcome::Unsupported),
    }
}

/// Add `@memryzed` to the `allowedTools` of every existing Kiro agent
/// config. Skips the example file. Returns `Unsupported` when no
/// editable agent file exists.
fn kiro_auto_approve(home: &Path) -> Result<AutoApproveOutcome> {
    let dir = home.join(".kiro").join("agents");
    if !dir.is_dir() {
        return Ok(AutoApproveOutcome::Unsupported);
    }
    let mut any_agent = false;
    let mut wrote = false;
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        // Only *.json (so agent_config.json.example, ext "example", is skipped).
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        any_agent = true;
        if add_to_json_string_array(&path, &["allowedTools"], "@memryzed")?
            == AutoApproveOutcome::Written
        {
            wrote = true;
        }
    }
    Ok(match (any_agent, wrote) {
        (false, _) => AutoApproveOutcome::Unsupported,
        (true, true) => AutoApproveOutcome::Written,
        (true, false) => AutoApproveOutcome::AlreadyPresent,
    })
}

/// Set `autoApprove: true` on the Memryzed server entry in Cursor's
/// MCP config. Requires the server entry to already exist (install
/// writes it first).
fn cursor_auto_approve(home: &Path) -> Result<AutoApproveOutcome> {
    let path = home.join(".cursor").join("mcp.json");
    if !path.is_file() {
        return Ok(AutoApproveOutcome::Unsupported);
    }
    let mut doc = read_or_default(&path)?;
    {
        let server = doc
            .get_mut("mcpServers")
            .and_then(|s| s.get_mut(SERVER_NAME))
            .and_then(|v| v.as_object_mut());
        let Some(server) = server else {
            return Ok(AutoApproveOutcome::Unsupported);
        };
        if server.get("autoApprove") == Some(&Value::Bool(true)) {
            return Ok(AutoApproveOutcome::AlreadyPresent);
        }
        server.insert("autoApprove".to_string(), Value::Bool(true));
    }
    backup_if_exists(&path)?;
    let pretty = serde_json::to_string_pretty(&doc)
        .map_err(|e| Error::Validation(format!("failed to serialize config: {e}")))?;
    fs::write(&path, format!("{pretty}\n"))?;
    Ok(AutoApproveOutcome::Written)
}

/// Ensure `value` appears in the string array at the nested `keys`
/// path within the JSON document at `file`, creating the file and any
/// intermediate objects. Preserves all other content. Returns whether
/// a write was needed.
fn add_to_json_string_array(file: &Path, keys: &[&str], value: &str) -> Result<AutoApproveOutcome> {
    let mut doc = read_or_default(file)?;
    let mut cur = &mut doc;
    for key in &keys[..keys.len() - 1] {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur
            .as_object_mut()
            .expect("just ensured object")
            .entry((*key).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if !cur.is_object() {
        *cur = Value::Object(Map::new());
    }
    let last = keys[keys.len() - 1];
    let arr = cur
        .as_object_mut()
        .expect("just ensured object")
        .entry(last.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !arr.is_array() {
        *arr = Value::Array(Vec::new());
    }
    let items = arr.as_array_mut().expect("just ensured array");
    if items.iter().any(|v| v.as_str() == Some(value)) {
        return Ok(AutoApproveOutcome::AlreadyPresent);
    }
    items.push(Value::String(value.to_string()));

    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    backup_if_exists(file)?;
    let pretty = serde_json::to_string_pretty(&doc)
        .map_err(|e| Error::Validation(format!("failed to serialize config: {e}")))?;
    fs::write(file, format!("{pretty}\n"))?;
    Ok(AutoApproveOutcome::Written)
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
    fn claude_auto_approve_adds_mcp_rule_idempotently() {
        let home = fixture_home();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        assert_eq!(
            write_auto_approve(&ClaudeCode, home.path()).unwrap(),
            AutoApproveOutcome::Written
        );
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(home.path().join(".claude").join("settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(v["permissions"]["allow"][0], "mcp__memryzed");
        // Second run is a no-op.
        assert_eq!(
            write_auto_approve(&ClaudeCode, home.path()).unwrap(),
            AutoApproveOutcome::AlreadyPresent
        );
    }

    #[test]
    fn kiro_auto_approve_trusts_memryzed_in_existing_agents() {
        let home = fixture_home();
        let agents = home.path().join(".kiro").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("default.json"),
            r#"{"name":"default","allowedTools":["read"]}"#,
        )
        .unwrap();
        // Example file must be ignored.
        std::fs::write(agents.join("x.json.example"), "{}").unwrap();

        assert_eq!(
            write_auto_approve(&KiroCli, home.path()).unwrap(),
            AutoApproveOutcome::Written
        );
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(agents.join("default.json")).unwrap())
                .unwrap();
        let tools: Vec<&str> = v["allowedTools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        assert!(tools.contains(&"@memryzed"));
        assert!(tools.contains(&"read"));
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
