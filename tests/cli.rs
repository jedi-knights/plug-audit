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

#[test]
fn config_disables_rule_category() {
    let tmp = tempfile::tempdir().expect("mktemp");
    let cfg = tmp.path().join("plug-audit.toml");
    std::fs::write(&cfg, "[categories]\ndeps = false\n").unwrap();

    // With deps disabled, only nvim/health-check fires on sample-repo.
    let output = bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--config")
        .arg(&cfg)
        .arg("--format=json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["summary"]["must_fix"], 1);
    assert_eq!(value["summary"]["should_fix"], 0);
    // No deps/* findings.
    for f in value["findings"].as_array().unwrap() {
        let rule = f["rule"].as_str().unwrap();
        assert!(
            !rule.starts_with("deps/"),
            "unexpected deps rule after category disable: {rule}"
        );
    }
}

#[test]
fn config_severity_override_bumps_finding_severity() {
    let tmp = tempfile::tempdir().expect("mktemp");
    let cfg = tmp.path().join("plug-audit.toml");
    std::fs::write(&cfg, "[severity]\n\"deps/optional-peer\" = \"must-fix\"\n").unwrap();

    // Under override, --strict should trip because deps/optional-peer
    // findings on sample-repo now count as Must Fix.
    bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--config")
        .arg(&cfg)
        .arg("--strict")
        .assert()
        .code(2);
}

#[test]
fn config_typo_fails_with_helpful_error() {
    let tmp = tempfile::tempdir().expect("mktemp");
    let cfg = tmp.path().join("plug-audit.toml");
    std::fs::write(&cfg, "[rules]\n\"nvim/does-not-exist\" = false\n").unwrap();

    bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--config")
        .arg(&cfg)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("nvim/does-not-exist"));
}

#[test]
fn explicit_config_missing_file_is_tool_error() {
    bin()
        .arg("check")
        .arg(fixture_path("tests/fixtures/sample-repo"))
        .arg("--config")
        .arg("/tmp/plug-audit-does-not-exist.toml")
        .assert()
        .code(1);
}

#[test]
fn autodiscovered_config_at_scan_root_is_used() {
    let tmp = tempfile::tempdir().expect("mktemp");
    // Empty scan dir + a config file at its root that disables no
    // rules — the auto-discovery path must at least parse and validate.
    std::fs::write(tmp.path().join(".plug-audit.toml"), "").unwrap();
    bin().arg("check").arg(tmp.path()).assert().success();
}
