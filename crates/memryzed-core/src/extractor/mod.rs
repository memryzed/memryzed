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

//! Rule-based memory extraction.
//!
//! Scans a user message for high-signal patterns and proposes
//! candidate memories. Each candidate carries a confidence score.
//! Candidates at or above the configured auto-approve threshold are
//! stored approved; the rest go to the pending queue for review.
//!
//! This is the conservative extractor described in
//! `docs/specs/v1.md` section 13. An optional Ollama-based extractor
//! is available for richer candidate extraction.

use std::sync::OnceLock;

use regex::Regex;

use crate::memory::{Kind, Scope};

pub mod ollama;

/// A proposed memory produced by the extractor.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The fact text, normalized.
    pub content: String,
    /// Suggested scope.
    pub scope: Scope,
    /// Suggested kind.
    pub kind: Kind,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
}

struct Pattern {
    re: Regex,
    kind: Kind,
    scope: Scope,
    confidence: f64,
    /// Render the captured groups into the stored content. The
    /// closure receives the regex captures.
    render: fn(&regex::Captures) -> Option<String>,
}

fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // Direct request to remember -> auto-approve confidence.
            Pattern {
                re: Regex::new(r"(?i)^\s*(?:please\s+)?remember(?:\s+that)?\s+(.+?)\s*$").unwrap(),
                kind: Kind::Fact,
                scope: Scope::Global,
                confidence: 1.0,
                render: |c| c.get(1).map(|m| m.as_str().to_string()),
            },
            Pattern {
                re: Regex::new(r"(?i)^\s*don'?t\s+forget(?:\s+that)?\s+(.+?)\s*$").unwrap(),
                kind: Kind::Fact,
                scope: Scope::Global,
                confidence: 1.0,
                render: |c| c.get(1).map(|m| m.as_str().to_string()),
            },
            // Preference: "I prefer X over Y".
            Pattern {
                re: Regex::new(r"(?i)\bI\s+prefer\s+(.+?)\s+over\s+(.+?)\s*[.!]?\s*$").unwrap(),
                kind: Kind::Preference,
                scope: Scope::Global,
                confidence: 0.95,
                render: |c| match (c.get(1), c.get(2)) {
                    (Some(a), Some(b)) => Some(format!(
                        "Prefers {} over {}",
                        a.as_str().trim(),
                        b.as_str().trim()
                    )),
                    _ => None,
                },
            },
            // Preference: "I always/usually use X".
            Pattern {
                re: Regex::new(r"(?i)\bI\s+(?:always|usually)\s+use\s+(.+?)\s*[.!]?\s*$").unwrap(),
                kind: Kind::Preference,
                scope: Scope::Global,
                confidence: 0.9,
                render: |c| c.get(1).map(|m| format!("Uses {}", m.as_str().trim())),
            },
            // Preference: "I never use X".
            Pattern {
                re: Regex::new(r"(?i)\bI\s+never\s+use\s+(.+?)\s*[.!]?\s*$").unwrap(),
                kind: Kind::Preference,
                scope: Scope::Global,
                confidence: 0.9,
                render: |c| {
                    c.get(1)
                        .map(|m| format!("Never uses {}", m.as_str().trim()))
                },
            },
            // Project fact: "this repo/project uses X".
            Pattern {
                re: Regex::new(
                    r"(?i)\bthis\s+(?:repo|repository|project|codebase)\s+uses\s+(.+?)\s*[.!]?\s*$",
                )
                .unwrap(),
                kind: Kind::Fact,
                scope: Scope::Project,
                confidence: 0.9,
                render: |c| c.get(1).map(|m| format!("Uses {}", m.as_str().trim())),
            },
            // Project fact: "the deploy/build/test/lint command is X".
            Pattern {
                re: Regex::new(
                    r"(?i)\bthe\s+(deploy|build|test|lint)\s+command\s+is\s+(.+?)\s*[.!]?\s*$",
                )
                .unwrap(),
                kind: Kind::Fact,
                scope: Scope::Project,
                confidence: 0.9,
                render: |c| match (c.get(1), c.get(2)) {
                    (Some(verb), Some(cmd)) => Some(format!(
                        "The {} command is {}",
                        verb.as_str().to_lowercase(),
                        cmd.as_str().trim()
                    )),
                    _ => None,
                },
            },
        ]
    })
}

/// Extract candidate memories from a single user message.
///
/// Returns at most one candidate per matching pattern. The
/// highest-confidence "remember that ..." form short-circuits other
/// matches so explicit requests are not double-counted.
pub fn extract(message: &str) -> Vec<Candidate> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut saw_explicit = false;
    for p in patterns() {
        if let Some(caps) = p.re.captures(trimmed) {
            if let Some(content) = (p.render)(&caps) {
                let content = normalize(&content);
                if content.is_empty() {
                    continue;
                }
                if p.confidence >= 1.0 {
                    saw_explicit = true;
                }
                out.push(Candidate {
                    content,
                    scope: p.scope,
                    kind: p.kind,
                    confidence: p.confidence,
                });
            }
        }
    }

    // When the user said "remember that ...", keep only that explicit
    // candidate; the inner clause may also match a preference rule.
    if saw_explicit {
        out.retain(|c| c.confidence >= 1.0);
        out.truncate(1);
    }

    dedup(out)
}

fn normalize(s: &str) -> String {
    let trimmed = s.trim().trim_end_matches(['.', '!', ' ']).trim();
    trimmed.to_string()
}

fn dedup(mut candidates: Vec<Candidate>) -> Vec<Candidate> {
    candidates.dedup_by(|a, b| a.content.eq_ignore_ascii_case(&b.content));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_is_high_confidence_and_exclusive() {
        let c = extract("remember that I prefer uv over pip");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].confidence, 1.0);
        assert_eq!(c[0].content, "I prefer uv over pip");
    }

    #[test]
    fn prefer_over_pattern() {
        let c = extract("I prefer pnpm over npm");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, Kind::Preference);
        assert_eq!(c[0].scope, Scope::Global);
        assert_eq!(c[0].content, "Prefers pnpm over npm");
        assert!(c[0].confidence >= 0.9);
    }

    #[test]
    fn always_use_pattern() {
        let c = extract("I always use rustls for TLS");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].content, "Uses rustls for TLS");
    }

    #[test]
    fn never_use_pattern() {
        let c = extract("I never use force push on shared branches");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].content, "Never uses force push on shared branches");
    }

    #[test]
    fn project_uses_pattern_has_project_scope() {
        let c = extract("this repo uses Vitest");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].scope, Scope::Project);
        assert_eq!(c[0].content, "Uses Vitest");
    }

    #[test]
    fn deploy_command_pattern() {
        let c = extract("the deploy command is make ship");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].scope, Scope::Project);
        assert_eq!(c[0].content, "The deploy command is make ship");
    }

    #[test]
    fn no_match_returns_empty() {
        assert!(extract("what time is it?").is_empty());
        assert!(extract("").is_empty());
        assert!(extract("   ").is_empty());
    }

    #[test]
    fn trailing_punctuation_is_trimmed() {
        let c = extract("I prefer pnpm over npm.");
        assert_eq!(c[0].content, "Prefers pnpm over npm");
    }
}
