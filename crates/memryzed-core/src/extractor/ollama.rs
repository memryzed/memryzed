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

//! Optional Ollama-backed extractor.
//!
//! When enabled in config, sends a message to a local Ollama
//! instance and asks a small model to propose memories as JSON.
//! Off by default. When Ollama is unreachable or returns malformed
//! output, callers fall back to the rule-based extractor.
//!
//! This talks only to the user's local Ollama; nothing leaves the
//! machine.

use serde::Deserialize;
use serde_json::json;

use crate::extractor::Candidate;
use crate::memory::{Kind, Scope};

/// Configuration for the Ollama extractor.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Base URL of the Ollama HTTP API.
    pub url: String,
    /// Model tag to use, for example "qwen2.5:3b".
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:11434".to_string(),
            model: "qwen2.5:3b".to_string(),
            timeout_secs: 30,
        }
    }
}

const SYSTEM_PROMPT: &str = "\
You extract durable facts and preferences from a developer's message \
for a memory system. Return ONLY a JSON array. Each element is an \
object with keys: content (string, the fact in third person), scope \
(one of global, project), kind (one of preference, fact, decision, \
todo), confidence (number 0..1). If nothing is worth remembering, \
return []. Do not include any prose outside the JSON.";

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct RawCandidate {
    content: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    confidence: Option<f64>,
}

/// Ask Ollama to extract candidates from a message.
///
/// Returns `Ok(None)` when Ollama is unreachable (the caller should
/// fall back to rule-based). Returns `Ok(Some(vec))` on success,
/// where the vec may be empty. Malformed model output yields an
/// empty vec rather than an error, since the model is free-form.
pub fn extract(config: &OllamaConfig, message: &str) -> Option<Vec<Candidate>> {
    let endpoint = format!("{}/api/generate", config.url.trim_end_matches('/'));
    let body = json!({
        "model": config.model,
        "system": SYSTEM_PROMPT,
        "prompt": message,
        "stream": false,
        "format": "json",
    });

    let resp = ureq::post(&endpoint)
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .send_json(body);

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                target: "memryzed::extractor::ollama",
                error = %e,
                "Ollama unreachable; falling back to rule-based"
            );
            return None;
        }
    };

    let parsed: OllamaGenerateResponse = match resp.into_json() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(target: "memryzed::extractor::ollama", error = %e, "bad Ollama envelope");
            return Some(Vec::new());
        }
    };

    Some(parse_candidates(&parsed.response))
}

/// Parse the model's JSON text into candidates. Tolerant: unknown
/// scope/kind values fall back to sensible defaults, out-of-range
/// confidence is clamped, empty content is dropped.
pub fn parse_candidates(text: &str) -> Vec<Candidate> {
    let raw: Vec<RawCandidate> = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    raw.into_iter()
        .filter_map(|r| {
            let content = r.content.trim().to_string();
            if content.is_empty() {
                return None;
            }
            let scope = match r.scope.as_deref() {
                Some("project") => Scope::Project,
                _ => Scope::Global,
            };
            let kind = match r.kind.as_deref() {
                Some("preference") => Kind::Preference,
                Some("decision") => Kind::Decision,
                Some("todo") => Kind::Todo,
                _ => Kind::Fact,
            };
            let confidence = r.confidence.unwrap_or(0.7).clamp(0.0, 1.0);
            Some(Candidate {
                content,
                scope,
                kind,
                confidence,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_well_formed_json() {
        let text = r#"[
            {"content": "Prefers pnpm over npm", "scope": "global", "kind": "preference", "confidence": 0.9},
            {"content": "Repo uses Vitest", "scope": "project", "kind": "fact", "confidence": 0.8}
        ]"#;
        let cands = parse_candidates(text);
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].kind, Kind::Preference);
        assert_eq!(cands[1].scope, Scope::Project);
    }

    #[test]
    fn empty_array_yields_no_candidates() {
        assert!(parse_candidates("[]").is_empty());
    }

    #[test]
    fn malformed_json_yields_no_candidates() {
        assert!(parse_candidates("not json at all").is_empty());
        assert!(parse_candidates("{\"content\": \"x\"}").is_empty()); // object, not array
    }

    #[test]
    fn unknown_scope_and_kind_default_safely() {
        let text = r#"[{"content":"x","scope":"wing","kind":"weird"}]"#;
        let cands = parse_candidates(text);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].scope, Scope::Global);
        assert_eq!(cands[0].kind, Kind::Fact);
        assert_eq!(cands[0].confidence, 0.7);
    }

    #[test]
    fn confidence_is_clamped() {
        let text = r#"[{"content":"x","confidence":5.0}]"#;
        let cands = parse_candidates(text);
        assert_eq!(cands[0].confidence, 1.0);
    }

    #[test]
    fn empty_content_dropped() {
        let text = r#"[{"content":"   "},{"content":"keep"}]"#;
        let cands = parse_candidates(text);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].content, "keep");
    }
}
