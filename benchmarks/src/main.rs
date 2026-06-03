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

//! Memryzed benchmark harness entry point.
//!
//! Usage:
//!
//! ```text
//! memryzed-bench --dataset path/to/normalized.json [--k 5,10] [--out result.json]
//! ```
//!
//! Datasets are not shipped in this repository. Convert a public
//! dataset (LongMemEval, LoCoMo, ConvoMem, MemBench) into the
//! normalized shape documented in `dataset.rs`, then point this
//! harness at the resulting file. See `benchmarks/README.md`.

mod dataset;
mod runner;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use dataset::Dataset;

#[derive(Debug, Parser)]
#[command(
    name = "memryzed-bench",
    about = "Quality benchmark harness for Memryzed."
)]
struct Cli {
    /// Path to a normalized dataset JSON file.
    #[arg(long, value_name = "PATH")]
    dataset: Option<PathBuf>,

    /// Directory of normalized dataset JSON files. Each is evaluated
    /// against its own haystack with one shared model load, and recall
    /// is aggregated across all questions. Used for per-scene
    /// benchmarks such as LongMemEval-S.
    #[arg(long, value_name = "PATH")]
    scene_dir: Option<PathBuf>,

    /// Comma-separated K values for recall@K.
    #[arg(long, default_value = "5,10")]
    k: String,

    /// Directory holding the embedding model cache.
    #[arg(long, value_name = "PATH")]
    models_dir: Option<PathBuf>,

    /// Write the result JSON to this path instead of stdout.
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let k_values: Vec<usize> = cli
        .k
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if k_values.is_empty() {
        anyhow::bail!("no valid K values parsed from --k");
    }

    let models_dir = cli
        .models_dir
        .or_else(|| {
            directories::BaseDirs::new().map(|d| d.home_dir().join(".memryzed").join("models"))
        })
        .unwrap_or_else(|| PathBuf::from(".models"));

    let result = match (cli.scene_dir, cli.dataset) {
        (Some(dir), _) => runner::run_scene_dir(&dir, &k_values, &models_dir)?,
        (None, Some(path)) => runner::run(&Dataset::load(&path)?, &k_values, &models_dir)?,
        (None, None) => anyhow::bail!("provide --dataset or --scene-dir"),
    };
    let json = serde_json::to_string_pretty(&result)?;

    match cli.out {
        Some(path) => {
            std::fs::write(&path, format!("{json}\n"))?;
            eprintln!("Wrote {}", path.display());
        }
        None => println!("{json}"),
    }
    Ok(())
}
