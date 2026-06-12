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

//! Auto-save hooks for Claude Code.
//!
//! Claude Code can run shell commands on lifecycle events. Memryzed
//! installs two:
//!
//! - A periodic hook that mines the current session transcript so
//!   recent turns become memories without the user asking.
//! - A pre-compaction hook that runs before Claude Code truncates
//!   context, so nothing is lost when the window fills.
//!
//! Both hooks invoke the `memryzed` binary against the transcript
//! file Claude Code exposes to the hook environment. The hook
//! scripts are written under the Memryzed data directory and
//! referenced from Claude Code's `settings.json`.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::error::{Error, Result};

/// Event a hook binds to in Claude Code's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// Fires periodically while a session is active.
    Periodic,
    /// Fires before the client compacts (truncates) context.
    PreCompact,
}

impl HookEvent {
    /// The Claude Code settings key this event maps to.
    fn settings_key(self) -> &'static str {
        match self {
            // Claude Code emits Stop at the end of each turn; we use it
            // as the periodic checkpoint trigger.
            HookEvent::Periodic => "Stop",
            HookEvent::PreCompact => "PreCompact",
        }
    }
}

/// Where Memryzed writes its generated hook scripts.
pub fn hooks_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("hooks")
}

/// Path to the generated periodic-checkpoint hook script.
pub fn periodic_script_path(data_dir: &Path) -> PathBuf {
    hooks_dir(data_dir).join("claude-periodic.sh")
}

/// Path to the generated pre-compaction hook script.
pub fn precompact_script_path(data_dir: &Path) -> PathBuf {
    hooks_dir(data_dir).join("claude-precompact.sh")
}

/// Write the hook scripts into the Memryzed data directory.
///
/// `binary_path` is the absolute path to the `memryzed` executable
/// the scripts should call. Returns the two script paths written.
pub fn write_scripts(data_dir: &Path, binary_path: &Path) -> Result<(PathBuf, PathBuf)> {
    let dir = hooks_dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    let bin = binary_path.to_string_lossy();
    let periodic = periodic_script_path(data_dir);
    let precompact = precompact_script_path(data_dir);

    std::fs::write(&periodic, periodic_script(&bin))?;
    std::fs::write(&precompact, precompact_script(&bin))?;
    set_executable(&periodic)?;
    set_executable(&precompact)?;

    Ok((periodic, precompact))
}

/// Merge Memryzed's hook entries into a Claude Code `settings.json`
/// document, returning the updated document.
///
/// Existing hooks for other tools are preserved. Re-running is
/// idempotent: a Memryzed entry for an event is replaced, not
/// duplicated.
pub fn merge_into_settings(mut settings: Value, data_dir: &Path) -> Result<Value> {
    if !settings.is_object() {
        settings = Value::Object(Map::new());
    }
    let root = settings.as_object_mut().expect("ensured object");
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks.is_object() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks.as_object_mut().expect("ensured object");

    for (event, script) in [
        (HookEvent::Periodic, periodic_script_path(data_dir)),
        (HookEvent::PreCompact, precompact_script_path(data_dir)),
    ] {
        let entry = json!({
            "hooks": [
                { "type": "command", "command": script.to_string_lossy() }
            ]
        });
        hooks.insert(event.settings_key().to_string(), json!([entry]));
    }

    Ok(settings)
}

/// Remove Memryzed's hook entries from a settings document. Returns
/// `true` if anything was removed.
pub fn remove_from_settings(settings: &mut Value, data_dir: &Path) -> bool {
    let Some(root) = settings.as_object_mut() else {
        return false;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return false;
    };
    let our_scripts = [
        periodic_script_path(data_dir).to_string_lossy().to_string(),
        precompact_script_path(data_dir)
            .to_string_lossy()
            .to_string(),
    ];
    let mut removed = false;
    for key in [
        HookEvent::Periodic.settings_key(),
        HookEvent::PreCompact.settings_key(),
    ] {
        if let Some(val) = hooks.get(key) {
            if entry_references_our_script(val, &our_scripts) {
                hooks.remove(key);
                removed = true;
            }
        }
    }
    removed
}

/// Whether a Claude `hooks[event]` value contains a command that
/// invokes one of our generated scripts.
///
/// Compares the `command` field directly rather than substring-matching
/// serialized JSON, which would break on Windows where path separators
/// are escaped (`\` becomes `\\`) during serialization.
fn entry_references_our_script(val: &Value, our_scripts: &[String]) -> bool {
    val.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("hooks").and_then(|h| h.as_array()))
        .flatten()
        .filter_map(|h| h.get("command").and_then(|c| c.as_str()))
        .any(|cmd| our_scripts.iter().any(|p| p == cmd))
}

fn periodic_script(bin: &str) -> String {
    let lines = [
        "#!/usr/bin/env bash".to_string(),
        "# Memryzed periodic checkpoint hook for Claude Code.".to_string(),
        "# Generated by `memryzed hooks install`. Safe to delete; re-run".to_string(),
        "# the installer to regenerate.".to_string(),
        "#".to_string(),
        "# Claude Code passes the active transcript path via the".to_string(),
        "# CLAUDE_TRANSCRIPT_PATH environment variable. We mine it so".to_string(),
        "# recent turns become candidate memories. Failures are swallowed".to_string(),
        "# so a hook problem never blocks the agent.".to_string(),
        "set -u".to_string(),
        "if [ -n \"${CLAUDE_TRANSCRIPT_PATH:-}\" ] && [ -f \"${CLAUDE_TRANSCRIPT_PATH}\" ]; then"
            .to_string(),
        format!(
            "  \"{bin}\" mine \"${{CLAUDE_TRANSCRIPT_PATH}}\" --source claude-code >/dev/null 2>&1 || true"
        ),
        "fi".to_string(),
        "exit 0".to_string(),
    ];
    format!("{}\n", lines.join("\n"))
}

fn precompact_script(bin: &str) -> String {
    let lines = [
        "#!/usr/bin/env bash".to_string(),
        "# Memryzed pre-compaction hook for Claude Code.".to_string(),
        "# Generated by `memryzed hooks install`. Runs before Claude Code".to_string(),
        "# truncates context so the session is captured first.".to_string(),
        "set -u".to_string(),
        "if [ -n \"${CLAUDE_TRANSCRIPT_PATH:-}\" ] && [ -f \"${CLAUDE_TRANSCRIPT_PATH}\" ]; then"
            .to_string(),
        format!(
            "  \"{bin}\" mine \"${{CLAUDE_TRANSCRIPT_PATH}}\" --source claude-code --force >/dev/null 2>&1 || true"
        ),
        "fi".to_string(),
        "exit 0".to_string(),
    ];
    format!("{}\n", lines.join("\n"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).map_err(Into::into)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    // No execute bit on Windows; Claude Code invokes the script via
    // its configured shell.
    Ok(())
}

/// Claude Code settings path under a home directory.
pub fn claude_settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

/// Read a settings document, returning an empty object when the file
/// is absent or empty. Malformed JSON is an error so we never
/// silently overwrite a user's real settings.
pub fn read_settings(path: &Path) -> Result<Value> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = std::fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw)
        .map_err(|e| Error::Validation(format!("invalid JSON in {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn data_dir() -> PathBuf {
        PathBuf::from("/home/x/.memryzed")
    }

    #[test]
    fn scripts_reference_the_binary_and_guard_the_transcript() {
        let s = periodic_script("/opt/memryzed");
        assert!(s.starts_with("#!/usr/bin/env bash"));
        assert!(s.contains("\"/opt/memryzed\" mine"));
        assert!(s.contains("CLAUDE_TRANSCRIPT_PATH"));
        assert!(s.contains("|| true"));
        // Pre-compaction forces a re-mine.
        assert!(precompact_script("/opt/memryzed").contains("--force"));
    }

    #[test]
    fn merge_preserves_existing_settings_and_other_hooks() {
        let existing = json!({
            "model": "sonnet",
            "hooks": {
                "PreToolUse": [{"hooks":[{"type":"command","command":"other.sh"}]}]
            }
        });
        let merged = merge_into_settings(existing, &data_dir()).unwrap();

        // Unrelated top-level key preserved.
        assert_eq!(merged["model"], "sonnet");
        // Unrelated hook preserved.
        assert!(merged["hooks"]["PreToolUse"].is_array());
        // Our two events added.
        assert!(merged["hooks"]["Stop"].is_array());
        assert!(merged["hooks"]["PreCompact"].is_array());
        let stop = serde_json::to_string(&merged["hooks"]["Stop"]).unwrap();
        assert!(stop.contains("claude-periodic.sh"));
    }

    #[test]
    fn merge_is_idempotent() {
        let once = merge_into_settings(json!({}), &data_dir()).unwrap();
        let twice = merge_into_settings(once.clone(), &data_dir()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn merge_into_non_object_replaces_with_object() {
        let merged = merge_into_settings(json!("garbage"), &data_dir()).unwrap();
        assert!(merged.is_object());
        assert!(merged["hooks"]["Stop"].is_array());
    }

    #[test]
    fn remove_strips_only_our_hooks() {
        let mut settings = merge_into_settings(
            json!({"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"other.sh"}]}]}}),
            &data_dir(),
        )
        .unwrap();

        assert!(remove_from_settings(&mut settings, &data_dir()));
        // Ours gone.
        assert!(settings["hooks"].get("Stop").is_none());
        assert!(settings["hooks"].get("PreCompact").is_none());
        // Theirs kept.
        assert!(settings["hooks"]["PreToolUse"].is_array());
    }

    #[test]
    fn remove_returns_false_when_nothing_present() {
        let mut settings = json!({"model": "sonnet"});
        assert!(!remove_from_settings(&mut settings, &data_dir()));
    }

    #[test]
    fn remove_matches_windows_style_paths() {
        // Regression: removal used to substring-match serialized JSON,
        // which escapes backslashes and never matched on Windows.
        let win = PathBuf::from(r"C:\Users\x\AppData\Local\memryzed");
        let mut settings = merge_into_settings(json!({}), &win).unwrap();
        assert!(settings["hooks"].get("Stop").is_some());
        assert!(remove_from_settings(&mut settings, &win));
        assert!(settings["hooks"].get("Stop").is_none());
        assert!(settings["hooks"].get("PreCompact").is_none());
    }

    #[test]
    fn read_settings_missing_file_is_empty_object() {
        let p = PathBuf::from("/no/such/settings.json");
        assert_eq!(read_settings(&p).unwrap(), Value::Object(Map::new()));
    }
}
