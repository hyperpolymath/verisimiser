// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//
// Emit git-sha + build-date as compile-time env vars so the binary can show
// them in `verisimiser --version` and `verisimiser version --json`.
// Closes #56 (V-L3-J1). No build-dep — uses the `git` CLI and `chrono` is
// available at runtime via the main dependency tree.

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    println!("cargo:rustc-env=VERISIMISER_GIT_SHA={}", sha);
    println!("cargo:rustc-env=VERISIMISER_GIT_DESCRIBE={}", describe);
    println!("cargo:rustc-env=VERISIMISER_BUILD_DATE={}", build_date);

    // Re-run when HEAD moves or git ref changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
