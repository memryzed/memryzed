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

//! `memryzed config` implementation.
//!
//! Reads and writes `~/.memryzed/config.toml`. The config is treated
//! as a generic TOML document; dotted keys address nested tables
//! (for example `memory.auto_approve_threshold`). It is deliberately
//! schema-light: it does not validate that a key is one Memryzed
//! recognizes, so users can stage settings freely.

use std::process::Command;

use anyhow::{Context as _, Result};
use toml::Value;

use crate::commands::Context;
use crate::exit;

pub enum Action {
    Show,
    Get { key: String },
    Set { key: String, value: String },
    Edit,
}

pub fn run(ctx: &Context, action: Action) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let path = data_dir.config_file();

    match action {
        Action::Show => {
            let raw = read_or_empty(&path)?;
            if !ctx.quiet {
                print!("{raw}");
                if !raw.ends_with('\n') {
                    println!();
                }
            }
        }
        Action::Get { key } => {
            let doc: Value = parse(&read_or_empty(&path)?)?;
            match lookup(&doc, &key) {
                Some(v) => println!("{}", render_scalar(v)),
                None => {
                    return Err(exit::Coded::new(
                        exit::GENERAL_ERROR,
                        format!("key {key:?} not found in config"),
                    )
                    .into())
                }
            }
        }
        Action::Set { key, value } => {
            let mut doc: Value = parse(&read_or_empty(&path)?)?;
            set_key(&mut doc, &key, &value)?;
            let serialized = toml::to_string_pretty(&doc)
                .map_err(|e| anyhow::anyhow!("failed to serialize config: {e}"))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, serialized)
                .with_context(|| format!("writing {}", path.display()))?;
            if !ctx.quiet {
                println!("Set {key} = {value}");
            }
        }
        Action::Edit => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            if !path.exists() {
                return Err(exit::Coded::new(
                    exit::CONFIG_ERROR,
                    format!("no config at {}; run `memryzed init`", path.display()),
                )
                .into());
            }
            let status = Command::new(&editor)
                .arg(&path)
                .status()
                .with_context(|| format!("launching editor {editor:?}"))?;
            if !status.success() {
                return Err(exit::Coded::new(
                    exit::GENERAL_ERROR,
                    format!("editor {editor:?} exited with failure"),
                )
                .into());
            }
        }
    }
    Ok(())
}

fn read_or_empty(path: &std::path::Path) -> Result<String> {
    if path.is_file() {
        Ok(std::fs::read_to_string(path)?)
    } else {
        Ok(String::new())
    }
}

fn parse(raw: &str) -> Result<Value> {
    if raw.trim().is_empty() {
        return Ok(Value::Table(Default::default()));
    }
    raw.parse::<Value>().map_err(|e| {
        exit::Coded::new(exit::CONFIG_ERROR, format!("invalid config TOML: {e}")).into()
    })
}

fn lookup<'a>(doc: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut current = doc;
    for part in dotted.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn set_key(doc: &mut Value, dotted: &str, raw_value: &str) -> Result<()> {
    let parts: Vec<&str> = dotted.split('.').collect();
    if parts.is_empty() {
        return Err(exit::Coded::new(exit::MISUSE, "empty config key").into());
    }
    // Ensure the root is a table.
    if !doc.is_table() {
        *doc = Value::Table(Default::default());
    }
    let mut current = doc;
    for part in &parts[..parts.len() - 1] {
        let table = current
            .as_table_mut()
            .ok_or_else(|| exit::Coded::new(exit::CONFIG_ERROR, "config path is not a table"))?;
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| Value::Table(Default::default()));
    }
    let last = parts[parts.len() - 1];
    let table = current
        .as_table_mut()
        .ok_or_else(|| exit::Coded::new(exit::CONFIG_ERROR, "config path is not a table"))?;
    table.insert(last.to_string(), coerce(raw_value));
    Ok(())
}

/// Coerce a CLI string into the most natural TOML scalar: bool,
/// integer, float, otherwise string.
fn coerce(raw: &str) -> Value {
    if let Ok(b) = raw.parse::<bool>() {
        return Value::Boolean(b);
    }
    if let Ok(i) = raw.parse::<i64>() {
        return Value::Integer(i);
    }
    if let Ok(f) = raw.parse::<f64>() {
        return Value::Float(f);
    }
    Value::String(raw.to_string())
}

fn render_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
