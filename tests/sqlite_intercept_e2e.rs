// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// End-to-end integration test for the SQLite Tier 1 path
// (V-L1-C1 / #46).
//
// Builds a tempfile-backed target + sidecar pair, installs the
// provenance interceptor, replays a mixed workload of inserts /
// updates / deletes across multiple entities, then verifies:
//
//   1. Every entity's provenance chain verifies.
//   2. The target database has only the rows the application wrote
//      — no `verisimdb_*` tables leak in.
//   3. The sidecar holds exactly the right number of entries.
//
// Tempfile-backed so the test exercises the real on-disk path
// (WAL, file locks, separate processes-files) rather than the
// in-memory shortcut used by unit tests.

use rusqlite::{params, Connection};
use std::sync::Arc;
use tempfile::TempDir;
use verisimiser::intercept::sqlite::{EntityIdResolver, SqliteInterceptor};
use verisimiser::tier1::provenance::{init_sidecar_schema, verify_chain};

fn setup() -> (TempDir, Connection, SqliteInterceptor) {
    let tmp = TempDir::new().expect("tempdir");
    let target_path = tmp.path().join("target.db");
    let sidecar_path = tmp.path().join("sidecar.db");

    let target = Connection::open(&target_path).expect("open target");
    target
        .execute_batch(
            "CREATE TABLE accounts (\
                 id INTEGER PRIMARY KEY,\
                 balance INTEGER NOT NULL\
             );",
        )
        .expect("target schema");

    let sidecar = Connection::open(&sidecar_path).expect("open sidecar");
    init_sidecar_schema(&sidecar).expect("sidecar schema");

    // Resolver: route rowid to a logical entity id `accounts:N` so
    // the sidecar entries are human-readable.
    let resolver: EntityIdResolver =
        Arc::new(|table, rowid| format!("{table}:{rowid}"));
    let interceptor = SqliteInterceptor::new(sidecar, "e2e-test")
        .with_resolver(resolver);
    interceptor.install(&target);

    (tmp, target, interceptor)
}

#[test]
fn e2e_mixed_workload_verifies_all_chains() {
    let (_tmp, target, interceptor) = setup();

    // 5 accounts, each: insert, then 3 updates, then delete.
    // Total: 5 * 5 = 25 writes; 5 chains each of length 5.
    const N_ACCOUNTS: usize = 5;
    const UPDATES_PER_ACCOUNT: usize = 3;
    let mut expected_entries = 0;

    for i in 1..=N_ACCOUNTS as i64 {
        target
            .execute(
                "INSERT INTO accounts (id, balance) VALUES (?1, ?2)",
                params![i, 100],
            )
            .unwrap();
        expected_entries += 1;
        for _ in 0..UPDATES_PER_ACCOUNT {
            target
                .execute(
                    "UPDATE accounts SET balance = balance + 10 WHERE id = ?1",
                    params![i],
                )
                .unwrap();
            expected_entries += 1;
        }
        target
            .execute("DELETE FROM accounts WHERE id = ?1", params![i])
            .unwrap();
        expected_entries += 1;
    }

    let sidecar = interceptor.sidecar();
    let conn = sidecar.lock().unwrap();

    // (1) Every chain verifies.
    for i in 1..=N_ACCOUNTS as i64 {
        let entity_id = format!("accounts:{i}");
        assert!(
            verify_chain(&conn, &entity_id).unwrap(),
            "chain for {entity_id} failed verification",
        );
    }

    // (2) Total entry count matches the workload.
    let actual: i64 = conn
        .query_row("SELECT COUNT(*) FROM verisimdb_provenance_log", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(actual, expected_entries as i64);

    // (3) Each chain has the expected length (1 insert + N updates + 1 delete).
    for i in 1..=N_ACCOUNTS as i64 {
        let entity_id = format!("accounts:{i}");
        let chain_len: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_provenance_log WHERE entity_id = ?1",
                [&entity_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            chain_len,
            (UPDATES_PER_ACCOUNT + 2) as i64,
            "wrong chain length for {entity_id}",
        );
    }

    // (4) Target has no surviving rows (all deleted) and no
    // verisimdb_* tables — V-L1-C1 isolation invariant.
    let target_rows: i64 = target
        .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(target_rows, 0);
    let leaked: i64 = target
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'verisimdb_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0, "verisimdb_* tables must not leak into the target");
}

#[test]
fn e2e_chain_survives_reopen_of_sidecar() {
    let (tmp, target, interceptor) = setup();
    let sidecar_path = tmp.path().join("sidecar.db");

    // Drive a small workload.
    target
        .execute(
            "INSERT INTO accounts (id, balance) VALUES (?1, ?2)",
            params![42i64, 1000],
        )
        .unwrap();
    target
        .execute("UPDATE accounts SET balance = 2000 WHERE id = ?1", params![42i64])
        .unwrap();

    // Drop the interceptor (and its sidecar handle); reopen and verify.
    drop(interceptor);
    drop(target);
    let conn = Connection::open(&sidecar_path).expect("reopen sidecar");
    assert!(
        verify_chain(&conn, "accounts:42").unwrap(),
        "chain must verify after sidecar close + reopen"
    );
    let head: String = conn
        .query_row(
            "SELECT head_hash FROM verisimdb_provenance_chain_head WHERE entity_id = ?1",
            ["accounts:42"],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!head.is_empty());
}
