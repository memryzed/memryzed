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

//! Secret redaction for captured conversation text.
//!
//! Design principle: **precision over recall**. A coding-agent
//! transcript is full of long, high-entropy strings that are NOT
//! secrets, commit hashes, UUIDs, base64 payloads, file paths, minified
//! code. Redacting those would silently corrupt the user's memory and
//! make recall worse. So this module never guesses from entropy or
//! length alone. It only redacts text that is *structurally
//! unmistakable* as a credential:
//!
//! 1. Vendor tokens with a fixed, unique prefix (AWS, GitHub, Slack,
//!    Stripe, OpenAI, Google, npm, etc.) whose format no ordinary
//!    word matches.
//! 2. PEM private-key blocks.
//! 3. Explicit secret assignments, a key whose *name* says secret
//!    (password, secret, token, api_key, ...) followed by a value.
//!
//! Everything else is left verbatim. The match is replaced with a
//! `[REDACTED:<kind>]` marker so the surrounding text, and the fact
//! that a secret was present, is preserved for context.

use std::sync::OnceLock;

use regex::Regex;

/// A single redaction rule: a label and the pattern that identifies a
/// secret with high confidence.
struct Rule {
    kind: &'static str,
    re: Regex,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut rules: Vec<Rule> = Vec::new();
        let mut add = |kind: &'static str, pat: &str| {
            rules.push(Rule {
                kind,
                re: Regex::new(pat).expect("valid redaction regex"),
            });
        };

    // PEM private key blocks (any key type). Multiline.
    add(
        "private_key",
        r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
    );

    // AWS access key id (AKIA/ASIA + 16 uppercase alnum).
    add("aws_access_key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b");

    // GitHub tokens (ghp_, gho_, ghu_, ghs_, ghr_, github_pat_).
    add(
        "github_token",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{60,})\b",
    );

    // Slack tokens (xoxb-, xoxp-, xoxa-, xoxr-).
    add("slack_token", r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b");

    // Stripe live/test secret keys.
    add("stripe_key", r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}\b");

    // OpenAI / Anthropic style keys.
    add("openai_key", r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b");
    add("anthropic_key", r"\bsk-ant-[A-Za-z0-9_-]{20,}\b");

    // Google API key.
    add("google_api_key", r"\bAIza[0-9A-Za-z_-]{35}\b");

    // npm token.
    add("npm_token", r"\bnpm_[A-Za-z0-9]{36}\b");

    // JWT (three base64url segments). The header almost always starts
    // with eyJ (base64 of '{"'), which makes this specific enough.
    add(
        "jwt",
        r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
    );

    // Explicit secret assignment: a key whose NAME marks it secret,
    // then = or : then a quoted-or-bare value. We only redact the
    // value, and require the value to be non-trivial (>= 6 chars,
    // not an obvious placeholder). The key name requirement is what
    // keeps this from matching ordinary `name = value` text.
    add(
        "assigned_secret",
        r#"(?i)(?:password|passwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|private[_-]?key|bearer)\b\s*[:=]\s*["']?([^\s"']{6,})["']?"#,
    );

        rules
    })
}

/// Obvious placeholder values that an assignment rule must not redact,
/// they carry no secret and redacting them would be noise.
fn is_placeholder(v: &str) -> bool {
    let lc = v.to_ascii_lowercase();
    matches!(
        lc.as_str(),
        "none" | "null" | "true" | "false" | "changeme" | "xxx" | "xxxx" | "todo" | "example"
    ) || lc.starts_with("your") // your_api_key, your-token-here
        || lc.starts_with("<") // <token>, <your-key>
        || lc.starts_with("${") // ${ENV_VAR}
        || lc.chars().all(|c| c == '*' || c == 'x' || c == '.') // masked
}

/// Replace high-confidence secrets in `text` with `[REDACTED:<kind>]`
/// markers, leaving all other text untouched. Returns the (possibly
/// unchanged) text and the number of redactions made.
pub fn redact(text: &str) -> (String, usize) {
    let mut out = text.to_string();
    let mut count = 0;

    for rule in rules().iter() {
        // The assigned_secret rule has a value capture group; redact
        // only the value so the key name stays for context. All other
        // rules redact the whole match.
        if rule.kind == "assigned_secret" {
            let mut result = String::with_capacity(out.len());
            let mut last = 0;
            for caps in rule.re.captures_iter(&out) {
                let whole = caps.get(0).unwrap();
                let val = caps.get(1).unwrap();
                if is_placeholder(val.as_str()) {
                    continue;
                }
                result.push_str(&out[last..val.start()]);
                result.push_str("[REDACTED:secret]");
                last = val.end();
                let _ = whole;
                count += 1;
            }
            if last > 0 {
                result.push_str(&out[last..]);
                out = result;
            }
        } else {
            let before = count;
            out = rule
                .re
                .replace_all(&out, |_: &regex::Captures<'_>| {
                    count += 1;
                    format!("[REDACTED:{}]", rule.kind)
                })
                .into_owned();
            let _ = before;
        }
    }

    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_vendor_tokens() {
        for s in [
            "my key is AKIAIOSFODNN7EXAMPLE here",
            "token ghp_1234567890abcdefghijklmnopqrstuvwxyz",
            "use xoxb-123456789012-abcdefghijklmno",
            "sk_live_4eC39HqLyjWDarjtT1zdp7dc",
            "stripe sk_test_abcdefghijklmnop1234",
            "AIzaA1234567890abcdefghijklmnopqrstuvwx",
        ] {
            let (out, n) = redact(s);
            assert!(n >= 1, "expected redaction in: {s}");
            assert!(out.contains("[REDACTED:"), "marker missing: {out}");
        }
    }

    #[test]
    fn redacts_private_key_block() {
        let s = "before\n-----BEGIN RSA PRIVATE KEY-----\nMIIB...lines...\n-----END RSA PRIVATE KEY-----\nafter";
        let (out, n) = redact(s);
        assert_eq!(n, 1);
        assert!(out.contains("before"));
        assert!(out.contains("after"));
        assert!(out.contains("[REDACTED:private_key]"));
        assert!(!out.contains("MIIB"));
    }

    #[test]
    fn redacts_assigned_secret_value_only() {
        let (out, n) = redact("DATABASE_PASSWORD=s3cr3tP@ssw0rd123");
        assert_eq!(n, 1);
        assert!(out.contains("PASSWORD"), "key name kept: {out}");
        assert!(out.contains("[REDACTED:secret]"));
        assert!(!out.contains("s3cr3tP@ssw0rd123"));
    }

    #[test]
    fn does_not_redact_ordinary_text() {
        // These must survive untouched: prose, code, paths, hashes,
        // UUIDs, version numbers, normal assignments.
        for s in [
            "Let's deploy the service to the staging cluster tomorrow.",
            "the commit hash is a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
            "id = 550e8400-e29b-41d4-a716-446655440000",
            "let count = items.len() + 1;",
            "the file is at /usr/local/bin/memryzed",
            "set the timeout = 30 seconds",
            "name = \"my-project\"",
            "version = 0.6.0",
            // Long base64-ish data that is NOT a credential.
            "the payload was SGVsbG8gd29ybGQgdGhpcyBpcyBub3QgYSBzZWNyZXQK",
            // A bearer mention without an actual token value.
            "we use bearer auth on that endpoint",
            // An identifier that contains 'key' but is a column name.
            "the primary key is the user_id column",
            // Public key reference (only PRIVATE keys are secrets).
            "paste your public_key into the form",
        ] {
            let (out, n) = redact(s);
            assert_eq!(n, 0, "false positive on: {s} -> {out}");
            assert_eq!(out, s);
        }
    }

    #[test]
    fn skips_placeholder_secret_values() {
        for s in [
            "api_key=your_api_key_here",
            "password = changeme",
            "secret: ${SECRET_ENV}",
            "token=<your-token>",
        ] {
            let (_, n) = redact(s);
            assert_eq!(n, 0, "redacted a placeholder: {s}");
        }
    }
}
