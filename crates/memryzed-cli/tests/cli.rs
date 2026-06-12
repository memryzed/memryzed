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

//! Integration tests spawning the `memryzed` binary.

use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    let mut c = Command::cargo_bin("memryzed").expect("memryzed binary built");
    // Tests must never trigger a real model download.
    c.env("MEMRYZED_DISABLE_EMBEDDING", "1");
    c
}

#[test]
fn version_flag_prints_workspace_version() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.7.0"));
}

#[test]
fn help_flag_succeeds() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("memryzed"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("remember"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("forget"));
}

#[test]
fn no_subcommand_prints_help_and_succeeds() {
    cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("memryzed"));
}

#[test]
fn init_creates_data_directory_database_and_config() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    assert!(dir.exists(), "init must create the data directory");
    assert!(
        dir.join("config.toml").is_file(),
        "init must write config.toml"
    );
    assert!(dir.join("bin").is_dir(), "init must create bin/");
    assert!(
        dir.join("db.sqlite").is_file(),
        "init must create db.sqlite"
    );
}

#[test]
fn init_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    for _ in 0..2 {
        cmd()
            .arg("--data-dir")
            .arg(&dir)
            .arg("init")
            .arg("--yes")
            .assert()
            .success();
    }

    assert!(dir.join("config.toml").is_file());
    assert!(dir.join("bin").is_dir());
    assert!(dir.join("db.sqlite").is_file());
}

#[test]
fn doctor_fails_before_init() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("doctor")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Data directory"))
        .stdout(predicate::str::contains("does not exist"));
}

#[test]
fn doctor_succeeds_after_init() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Database integrity"))
        .stdout(predicate::str::contains("schema v6"))
        .stdout(predicate::str::contains("All systems healthy"));
}

#[test]
fn remember_then_list_shows_the_memory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("I prefer pnpm")
        .arg("--scope")
        .arg("global")
        .assert()
        .success()
        .stdout(predicate::str::contains("Stored memory"))
        .stdout(predicate::str::contains("I prefer pnpm"));

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("I prefer pnpm"))
        .stdout(predicate::str::contains("global"));
}

#[test]
fn remember_with_pin_lists_first_with_marker() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("non-pinned thought")
        .arg("--scope")
        .arg("global")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("important fact")
        .arg("--scope")
        .arg("global")
        .arg("--pin")
        .assert()
        .success();

    let output = cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).to_string();

    let pinned_pos = text.find("important fact").expect("pinned listed");
    let normal_pos = text.find("non-pinned thought").expect("normal listed");
    assert!(
        pinned_pos < normal_pos,
        "pinned memory must list first; got:\n{text}"
    );
}

#[test]
fn show_prints_full_memory_detail() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    let stdout = cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("hello world")
        .arg("--scope")
        .arg("global")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&stdout).to_string();
    let id = stdout
        .split_whitespace()
        .find(|t| t.starts_with("mem_"))
        .expect("ID present in remember output")
        .to_string();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("show")
        .arg(&id)
        .assert()
        .success()
        .stdout(predicate::str::contains(&id))
        .stdout(predicate::str::contains("hello world"))
        .stdout(predicate::str::contains("Scope"))
        .stdout(predicate::str::contains("Kind"));
}

#[test]
fn forget_archives_then_list_excludes_by_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    let stdout = cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("forget me")
        .arg("--scope")
        .arg("global")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&stdout).to_string();
    let id = stdout
        .split_whitespace()
        .find(|t| t.starts_with("mem_"))
        .expect("ID present")
        .to_string();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("forget")
        .arg(&id)
        .assert()
        .success()
        .stdout(predicate::str::contains("Archived"));

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No memories match the filter"));

    // Archived memories are still visible with --status archived.
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("list")
        .arg("--status")
        .arg("archived")
        .assert()
        .success()
        .stdout(predicate::str::contains("forget me"));
}

#[test]
fn forget_unknown_id_returns_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("forget")
        .arg("mem_doesnotexist")
        .assert()
        .failure();
}

#[test]
fn remember_unknown_scope_returns_misuse() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("x")
        .arg("--scope")
        .arg("wing")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn sessions_empty_then_resume_reports_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("sessions")
        .assert()
        .success()
        .stdout(predicate::str::contains("No sessions"));

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("resume")
        .assert()
        .success()
        .stdout(predicate::str::contains("No resumable session"));
}

#[test]
fn update_check_returns_a_release_status() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    // `update --check` reports without attempting an install.
    // The exit code is zero whether or not a network check succeeds:
    // an offline test still reports a clean "current" or "unknown"
    // status. A non-zero exit would signal a real failure.
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("update")
        .arg("--check")
        .assert()
        .success();
}

#[test]
fn install_print_emits_a_config_block() {
    cmd()
        .arg("install")
        .arg("--print")
        .arg("--client")
        .arg("kiro")
        .assert()
        .success()
        .stdout(predicate::str::contains("mcpServers"))
        .stdout(predicate::str::contains("memryzed"))
        .stdout(predicate::str::contains("serve"));
}

#[test]
fn config_set_then_get_round_trips() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("config")
        .arg("set")
        .arg("memory.auto_approve_threshold")
        .arg("0.7")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("config")
        .arg("get")
        .arg("memory.auto_approve_threshold")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.7"));
}

#[test]
fn export_then_import_round_trips_a_memory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("b");
    let dump = tmp.path().join("dump.json");

    // Seed store A.
    cmd()
        .arg("--data-dir")
        .arg(&dir_a)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();
    cmd()
        .arg("--data-dir")
        .arg(&dir_a)
        .arg("remember")
        .arg("portable fact")
        .arg("--scope")
        .arg("global")
        .assert()
        .success();

    // Export A to a file.
    let out = cmd()
        .arg("--data-dir")
        .arg(&dir_a)
        .arg("export")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    std::fs::write(&dump, out).unwrap();

    // Import into fresh store B.
    cmd()
        .arg("--data-dir")
        .arg(&dir_b)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();
    cmd()
        .arg("--data-dir")
        .arg(&dir_b)
        .arg("import")
        .arg(&dump)
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("new memories"));

    // B now lists the fact.
    cmd()
        .arg("--data-dir")
        .arg(&dir_b)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("portable fact"));
}

#[test]
fn log_prints_nothing_gracefully_when_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();
    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("log")
        .assert()
        .success();
}

#[test]
fn doctor_reports_embedder_skip_when_disabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Embedding model"))
        .stdout(predicate::str::contains("embedder disabled"));
}

#[test]
fn remember_announces_skipped_embedding_when_disabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("memryzed");

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("init")
        .arg("--yes")
        .assert()
        .success();

    cmd()
        .arg("--data-dir")
        .arg(&dir)
        .arg("remember")
        .arg("ephemeral note")
        .arg("--scope")
        .arg("global")
        .assert()
        .success()
        .stdout(predicate::str::contains("Stored memory"))
        .stdout(predicate::str::contains("embedding: skipped"));
}
