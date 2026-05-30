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

//! Source-format adapters for transcript mining.
//!
//! Each supported client stores its session transcripts as JSON Lines
//! but with a different per-line shape. An adapter turns one raw file
//! into a normalized list of [`Turn`]s. Lines that do not represent a
//! user or assistant message are ignored.

use std::path::Path;

use serde_json::Value;

use super::Turn;

/// A transcript source format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Detect from the path layout.
    Auto,
    /// Kiro CLI sessions (`~/.kiro/sessions/`).
    Kiro,
    /// Claude Code sessions (`~/.claude/projects/`).
    ClaudeCode,
}

impl Source {
    /// Parse a raw JSONL transcript into normalized turns.
    pub fn parse(self, raw: &str) -> Vec<Turn> {
        match self {
            Source::ClaudeCode => parse_jsonl(raw, claude_line),
            // Auto resolves to Kiro at the file level; treat as Kiro.
            Source::Kiro | Source::Auto => parse_jsonl(raw, kiro_line),
        }
    }

    /// CLI identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Auto => "auto",
            Source::Kiro => "kiro",
            Source::ClaudeCode => "claude-code",
        }
    }
}

impl std::str::FromStr for Source {
    type Err = crate::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Source::Auto),
            "kiro" => Ok(Source::Kiro),
            "claude-code" | "claude" => Ok(Source::ClaudeCode),
            other => Err(crate::error::Error::Validation(format!(
                "unknown mining source {other:?}; expected auto, kiro, or claude-code"
            ))),
        }
    }
}

/// Guess the source format from a path. Returns `None` when the path
/// gives no hint, leaving the caller to fall back to a default.
pub fn detect_source(path: &Path) -> Option<Source> {
    let s = path.to_string_lossy();
    if s.contains(".claude") {
        Some(Source::ClaudeCode)
    } else if s.contains(".kiro") {
        Some(Source::Kiro)
    } else {
        None
    }
}

/// Parse each non-empty line with `line_fn`, collecting the turns it
/// yields. Malformed lines are skipped rather than failing the file,
/// since real transcripts often contain tool-call and metadata lines.
fn parse_jsonl(raw: &str, line_fn: fn(&Value) -> Option<Turn>) -> Vec<Turn> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(turn) = line_fn(&value) {
            if !turn.text.trim().is_empty() {
                out.push(turn);
            }
        }
    }
    out
}

/// Normalize one Kiro session line.
///
/// Kiro v1 lines have a top-level `kind` (`Prompt` for a user turn,
/// `AssistantMessage` for an assistant turn) and a `data.content`
/// array of blocks. Text blocks are `{"kind":"text","data":"..."}`;
/// thinking and tool blocks are ignored.
fn kiro_line(v: &Value) -> Option<Turn> {
    let kind = v.get("kind")?.as_str()?;
    let role = match kind {
        "Prompt" => "user",
        "AssistantMessage" => "assistant",
        _ => return None,
    };
    let content = v.get("data")?.get("content")?;
    let text = kiro_text(content);
    Some(Turn {
        role: role.to_string(),
        text,
    })
}

/// Join the text blocks of a Kiro `data.content` array.
fn kiro_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if item.get("kind").and_then(|k| k.as_str()) == Some("text") {
                    if let Some(s) = item.get("data").and_then(|d| d.as_str()) {
                        parts.push(s.to_string());
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

/// Normalize one Claude Code session line.
///
/// Claude Code wraps the turn under `type: "user" | "assistant"` with
/// a nested `message.content` that is a string or content-block array.
fn claude_line(v: &Value) -> Option<Turn> {
    let kind = v.get("type")?.as_str()?;
    if kind != "user" && kind != "assistant" {
        return None;
    }
    let content = v
        .get("message")
        .and_then(|m| m.get("content"))
        .or_else(|| v.get("content"))?;
    Some(Turn {
        role: kind.to_string(),
        text: extract_text(content),
    })
}

/// Pull plain text out of a content value that may be a bare string
/// or an array of `{type, text}` blocks. Non-text blocks (tool calls,
/// images) are skipped.
fn extract_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(s) = item.as_str() {
                    parts.push(s.to_string());
                } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(t.to_string());
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_from_str_round_trip() {
        assert_eq!("auto".parse::<Source>().unwrap(), Source::Auto);
        assert_eq!("kiro".parse::<Source>().unwrap(), Source::Kiro);
        assert_eq!("claude-code".parse::<Source>().unwrap(), Source::ClaudeCode);
        assert_eq!("claude".parse::<Source>().unwrap(), Source::ClaudeCode);
        assert!("wat".parse::<Source>().is_err());
    }

    #[test]
    fn detect_source_from_path_hints() {
        assert_eq!(
            detect_source(Path::new("/home/x/.claude/projects/a.jsonl")),
            Some(Source::ClaudeCode)
        );
        assert_eq!(
            detect_source(Path::new("/home/x/.kiro/sessions/a.jsonl")),
            Some(Source::Kiro)
        );
        assert_eq!(detect_source(Path::new("/tmp/random.jsonl")), None);
    }

    #[test]
    fn kiro_parses_prompt_and_assistant_lines() {
        let raw = concat!(
            r#"{"version":"v1","kind":"Prompt","data":{"content":[{"kind":"text","data":"I prefer uv over pip"}]}}"#,
            "\n",
            r#"{"version":"v1","kind":"AssistantMessage","data":{"content":[{"kind":"thinking","data":{"text":"hmm"}},{"kind":"text","data":"Noted."}]}}"#,
            "\n",
            r#"{"version":"v1","kind":"ToolUse","data":{"content":[{"kind":"text","data":"ignored"}]}}"#,
        );
        let turns = Source::Kiro.parse(raw);
        // Two real turns; the ToolUse line is not a user/assistant turn.
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[0].text, "I prefer uv over pip");
        assert_eq!(turns[1].role, "assistant");
        // Only the text block is kept; the thinking block is dropped.
        assert_eq!(turns[1].text, "Noted.");
    }

    #[test]
    fn kiro_ignores_unknown_and_malformed_lines() {
        let raw = concat!(
            "not json\n",
            r#"{"kind":"Other","data":{}}"#,
            "\n",
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"hi"}]}}"#,
        );
        let turns = Source::Kiro.parse(raw);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].text, "hi");
    }

    #[test]
    fn claude_parses_string_and_block_content() {
        let raw = concat!(
            r#"{"type":"user","message":{"content":"deploy with make ship"}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Done."}]}}"#,
            "\n",
            r#"{"type":"system","message":{"content":"ignored"}}"#,
        );
        let turns = Source::ClaudeCode.parse(raw);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[0].text, "deploy with make ship");
        assert_eq!(turns[1].role, "assistant");
        assert_eq!(turns[1].text, "Done.");
    }

    #[test]
    fn empty_text_turns_are_dropped() {
        let raw =
            r#"{"kind":"Prompt","data":{"content":[{"kind":"thinking","data":{"text":"x"}}]}}"#;
        // No text block -> empty text -> dropped.
        assert!(Source::Kiro.parse(raw).is_empty());
    }
}
