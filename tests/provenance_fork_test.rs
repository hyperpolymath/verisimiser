// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// ADR-0010 (provenance forks are first-class) acceptance suite for
// #31 (and #32, superseded). Provenance forks — two valid children of
// the same predecessor (network-partitioned honest writers, replicas,
// simulation branches) — must be representable, persisted, detectable,
// and verifiable. The integrity property is tamper-evidence and
// no-silent-loss, NOT linearity.
//
// These tests were failing-by-design on `main`; the #31 implementation
// (multi-head tip set + fork-aware append + fork_points + per-branch
// verify) makes them pass.

use rusqlite::{Connection, params};
use verisimiser::abi::ProvenanceEntry;
use verisimiser::tier1::provenance::{
    append_provenance, append_provenance_fork, fork_points, init_sidecar_schema, verify_chain,
};

fn open_sidecar() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory sidecar");
    init_sidecar_schema(&conn).expect("init sidecar schema");
    conn
}

/// Rows in the multi-head tip set for `entity_id`.
fn head_count(conn: &Connection, entity_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM verisimdb_provenance_chain_heads WHERE entity_id = ?1",
        [entity_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Children of `parent` (rows whose `previous_hash = parent`). > 1 ⇒ fork.
fn child_count(conn: &Connection, entity_id: &str, parent: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM verisimdb_provenance_log \
         WHERE entity_id = ?1 AND previous_hash = ?2",
        params![entity_id, parent],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn log_count(conn: &Connection, entity_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM verisimdb_provenance_log WHERE entity_id = ?1",
        [entity_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[test]
fn fork_can_be_written_and_both_branches_persist() {
    let mut conn = open_sidecar();
    let entity = "account:42";

    // Genesis, then a normal linear child (branch A) off it.
    let genesis = append_provenance(&mut conn, entity, "accounts", "insert", "alice", None, None)
        .expect("genesis append");
    let _branch_a = append_provenance(&mut conn, entity, "accounts", "update", "alice", None, None)
        .expect("branch A append (linear, off genesis)");

    // A second, partitioned-but-honest writer extends the chain from
    // the SAME genesis tip — a legitimate fork via the explicit API.
    let branch_b = append_provenance_fork(
        &mut conn, entity, "accounts", "update", "bob", None, None, &genesis,
    )
    .expect("branch B append (fork from genesis)");

    // Genesis must have two children — the fork is representable, not
    // silently collapsed.
    assert_eq!(
        child_count(&conn, entity, &genesis),
        2,
        "genesis must have two children (branch A + branch B)"
    );
    // Three log rows total: genesis, A, B.
    assert_eq!(log_count(&conn, entity), 3, "all three entries persist");
    // Two live branch tips, both tracked (linear A-tip + fork B-tip).
    assert_eq!(
        head_count(&conn, entity),
        2,
        "a forked entity records one head per branch"
    );
    assert!(!branch_b.is_empty());
}

#[test]
fn fork_points_detects_the_divergence() {
    let mut conn = open_sidecar();
    let entity = "doc:7";

    let genesis = append_provenance(&mut conn, entity, "docs", "insert", "a", None, None).unwrap();
    let _a = append_provenance(&mut conn, entity, "docs", "update", "a", None, None).unwrap();
    let _b = append_provenance_fork(
        &mut conn, entity, "docs", "update", "b", None, None, &genesis,
    )
    .unwrap();

    let forks = fork_points(&conn, entity).expect("fork_points query");
    assert_eq!(forks.len(), 1, "exactly one divergence point");
    assert_eq!(forks[0].predecessor, genesis, "the fork is at genesis");
    assert_eq!(forks[0].children, 2, "genesis has two children");

    // A purely linear entity has no fork points.
    let mut c2 = open_sidecar();
    append_provenance(&mut c2, "lin:1", "t", "insert", "a", None, None).unwrap();
    append_provenance(&mut c2, "lin:1", "t", "update", "a", None, None).unwrap();
    assert!(fork_points(&c2, "lin:1").unwrap().is_empty());
}

#[test]
fn each_branch_verifies_independently() {
    let mut conn = open_sidecar();
    let entity = "sim:1";

    let genesis = append_provenance(&mut conn, entity, "t", "insert", "a", None, None).unwrap();
    append_provenance(&mut conn, entity, "t", "update", "a", None, None).unwrap();
    append_provenance_fork(
        &mut conn,
        entity,
        "t",
        "transform",
        "b",
        None,
        None,
        &genesis,
    )
    .unwrap();

    // Divergence is not tampering: every branch is hash-consistent, so
    // the forked entity must still verify true.
    assert!(
        verify_chain(&conn, entity).expect("verify forked entity"),
        "a forked-but-honest history must verify (each branch consistent)"
    );

    // Tampering one branch's row must still be caught.
    conn.execute(
        "UPDATE verisimdb_provenance_log SET actor = 'mallory' \
         WHERE entity_id = ?1 AND actor = 'b'",
        [entity],
    )
    .unwrap();
    assert!(
        !verify_chain(&conn, entity).unwrap(),
        "tampering a forked branch must still fail verification"
    );
}

#[test]
fn exact_duplicate_entry_is_rejected() {
    let conn = open_sidecar();
    let entity = "dup:1";
    let ts = chrono::Utc::now();
    let hash = ProvenanceEntry::compute_hash("", entity, "insert", "a", &ts, None, None);

    let insert = |h: &str| {
        conn.execute(
            "INSERT INTO verisimdb_provenance_log \
             (hash, previous_hash, entity_id, table_name, operation, actor, \
              timestamp, before_snapshot, transformation) \
             VALUES (?1, '', ?2, 't', 'insert', 'a', ?3, NULL, NULL)",
            params![h, entity, ts.to_rfc3339()],
        )
    };

    insert(&hash).expect("first insert of a unique entry succeeds");
    // A byte-identical entry has the same domain-tagged hash and so
    // collides on the `hash` PRIMARY KEY — the correct duplicate guard
    // the superseded UNIQUE index was trying (wrongly) to provide.
    assert!(
        insert(&hash).is_err(),
        "an exact-duplicate entry must be rejected by the hash PK"
    );
}
