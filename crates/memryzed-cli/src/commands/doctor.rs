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

//! `memryzed doctor` implementation.
//!
//! Runs a series of checks and prints a summary. v0.1.0-alpha.1
//! covers the binary-level checks: version, executable location,
//! data directory, configuration file. Database, embedding model,
//! and integration checks are added as those features land.

use std::env;

use anyhow::Result;

use crate::commands::Context;
use crate::exit;

pub fn run(ctx: &Context) -> Result<()> {
    let mut report = Report::new();

    report.section("Installation");
    report.add(check_binary_path());
    report.add(check_version());

    report.section("Data");
    let data = check_data_dir(ctx);
    let data_ok = matches!(data.status, Status::Ok | Status::Skip);
    report.add(data);
    if data_ok {
        report.add(check_config_file(ctx));
    }

    report.section("Memory and integrations");
    report.add(skipped(
        "Database integrity",
        "no database in v0.1.0-alpha.1; lands in alpha.2",
    ));
    report.add(skipped(
        "Embedding model",
        "no embeddings in v0.1.0-alpha.1; lands in alpha.3",
    ));
    report.add(skipped(
        "MCP integrations",
        "auto-detect lands in v0.1.0-beta.1",
    ));

    report.print(ctx.quiet);

    if report.has_failures() {
        return Err(exit::Coded::new(exit::GENERAL_ERROR, "doctor reported failures").into());
    }
    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Status {
    Ok,
    Fail,
    Skip,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Ok => "ok  ",
            Status::Fail => "fail",
            Status::Skip => "skip",
        }
    }
}

struct CheckResult {
    name: String,
    status: Status,
    detail: Option<String>,
}

struct Report {
    sections: Vec<(String, Vec<CheckResult>)>,
}

impl Report {
    fn new() -> Self {
        Self { sections: vec![] }
    }

    fn section(&mut self, name: &str) {
        self.sections.push((name.to_string(), vec![]));
    }

    fn add(&mut self, r: CheckResult) {
        self.sections
            .last_mut()
            .expect("section must be declared before adding checks")
            .1
            .push(r);
    }

    fn has_failures(&self) -> bool {
        self.sections
            .iter()
            .flat_map(|(_, checks)| checks.iter())
            .any(|c| c.status == Status::Fail)
    }

    fn print(&self, quiet: bool) {
        if quiet {
            return;
        }
        println!("Memryzed doctor");
        for (name, checks) in &self.sections {
            println!();
            println!("{name}");
            for c in checks {
                let label = c.status.label();
                match &c.detail {
                    Some(detail) => println!("  [{}] {} - {}", label, c.name, detail),
                    None => println!("  [{}] {}", label, c.name),
                }
            }
        }
        println!();
        if self.has_failures() {
            println!("Doctor reported failures.");
        } else {
            println!("All systems healthy.");
        }
    }
}

fn ok(name: &str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name: name.to_string(),
        status: Status::Ok,
        detail: Some(detail.into()),
    }
}

fn fail(name: &str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name: name.to_string(),
        status: Status::Fail,
        detail: Some(detail.into()),
    }
}

fn skipped(name: &str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name: name.to_string(),
        status: Status::Skip,
        detail: Some(detail.into()),
    }
}

fn check_binary_path() -> CheckResult {
    match env::current_exe() {
        Ok(path) => ok("Binary location", path.display().to_string()),
        Err(err) => fail(
            "Binary location",
            format!("could not determine current_exe: {err}"),
        ),
    }
}

fn check_version() -> CheckResult {
    ok("Version", memryzed_core::VERSION.to_string())
}

fn check_data_dir(ctx: &Context) -> CheckResult {
    let dir = match ctx.data_dir() {
        Ok(d) => d,
        Err(err) => return fail("Data directory", err.to_string()),
    };
    if dir.exists() {
        ok("Data directory", dir.root().display().to_string())
    } else {
        fail(
            "Data directory",
            format!(
                "{} does not exist; run `memryzed init`",
                dir.root().display()
            ),
        )
    }
}

fn check_config_file(ctx: &Context) -> CheckResult {
    let dir = match ctx.data_dir() {
        Ok(d) => d,
        Err(err) => return fail("Configuration", err.to_string()),
    };
    let path = dir.config_file();
    if path.is_file() {
        ok("Configuration", path.display().to_string())
    } else {
        fail(
            "Configuration",
            format!("missing at {}; run `memryzed init`", path.display()),
        )
    }
}
