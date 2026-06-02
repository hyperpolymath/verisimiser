// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Provenance tracking via SHA-256 hash chains, persisted to a SQLite
// sidecar. Write-path observer: records what happened, never changes
// what happened.
//
// This module is the SQLite-backed implementation of the Provenance
// concern (per ADR-0004 octad). Other backends (file-based, in-memory,
// remote VeriSimDB instance) would parallel this surface.
//
// V-L1-C1 (#46): `append_provenance` + `verify_chain` + sidecar schema
// management. The canonical hash is computed by
// `abi::ProvenanceEntry::compute_hash` (domain-tagged + length-prefixed
// — see ADR-0002 / #27); this module just persists the entries.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, TransactionBehavior, params};

// =========================================================================
// Canonical entry shape
// =========================================================================

// The provenance entry type is defined once, in `crate::abi`. It is the
// canonical representation used across the Rust CLI, the Idris2 ABI
// proofs, and the Zig FFI bridge, and it is the type persisted at the
// SQLite boundary by `append_provenance` below.
//
// This module previously carried a byte-for-byte duplicate struct
// (same fields, its own `compute_hash`/`verify`) under a different
// name. It was orphaned — nothing constructed it — and a second copy of
// the hash function is an integrity hazard: a future change to one
// `compute_hash` would silently leave the other broken (#26). The
// duplicate has been deleted; the canonical type is re-exported here so
// `tier1::provenance::ProvenanceEntry` resolves to the one definition.
pub use crate::abi::ProvenanceEntry;

// =========================================================================
// SQLite sidecar schema
// =========================================================================

/// SQL DDL for the provenance sidecar schema (ADR-0010: provenance forks
/// are first-class).
///
/// * `verisimdb_provenance_log` — append-only log of every entry. The
///   `hash` PRIMARY KEY *is* the duplicate guard: the preimage is
///   domain-tagged and covers every tamper-relevant field (ADR-0002 /
///   #27), so an exact-duplicate row necessarily collides on `hash`.
///   We deliberately do **not** add `UNIQUE(entity_id, previous_hash)`
///   (#32, superseded by ADR-0010): that would reject a *divergent*
///   second writer's legitimate history at insert time, making a real
///   fork impossible to record, detect or audit.
/// * `idx_provenance_predecessor` — **non-unique** index making fork
///   *detection* O(log n): two children of one predecessor are two
///   rows sharing `(entity_id, previous_hash)` with distinct `hash`.
/// * `verisimdb_provenance_chain_heads` — the set of live branch tips
///   per entity. One row per entity for a linear chain; several rows
///   when the entity has legitimately forked.
/// * `verisimdb_provenance_chain_head` — the legacy single-head table,
///   kept (non-destructively) one release for migration. New writes go
///   to `_chain_heads`; the `INSERT … SELECT` below copies any legacy
///   heads forward idempotently (no-op on a fresh sidecar).
///
/// Mirrors `codegen/overlay.rs::generate_provenance_table` (kept in
/// sync — see ADR-0008 dialect-split work, #45).
pub const SIDECAR_DDL: &str = "\
    CREATE TABLE IF NOT EXISTS verisimdb_provenance_log (\
        hash          TEXT PRIMARY KEY,\
        previous_hash TEXT NOT NULL,\
        entity_id     TEXT NOT NULL,\
        table_name    TEXT NOT NULL,\
        operation     TEXT NOT NULL,\
        actor         TEXT NOT NULL,\
        timestamp     TEXT NOT NULL,\
        before_snapshot TEXT,\
        transformation  TEXT,\
        CHECK (operation IN ('insert','update','delete','transform'))\
    );\
    CREATE INDEX IF NOT EXISTS idx_provenance_entity ON verisimdb_provenance_log(entity_id);\
    CREATE INDEX IF NOT EXISTS idx_provenance_table  ON verisimdb_provenance_log(table_name);\
    CREATE INDEX IF NOT EXISTS idx_provenance_predecessor \
        ON verisimdb_provenance_log(entity_id, previous_hash);\
    CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_head (\
        entity_id TEXT PRIMARY KEY,\
        head_hash TEXT NOT NULL\
    );\
    CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_heads (\
        entity_id TEXT NOT NULL,\
        head_hash TEXT NOT NULL,\
        PRIMARY KEY (entity_id, head_hash)\
    );\
    INSERT OR IGNORE INTO verisimdb_provenance_chain_heads (entity_id, head_hash) \
        SELECT entity_id, head_hash FROM verisimdb_provenance_chain_head;";

/// Create the provenance sidecar tables in `conn` if they don't already
/// exist. Idempotent — safe to call on every open of an existing
/// sidecar.
pub fn init_sidecar_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SIDECAR_DDL)
}

// =========================================================================
// Append + verify
// =========================================================================

/// Append a new provenance entry for `entity_id` to the sidecar.
///
/// V-L1-C1 (#46). Wraps the read-chain-head + insert-log + update-head
/// triple in a single `BEGIN IMMEDIATE` transaction so the chain stays
/// strictly serial even under concurrent writers (SQLite will queue
/// concurrent IMMEDIATE transactions). Returns the freshly-computed
/// hash for the caller's records.
///
/// The `table_name` field is passed explicitly rather than inferred
/// from the connection because the typical caller is an
/// `sqlite3_update_hook` whose callback already knows the table.
pub fn append_provenance(
    conn: &mut Connection,
    entity_id: &str,
    table_name: &str,
    operation: &str,
    actor: &str,
    before_snapshot: Option<&str>,
    transformation: Option<&str>,
) -> anyhow::Result<String> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    // Linear fast-path: the parent is the entity's *unique* current
    // head. Zero heads ⇒ genesis (prev = ""). More than one head ⇒ the
    // entity has legitimately forked and there is no single tip to
    // extend; the caller must choose a branch via `append_provenance_fork`
    // (ADR-0010 §2).
    let heads = head_set(&tx, entity_id)?;
    let prev_hash: String = match heads.len() {
        0 => String::new(),
        1 => heads[0].clone(),
        n => {
            return Err(anyhow::anyhow!(format!(
                "entity {entity_id:?} has {n} chain heads (forked); linear \
                 append is ambiguous — use append_provenance_fork(from_hash) \
                 to extend a specific branch (ADR-0010)"
            )));
        }
    };

    let timestamp = Utc::now();
    let hash = ProvenanceEntry::compute_hash(
        &prev_hash,
        entity_id,
        operation,
        actor,
        &timestamp,
        before_snapshot,
        transformation,
    );

    insert_log_row(
        &tx,
        &hash,
        &prev_hash,
        entity_id,
        table_name,
        operation,
        actor,
        &timestamp,
        before_snapshot,
        transformation,
    )?;

    // Linear advance: drop the consumed parent tip, add the new tip, so
    // a normal append keeps exactly one head.
    if !prev_hash.is_empty() {
        tx.execute(
            "DELETE FROM verisimdb_provenance_chain_heads \
             WHERE entity_id = ?1 AND head_hash = ?2",
            params![entity_id, prev_hash],
        )?;
    }
    add_head(&tx, entity_id, &hash)?;

    tx.commit()?;
    Ok(hash)
}

/// Extend the chain of `entity_id` from a *specific ancestor* `from_hash`
/// rather than the current tip — i.e. deliberately record a fork (ADR-0010
/// §2). This is honest divergent history (partitioned/replicated/offline
/// writers, simulation branches), not tampering.
///
/// Unlike [`append_provenance`], this *adds* a head without removing one:
/// the entity gains a new branch tip and now has ≥2 heads. `from_hash`
/// must be an existing entry in this entity's log.
#[allow(clippy::too_many_arguments)]
pub fn append_provenance_fork(
    conn: &mut Connection,
    entity_id: &str,
    table_name: &str,
    operation: &str,
    actor: &str,
    before_snapshot: Option<&str>,
    transformation: Option<&str>,
    from_hash: &str,
) -> anyhow::Result<String> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let ancestor_exists: bool = tx
        .query_row(
            "SELECT 1 FROM verisimdb_provenance_log \
             WHERE entity_id = ?1 AND hash = ?2",
            params![entity_id, from_hash],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !ancestor_exists {
        return Err(anyhow::anyhow!(format!(
            "from_hash {from_hash:?} is not an entry in entity {entity_id:?}'s \
             chain; cannot fork from a non-existent ancestor"
        )));
    }

    let timestamp = Utc::now();
    let hash = ProvenanceEntry::compute_hash(
        from_hash,
        entity_id,
        operation,
        actor,
        &timestamp,
        before_snapshot,
        transformation,
    );

    insert_log_row(
        &tx,
        &hash,
        from_hash,
        entity_id,
        table_name,
        operation,
        actor,
        &timestamp,
        before_snapshot,
        transformation,
    )?;

    // A fork *adds* a tip and removes none: the entity now has ≥2 heads.
    add_head(&tx, entity_id, &hash)?;

    tx.commit()?;
    Ok(hash)
}

/// A predecessor in `entity_id`'s log that has more than one child —
/// i.e. the point at which the history diverged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkPoint {
    /// Hash of the shared predecessor entry (or `""` for a forked genesis).
    pub predecessor: String,
    /// How many distinct children chain directly from it (always ≥ 2).
    pub children: u64,
}

/// Every fork point in `entity_id`'s history. Empty ⇒ the chain is
/// linear. O(log n) via `idx_provenance_predecessor` (ADR-0010 §1/§3).
pub fn fork_points(conn: &Connection, entity_id: &str) -> rusqlite::Result<Vec<ForkPoint>> {
    let mut stmt = conn.prepare(
        "SELECT previous_hash, COUNT(*) AS c \
         FROM verisimdb_provenance_log \
         WHERE entity_id = ?1 \
         GROUP BY previous_hash HAVING c > 1 \
         ORDER BY previous_hash",
    )?;
    let rows = stmt.query_map([entity_id], |row| {
        Ok(ForkPoint {
            predecessor: row.get::<_, String>(0)?,
            children: row.get::<_, i64>(1)? as u64,
        })
    })?;
    rows.collect()
}

// --- internal helpers -----------------------------------------------------

/// The current set of branch-tip hashes for `entity_id`.
fn head_set(conn: &Connection, entity_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT head_hash FROM verisimdb_provenance_chain_heads WHERE entity_id = ?1")?;
    let rows = stmt.query_map([entity_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Add `hash` to the entity's head set (idempotent). Also best-effort
/// updates the legacy single-head table so a one-release-old reader
/// still sees *a* head (it cannot represent the fork, but stays valid).
fn add_head(conn: &Connection, entity_id: &str, hash: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO verisimdb_provenance_chain_heads (entity_id, head_hash) \
         VALUES (?1, ?2)",
        params![entity_id, hash],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO verisimdb_provenance_chain_head (entity_id, head_hash) \
         VALUES (?1, ?2)",
        params![entity_id, hash],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_log_row(
    conn: &Connection,
    hash: &str,
    previous_hash: &str,
    entity_id: &str,
    table_name: &str,
    operation: &str,
    actor: &str,
    timestamp: &DateTime<Utc>,
    before_snapshot: Option<&str>,
    transformation: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO verisimdb_provenance_log \
         (hash, previous_hash, entity_id, table_name, operation, actor, timestamp, \
          before_snapshot, transformation) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            hash,
            previous_hash,
            entity_id,
            table_name,
            operation,
            actor,
            timestamp.to_rfc3339(),
            before_snapshot,
            transformation,
        ],
    )?;
    Ok(())
}

/// Verify that every branch of `entity_id`'s chain is internally
/// hash-consistent (ADR-0010 §3).
///
/// A forked entity is **not** a tampered one: linearity is not the
/// integrity property — tamper-evidence and no-silent-loss are. So
/// rather than assume a single linear walk, this builds the entity's
/// `hash → entry` map, identifies every branch tip (a hash that is no
/// row's `previous_hash`, unioned with the recorded head set), and
/// walks each tip back to a genesis (`previous_hash == ""`). Each node
/// on every branch must (a) recompute to its stored `hash`, and (b)
/// chain to a present predecessor (or genesis). Shared prefixes are
/// re-checked; correctness over micro-optimisation.
///
/// Returns `Ok(true)` iff *all* branches verify; `Ok(false)` on the
/// first inconsistency. An empty entity verifies vacuously.
pub fn verify_chain(conn: &Connection, entity_id: &str) -> rusqlite::Result<bool> {
    use std::collections::{HashMap, HashSet};

    struct Node {
        previous_hash: String,
        operation: String,
        actor: String,
        ts_str: String,
        before_snapshot: Option<String>,
        transformation: Option<String>,
    }

    let mut stmt = conn.prepare(
        "SELECT hash, previous_hash, operation, actor, timestamp, \
                before_snapshot, transformation \
         FROM verisimdb_provenance_log WHERE entity_id = ?1",
    )?;
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let mut has_child: HashSet<String> = HashSet::new();
    let iter = stmt.query_map([entity_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Node {
                previous_hash: row.get::<_, String>(1)?,
                operation: row.get::<_, String>(2)?,
                actor: row.get::<_, String>(3)?,
                ts_str: row.get::<_, String>(4)?,
                before_snapshot: row.get::<_, Option<String>>(5)?,
                transformation: row.get::<_, Option<String>>(6)?,
            },
        ))
    })?;
    for r in iter {
        let (hash, node) = r?;
        if !node.previous_hash.is_empty() {
            has_child.insert(node.previous_hash.clone());
        }
        nodes.insert(hash, node);
    }
    if nodes.is_empty() {
        return Ok(true); // vacuous
    }

    // Tips = recorded heads ∪ any hash nothing chains from.
    let mut tips: HashSet<String> = head_set(conn, entity_id)?.into_iter().collect();
    for h in nodes.keys() {
        if !has_child.contains(h) {
            tips.insert(h.clone());
        }
    }

    for tip in tips {
        let mut cursor = tip;
        loop {
            let Some(node) = nodes.get(&cursor) else {
                // A tip recorded in the head set with no log row, or a
                // dangling previous_hash: broken chain.
                return Ok(false);
            };
            let timestamp = match DateTime::parse_from_rfc3339(&node.ts_str) {
                Ok(t) => t.with_timezone(&Utc),
                Err(_) => return Ok(false),
            };
            let recomputed = ProvenanceEntry::compute_hash(
                &node.previous_hash,
                entity_id,
                &node.operation,
                &node.actor,
                &timestamp,
                node.before_snapshot.as_deref(),
                node.transformation.as_deref(),
            );
            if recomputed != cursor {
                return Ok(false);
            }
            if node.previous_hash.is_empty() {
                break; // reached genesis: this branch is consistent
            }
            cursor = node.previous_hash.clone();
        }
    }

    Ok(true)
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn open_sidecar() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_sidecar_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_sidecar_schema(&conn).unwrap();
        init_sidecar_schema(&conn).unwrap(); // re-running must not error
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name LIKE 'verisimdb_provenance%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // log + legacy single-head + multi-head set (ADR-0010 keeps the
        // legacy table one release for non-destructive migration).
        assert_eq!(count, 3, "expected 3 provenance tables");
    }

    #[test]
    fn genesis_entry_chains_from_empty() {
        let mut conn = open_sidecar();
        let hash =
            append_provenance(&mut conn, "e1", "users", "insert", "alice", None, None).unwrap();
        assert!(!hash.is_empty());

        let prev: String = conn
            .query_row(
                "SELECT previous_hash FROM verisimdb_provenance_log WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(prev, "", "genesis must chain from empty previous_hash");

        let heads: Vec<String> = {
            let mut s = conn
                .prepare(
                    "SELECT head_hash FROM verisimdb_provenance_chain_heads WHERE entity_id='e1'",
                )
                .unwrap();
            let r = s.query_map([], |x| x.get::<_, String>(0)).unwrap();
            r.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(heads, vec![hash], "genesis must record exactly one head");
    }

    #[test]
    fn sequential_appends_chain_correctly() {
        let mut conn = open_sidecar();
        let h1 =
            append_provenance(&mut conn, "e1", "users", "insert", "alice", None, None).unwrap();
        let h2 = append_provenance(
            &mut conn,
            "e1",
            "users",
            "update",
            "alice",
            Some("{\"name\":\"Alice\"}"),
            None,
        )
        .unwrap();
        let h3 = append_provenance(&mut conn, "e1", "users", "delete", "bob", None, None).unwrap();
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);

        let heads: Vec<String> = {
            let mut s = conn
                .prepare(
                    "SELECT head_hash FROM verisimdb_provenance_chain_heads WHERE entity_id='e1'",
                )
                .unwrap();
            let r = s.query_map([], |x| x.get::<_, String>(0)).unwrap();
            r.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(
            heads,
            vec![h3.clone()],
            "a linear chain advances its single head, never accumulates tips"
        );

        assert!(
            verify_chain(&conn, "e1").unwrap(),
            "fresh three-entry chain must verify"
        );
    }

    #[test]
    fn verify_chain_detects_tampered_hash() {
        let mut conn = open_sidecar();
        append_provenance(&mut conn, "e1", "users", "insert", "alice", None, None).unwrap();
        append_provenance(&mut conn, "e1", "users", "update", "alice", None, None).unwrap();

        // Tamper with one entry's operation field after the fact —
        // the stored hash no longer matches the recompute.
        conn.execute(
            "UPDATE verisimdb_provenance_log SET operation='transform' WHERE entity_id='e1' \
             AND operation='update'",
            [],
        )
        .unwrap();

        assert!(
            !verify_chain(&conn, "e1").unwrap(),
            "tampered entry must fail verification"
        );
    }

    #[test]
    fn verify_chain_detects_broken_chain_link() {
        let mut conn = open_sidecar();
        append_provenance(&mut conn, "e1", "users", "insert", "alice", None, None).unwrap();
        append_provenance(&mut conn, "e1", "users", "update", "alice", None, None).unwrap();

        // Splice a fake previous_hash into the second entry.
        conn.execute(
            "UPDATE verisimdb_provenance_log SET previous_hash='deadbeef' \
             WHERE entity_id='e1' AND operation='update'",
            [],
        )
        .unwrap();

        assert!(
            !verify_chain(&conn, "e1").unwrap(),
            "broken chain link must fail verification"
        );
    }

    #[test]
    fn distinct_entities_have_independent_chains() {
        let mut conn = open_sidecar();
        append_provenance(&mut conn, "e1", "users", "insert", "alice", None, None).unwrap();
        append_provenance(&mut conn, "e2", "users", "insert", "bob", None, None).unwrap();
        append_provenance(&mut conn, "e1", "users", "update", "alice", None, None).unwrap();
        append_provenance(&mut conn, "e2", "users", "update", "bob", None, None).unwrap();

        assert!(verify_chain(&conn, "e1").unwrap());
        assert!(verify_chain(&conn, "e2").unwrap());

        let e1_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_provenance_log WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let e2_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_provenance_log WHERE entity_id='e2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(e1_count, 2);
        assert_eq!(e2_count, 2);
    }
}
