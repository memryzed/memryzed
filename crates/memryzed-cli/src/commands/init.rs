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
//! Creates the data directory, writes a default `config.toml`,
//! initializes the database, and downloads the embedding model.
//! Idempotent: a second `init` does not overwrite existing files.

use std::fs;

use anyhow::{Context as _, Result};

use memryzed_core::embedder::{make_default, ENV_DISABLE};
use memryzed_core::Database;

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
        println!("    db.sqlite        memory store");
        println!("    bin/             reserved for the binary directory");
        println!("    models/          embedding model cache (~130 MB on first run)");
        println!();
    }

    let already_existed = data_dir.exists();

    fs::create_dir_all(&root)
        .with_context(|| format!("creating data directory at {}", root.display()))?;
    fs::create_dir_all(data_dir.bin_dir())
        .with_context(|| format!("creating bin directory at {}", data_dir.bin_dir().display()))?;
    fs::create_dir_all(data_dir.models_dir()).with_context(|| {
        format!(
            "creating models directory at {}",
            data_dir.models_dir().display()
        )
    })?;

    let config_path = data_dir.config_file();
    let wrote_config = if config_path.exists() {
        false
    } else {
        fs::write(&config_path, DEFAULT_CONFIG)
            .with_context(|| format!("writing default config at {}", config_path.display()))?;
        true
    };

    // Open (and migrate) the database. Idempotent.
    let db_path = data_dir.db_file();
    let db_already_existed = db_path.exists();
    let db = Database::open(&db_path)
        .with_context(|| format!("initializing database at {}", db_path.display()))?;
    db.integrity_check()
        .with_context(|| "post-init integrity check failed")?;

    // Warm-load the embedding model, keeping the handle for backfill.
    // Skipped if MEMRYZED_DISABLE_EMBEDDING is set.
    let embedding_disabled = std::env::var(ENV_DISABLE).is_ok();
    let embedder_status = if embedding_disabled {
        EmbedderInit::Skipped
    } else {
        if !ctx.quiet {
            println!("Loading embedding model (downloads on first run)...");
        }
        match make_default(&data_dir.models_dir()) {
            Ok(e) => EmbedderInit::Loaded {
                model: e.model_id().to_string(),
                dim: e.dimension(),
            },
            Err(e) => EmbedderInit::Failed(e.to_string()),
        }
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
        if db_already_existed {
            println!(
                "Database already present at {}; ran migrations to schema v{}.",
                db_path.display(),
                db.schema_version()?
            );
        } else {
            println!(
                "Created database at {} (schema v{}).",
                db_path.display(),
                db.schema_version()?
            );
        }
        match embedder_status {
            EmbedderInit::Loaded { model, dim } => {
                let dim_label = dim.map(|d| d.to_string()).unwrap_or_else(|| "?".into());
                println!("Loaded embedding model {model} (dim={dim_label}).");
            }
            EmbedderInit::Skipped => {
                println!("Skipped embedding model (MEMRYZED_DISABLE_EMBEDDING set).");
            }
            EmbedderInit::Failed(e) => {
                println!("Embedding model could not be loaded: {e}");
                println!("Memryzed will store memories without embeddings until this resolves.");
            }
        }
    }

    if !ctx.quiet {
        println!();
        println!("Memryzed is initialized.");
        println!(
            "Your agent history is imported and embedded automatically in the \
             background while your agent runs. No further commands are needed."
        );
        println!("Run `memryzed doctor` to verify, or `memryzed --help` for all commands.");
    }

    Ok(())
}

enum EmbedderInit {
    Loaded { model: String, dim: Option<usize> },
    Skipped,
    Failed(String),
}
