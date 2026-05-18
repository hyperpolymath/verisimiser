// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Acceptance suite for #51 (V-L3-I1): tracing is wired; the stdout data
// contract is never contaminated by diagnostics; `--log-format=json`
// validates against docs/logging.schema.json; `--log-level` and the
// `RUST_LOG` fallback work with the documented precedence.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verisimiser"))
}

/// `version --json` is data output: stdout must be exactly one JSON object,
/// with no diagnostic contamination, regardless of log format.
#[test]
fn version_json_data_stays_pure_on_stdout() {
    let out = bin()
        .args(["--log-format", "json", "version", "--json"])
        .output()
        .expect("run verisimiser");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is a single clean JSON value");
    assert!(v.get("version").is_some(), "version field present: {v}");
}

/// The `octad` table is data output and must land on stdout even with
/// JSON diagnostics enabled.
#[test]
fn octad_table_is_data_on_stdout() {
    let out = bin()
        .args(["octad", "--log-format", "json"])
        .output()
        .expect("run verisimiser");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("VeriSimDB Octad"),
        "octad data on stdout, got: {stdout}"
    );
}

/// `init` now produces *no* stdout (its old "Created …" line is a
/// diagnostic). With `--log-format=json` the diagnostic must appear on
/// stderr and validate against the documented schema.
#[test]
fn init_diagnostic_is_json_on_stderr_and_schema_valid() {
    let dir = tempfile::tempdir().unwrap();
    let out = bin()
        .current_dir(dir.path())
        .args(["--log-format", "json", "--log-level", "info", "init"])
        .output()
        .expect("run verisimiser init");
    assert!(out.status.success(), "init exits 0");

    // Data contract: init has no stdout payload.
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.trim().is_empty(),
        "init must not write to stdout, got: {stdout:?}"
    );

    // Diagnostic: a JSON line on stderr matching docs/logging.schema.json.
    let stderr = String::from_utf8(out.stderr).unwrap();
    let line = stderr
        .lines()
        .find(|l| l.contains("created manifest"))
        .unwrap_or_else(|| panic!("expected a 'created manifest' log line, got: {stderr}"));
    let log: serde_json::Value =
        serde_json::from_str(line).expect("each diagnostic line is valid JSON");

    // Documented stable fields.
    assert!(log["timestamp"].is_string(), "timestamp present");
    let level = log["level"].as_str().expect("level is a string");
    assert!(
        ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].contains(&level),
        "level in enum, got {level}"
    );
    assert!(log["target"].is_string(), "target present");
    assert_eq!(
        log["fields"]["message"], "created manifest",
        "fields.message carries the event message"
    );
    // The structured field on the event is preserved.
    assert!(
        log["fields"]["backend"].is_string(),
        "structured backend field present: {log}"
    );
}

/// `--log-level=error` filters out the info diagnostic; stdout still empty.
#[test]
fn log_level_filters_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let out = bin()
        .current_dir(dir.path())
        .args(["--log-format", "json", "--log-level", "error", "init"])
        .output()
        .expect("run verisimiser init");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        !stderr.contains("created manifest"),
        "info diagnostic suppressed at --log-level=error, got: {stderr}"
    );
}

/// `--log-level` overrides `RUST_LOG` (documented precedence:
/// --log-level > RUST_LOG > info).
#[test]
fn log_level_flag_overrides_rust_log_env() {
    let dir = tempfile::tempdir().unwrap();
    let out = bin()
        .current_dir(dir.path())
        .env("RUST_LOG", "error")
        .args(["--log-format", "json", "--log-level", "info", "init"])
        .output()
        .expect("run verisimiser init");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("created manifest"),
        "--log-level=info must override RUST_LOG=error, got: {stderr}"
    );
}

/// With no flag and no `RUST_LOG`, the default level is `info`.
#[test]
fn default_level_is_info() {
    let dir = tempfile::tempdir().unwrap();
    let out = bin()
        .current_dir(dir.path())
        .env_remove("RUST_LOG")
        .args(["--log-format", "json", "init"])
        .output()
        .expect("run verisimiser init");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("created manifest"),
        "info visible by default, got: {stderr}"
    );
}
