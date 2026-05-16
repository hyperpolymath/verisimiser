// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// `verisimiser doctor` — environment-level diagnostics. Mirrors `just
// doctor` with CLI semantics + `--json`. Closes #53.
//
// Distinction from `verisimiser validate`:
//
// - `validate` is *manifest-centric* — does this `verisimiser.toml` make
//   sense? Per-check failure surfaces TOML, schema, sidecar issues.
// - `doctor` is *environment-centric* — is this host fit to run
//   verisimiser at all? Reports on toolchain, PATH, working directory.
//   When a manifest path is supplied, also runs the manifest checks.

use std::process::Command;

use crate::manifest::{ValidationCheck, ValidationReport, validate_manifest};

/// Run all doctor checks. If `manifest_path` is `Some(_)`, the manifest
/// validation checks (from [`validate_manifest`]) are appended to the
/// environment checks.
pub fn run_doctor(manifest_path: Option<&str>) -> ValidationReport {
    let mut checks: Vec<ValidationCheck> = Vec::new();

    checks.push(check_command_in_path("cargo", "Rust toolchain (cargo)"));
    checks.push(check_command_in_path("git", "git in PATH"));
    checks.push(check_cwd_writable());

    let manifest_label = manifest_path.unwrap_or("<none>").to_string();
    if let Some(path) = manifest_path {
        let report = validate_manifest(path);
        checks.extend(report.checks);
    }

    let passed = checks.iter().all(|c| c.passed);
    ValidationReport {
        manifest: manifest_label,
        passed,
        checks,
    }
}

/// Check whether a CLI tool resolves on PATH. Runs `<cmd> --version` with
/// a short timeout-free invocation; we only care about exit status.
fn check_command_in_path(cmd: &str, description: &str) -> ValidationCheck {
    let name = format!("path-{}", cmd);
    let status = Command::new(cmd).arg("--version").output();
    match status {
        Ok(out) if out.status.success() => ValidationCheck {
            name,
            description: description.to_string(),
            passed: true,
            detail: None,
        },
        Ok(out) => ValidationCheck {
            name,
            description: description.to_string(),
            passed: false,
            detail: Some(format!(
                "`{} --version` exited with status {:?}",
                cmd,
                out.status.code()
            )),
        },
        Err(e) => ValidationCheck {
            name,
            description: description.to_string(),
            passed: false,
            detail: Some(format!("`{}` not found on PATH: {}", cmd, e)),
        },
    }
}

/// Check whether the current working directory is writable. Verisimiser
/// writes manifests, sidecar databases, and generated DDL — a read-only
/// cwd will fail with permission errors at runtime.
fn check_cwd_writable() -> ValidationCheck {
    let cwd_meta = std::env::current_dir().and_then(std::fs::metadata);
    match cwd_meta {
        Ok(md) if !md.permissions().readonly() => ValidationCheck {
            name: "cwd-writable".to_string(),
            description: "Current working directory is writable".to_string(),
            passed: true,
            detail: None,
        },
        Ok(_) => ValidationCheck {
            name: "cwd-writable".to_string(),
            description: "Current working directory is writable".to_string(),
            passed: false,
            detail: Some("cwd is read-only".to_string()),
        },
        Err(e) => ValidationCheck {
            name: "cwd-writable".to_string(),
            description: "Current working directory is writable".to_string(),
            passed: false,
            detail: Some(format!("cannot stat cwd: {}", e)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::run_doctor;

    /// Doctor without a manifest path runs only environment checks.
    #[test]
    fn doctor_without_manifest_runs_env_checks_only() {
        let report = run_doctor(None);
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"path-cargo"));
        assert!(names.contains(&"path-git"));
        assert!(names.contains(&"cwd-writable"));
        // No manifest-* checks present.
        assert!(!names.iter().any(|n| n.starts_with("manifest-")));
    }

    /// Doctor with a manifest path runs env checks AND manifest checks.
    #[test]
    fn doctor_with_manifest_runs_both_sets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.db");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"sqlite\"\n\
             [sidecar]\n\
             storage = \"sqlite\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = run_doctor(Some(path.to_str().unwrap()));
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        // Env checks still present.
        assert!(names.contains(&"path-cargo"));
        // Manifest-loads check appended.
        assert!(names.contains(&"manifest-loads"));
    }
}
