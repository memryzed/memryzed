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
    Command::cargo_bin("memryzed").expect("memryzed binary built")
}

#[test]
fn version_flag_prints_workspace_version() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0-alpha.1"));
}

#[test]
fn help_flag_succeeds() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("memryzed"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("doctor"));
}

#[test]
fn no_subcommand_prints_help_and_succeeds() {
    cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("memryzed"));
}

#[test]
fn init_creates_data_directory_and_config() {
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

    // Both runs leave the same files in place.
    assert!(dir.join("config.toml").is_file());
    assert!(dir.join("bin").is_dir());
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
        .stdout(predicate::str::contains("All systems healthy"));
}

#[test]
fn unimplemented_subcommand_fails_with_message() {
    cmd()
        .arg("serve")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}
