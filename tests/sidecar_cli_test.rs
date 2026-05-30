// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// End-to-end coverage for the `provenance` and `history` CLI subcommands
// over the JSON sidecar backend (V-L2-F4, #150). The store is seeded through
// the library's *locked* write path (real hashes), then the built binary is
// driven against it so the full manifest → resolve → read → print plumbing
// is exercised.

use std::path::Path;
use std::process::Command;

use verisimiser::sidecar::JsonFormat;
use verisimiser::sidecar::json;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verisimiser"))
}

fn write_manifest(dir: &Path, sidecar: &Path, format: &str) -> std::path::PathBuf {
    let toml = format!(
        "[database]\nbackend = \"sqlite\"\n\
         [sidecar]\nstorage = \"json\"\nformat = \"{}\"\npath = \"{}\"\n",
        format,
        sidecar.display().to_string().replace('\\', "/")
    );
    let path = dir.join("verisimiser.toml");
    std::fs::write(&path, toml).unwrap();
    path
}

#[test]
fn provenance_and_history_over_json_ndjson() {
    let dir = tempfile::tempdir().unwrap();
    let sidecar = dir.path().join("sidecar.ndjson");

    // Seed real provenance + temporal data through the locked write path.
    json::with_locked(&sidecar, JsonFormat::Ndjson, |store| {
        store.append_provenance("e1", "users", "insert", "alice", None, None)?;
        store.append_provenance("e1", "users", "update", "alice", Some("{\"n\":1}"), None)?;
        store.append_temporal_version("e1", "users", "{\"n\":0}", "insert");
        store.append_temporal_version("e1", "users", "{\"n\":1}", "update");
        Ok(())
    })
    .unwrap();

    let manifest = write_manifest(dir.path(), &sidecar, "ndjson");
    let manifest = manifest.to_str().unwrap();

    // provenance: lists entries and verifies the chain.
    let out = bin()
        .args(["provenance", "-m", manifest, "e1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "provenance failed: {out:?}");
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("Provenance chain for entity: e1"), "{s}");
    assert!(s.contains("Chain verified: yes"), "chain must verify: {s}");
    assert!(s.contains("insert") && s.contains("update"), "{s}");

    // history: lists both versions, the latest marked current.
    let out = bin()
        .args(["history", "-m", manifest, "e1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "history failed: {out:?}");
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("[users] v1"), "{s}");
    assert!(s.contains("[users] v2"), "{s}");
    assert!(s.contains("current"), "latest version marked current: {s}");
}

#[test]
fn provenance_empty_entity_reports_no_entries_and_verifies_vacuously() {
    let dir = tempfile::tempdir().unwrap();
    let sidecar = dir.path().join("s.json");
    // Materialise an empty store.
    json::with_locked(&sidecar, JsonFormat::Plain, |_store| Ok(())).unwrap();

    let manifest = write_manifest(dir.path(), &sidecar, "plain");
    let out = bin()
        .args(["provenance", "-m", manifest.to_str().unwrap(), "ghost"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("(no entries)"), "{s}");
    assert!(
        s.contains("Chain verified: yes"),
        "empty chain verifies: {s}"
    );
}
