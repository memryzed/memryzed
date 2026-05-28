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

//! `memryzed init` implementation.
//!
//! Creates the data directory and writes a default `config.toml`.
//! Idempotent: a second `init` does not overwrite existing files.
//!
//! The full v1 spec calls for downloading the embedding model here.
//! That step is added in v0.1.0-alpha.3 when the embedder lands.

use std::fs;

use anyhow::{Context as _, Result};

use crate::commands::Context;

const DEFAULT_CONFIG: &str = include_str!("../../assets/default-config.toml");

pub fn run(ctx: &Context, _yes: bool) -> Result<()> {
    let data_dir = ctx.data_dir()?;
    let root = data_dir.root().to_path_buf();

    if !ctx.quiet {
        println!("Memryzed init");
        println!();
        println!("This will create:");
        println!("  {}", root.display());
        println!("    config.toml      default configuration");
        println!("    bin/             reserved for the binary directory");
        println!();
    }

    let already_existed = data_dir.exists();

    fs::create_dir_all(&root)
        .with_context(|| format!("creating data directory at {}", root.display()))?;
    fs::create_dir_all(data_dir.bin_dir())
        .with_context(|| format!("creating bin directory at {}", data_dir.bin_dir().display()))?;

    let config_path = data_dir.config_file();
    let wrote_config = if config_path.exists() {
        false
    } else {
        fs::write(&config_path, DEFAULT_CONFIG)
            .with_context(|| format!("writing default config at {}", config_path.display()))?;
        true
    };

    if !ctx.quiet {
        if already_existed {
            println!(
                "Memryzed data directory already exists at {}.",
                root.display()
            );
        } else {
            println!("Created {}.", root.display());
        }
        if wrote_config {
            println!("Wrote default configuration at {}.", config_path.display());
        } else {
            println!(
                "Configuration already present at {}; left unchanged.",
                config_path.display()
            );
        }
        println!();
        println!("Memryzed is initialized.");
        println!(
            "Run `memryzed doctor` to verify, or `memryzed --help` for the full command list."
        );
    }

    Ok(())
}
