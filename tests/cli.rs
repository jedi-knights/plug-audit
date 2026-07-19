//! End-to-end CLI tests. Exercises the built binary against the same
//! fixture repo that drives the parser and discovery unit tests, so a
//! rule regression that only shows up at the CLI seam gets caught here
//! rather than in adoption.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

fn bin() -> Command {
    Command::cargo_bin("plug-audit").expect("binary built by cargo")
}

fn fixture_path(relative: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn check_on_sample_repo_reports_findings() {
    // sample-repo has no health.lua for the primary module → at least
    // one Must Fix finding. plugin/go-task.lua's unguarded requires
    // also trip deps/optional-peer.
    bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .assert()
        .success()
        .stdout(predicate::str::contains("## Findings"))
        .stdout(predicate::str::contains("### Must Fix"))
        .stdout(predicate::str::contains("nvim/health-check")); // rule anchor line contains the message
}

#[test]
fn check_on_empty_repo_reports_no_findings() {
    let tmp = tempfile::tempdir().expect("mktemp");
    bin()
        .arg("check")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no findings"));
}

#[test]
fn strict_mode_exits_2_on_must_fix() {
    bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--strict")
        .assert()
        .code(2);
}

#[test]
fn strict_mode_exits_0_when_no_must_fix() {
    // The rules-only fixtures under tests/fixtures/rules/nvim-plug-mapping
    // contain no plugin surface, so no health-check Must Fix fires.
    let tmp = tempfile::tempdir().expect("mktemp");
    bin()
        .arg("check")
        .arg(tmp.path())
        .arg("--strict")
        .assert()
        .success();
}

#[test]
fn help_prints_check_subcommand() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("check"));
}

#[test]
fn json_format_produces_parseable_output() {
    let output = bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--format=json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("--format=json emits parseable JSON");

    assert!(
        value["findings"].is_array(),
        "envelope must include a `findings` array"
    );
    assert!(value["summary"]["total"].is_number());
    assert!(value["summary"]["must_fix"].as_u64().unwrap() >= 1);
    assert_eq!(
        value["version"].as_str().unwrap(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn json_format_empty_repo_still_valid() {
    let tmp = tempfile::tempdir().expect("mktemp");
    let output = bin()
        .arg("check")
        .arg(tmp.path())
        .arg("--format=json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["summary"]["total"], 0);
    assert_eq!(value["findings"].as_array().unwrap().len(), 0);
}
