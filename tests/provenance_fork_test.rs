// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//
// FAILING-BY-DESIGN test for the fork-impossibility defect
// (#31 + #32, see docs/decisions/0010-provenance-forks-are-first-class.adoc).
//
// This test encodes the *desired* behaviour: a legitimate provenance
// fork (two valid children of the same predecessor — e.g. two
// network-partitioned honest writers, or a simulation branch) must be
// representable, persisted, and detectable.
//
// It is EXPECTED TO FAIL on `main` today, because:
//   * `verisimdb_provenance_chain_head` has `entity_id` as PRIMARY KEY,
//     so an entity can only ever record ONE head — the second branch's
//     head is silently overwritten (INSERT OR REPLACE).
//   * there is no fork-aware append / detection surface.
//   * if #32's `UNIQUE INDEX(entity_id, previous_hash)` were applied,
//     the second child insert would additionally fail with a
//     constraint violation.
//
// The implementing PR for #31/#32 makes this test pass (multi-head
// table + fork-aware append + `fork_points`). Until then it documents
// the defect in executable form.
//
// It compiles against the *current* public surface so CI exercises it
// rather than ignoring it; the assertions — not the compile — are what
// fail.

use rusqlite::{params, Connection};
use verisimiser::abi::ProvenanceEntry;
use verisimiser::tier1::provenance::{append_provenance, init_sidecar_schema};

fn open_sidecar() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory sidecar");
    init_sidecar_schema(&conn).expect("init sidecar schema");
    conn
}

/// Count chain heads recorded for an entity. Today this can only ever
/// be 0 or 1 because `entity_id` is the PRIMARY KEY of the head table;
/// the target design records one row per live branch tip.
fn head_count(conn: &Connection, entity_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM verisimdb_provenance_chain_head WHERE entity_id = ?1",
        [entity_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Number of rows in the log whose `previous_hash` is `parent` — i.e.
/// how many children that node has. > 1 ==> a fork at `parent`.
fn child_count(conn: &Connection, entity_id: &str, parent: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM verisimdb_provenance_log \
         WHERE entity_id = ?1 AND previous_hash = ?2",
        params![entity_id, parent],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[test]
fn fork_can_be_written_and_both_branches_persist() {
    let mut conn = open_sidecar();
    let entity = "account:42";

    // Genesis + one normal child via the supported linear path.
    let genesis = append_provenance(
        &mut conn, entity, "accounts", "insert", "alice", None, None,
    )
    .expect("genesis append");
    let _branch_a = append_provenance(
        &mut conn, entity, "accounts", "update", "alice", None, None,
    )
    .expect("branch A append");

    // A second, legitimate writer (partitioned from the first) extends
    // the chain from the SAME genesis tip: a fork. There is no
    // supported API for "chain from this specific ancestor" yet, so we
    // construct the entry the way the target `append_provenance_fork`
    // will and write it directly. The hash is canonical and the row is
    // internally valid — it is honest history, not tampering.
    let ts = chrono::Utc::now();
    let branch_b_hash = ProvenanceEntry::compute_hash(
        &genesis, entity, "update", "bob", &ts, None, None,
    );
    conn.execute(
        "INSERT INTO verisimdb_provenance_log \
         (hash, previous_hash, entity_id, table_name, operation, actor, \
          timestamp, before_snapshot, transformation) \
         VALUES (?1, ?2, ?3, 'accounts', 'update', 'bob', ?4, NULL, NULL)",
        params![branch_b_hash, genesis, entity, ts.to_rfc3339()],
    )
    .expect("fork row insert (fails here once #32 unique index is added)");

    // The target design also records branch B's head. Today the head
    // table cannot hold two heads for one entity (entity_id is PK), so
    // we attempt the insert the implementing PR will do.
    let _ = conn.execute(
        "INSERT INTO verisimdb_provenance_chain_head (entity_id, head_hash) \
         VALUES (?1, ?2)",
        params![entity, branch_b_hash],
    );

    // --- Desired-behaviour assertions (expected to FAIL on main) ---

    // Both children of genesis must be retained: this is a true fork.
    assert_eq!(
        child_count(&conn, entity, &genesis),
        2,
        "genesis must have two children (branch A + branch B) — the \
         fork must be representable, not silently collapsed",
    );

    // The entity now has two live branch tips; both must be tracked.
    assert_eq!(
        head_count(&conn, entity),
        2,
        "a forked entity must record one head per branch; today the \
         single-row-per-entity head table cannot express this (#31)",
    );
}
