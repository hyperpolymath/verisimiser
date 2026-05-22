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

use crate::abi::ProvenanceEntry;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};

// =========================================================================
// Public re-export: the canonical entry shape
// =========================================================================

/// A single link in the provenance hash chain. Mirrors
/// `abi::ProvenanceEntry` 1:1 — kept here for backward compatibility
/// with code that imported `tier1::provenance::ProvenanceRecord`. New
/// callers should prefer the canonical type in `abi`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub hash: String,
    pub previous_hash: String,
    pub entity_id: String,
    pub operation: String,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
    pub before_snapshot: Option<String>,
    pub transformation: Option<String>,
}

impl ProvenanceRecord {
    /// Backward-compat shim. Computes the canonical hash via
    /// `abi::ProvenanceEntry::compute_hash` rather than the older
    /// string-based form.
    pub fn compute_hash(
        previous_hash: &str,
        entity_id: &str,
        operation: &str,
        actor: &str,
        timestamp: &DateTime<Utc>,
        before_snapshot: Option<&str>,
        transformation: Option<&str>,
    ) -> String {
        ProvenanceEntry::compute_hash(
            previous_hash,
            entity_id,
            operation,
            actor,
            timestamp,
            before_snapshot,
            transformation,
        )
    }

    /// Verify that this record's stored hash matches a fresh recompute.
    pub fn verify(&self) -> bool {
        let expected = Self::compute_hash(
            &self.previous_hash,
            &self.entity_id,
            &self.operation,
            &self.actor,
            &self.timestamp,
            self.before_snapshot.as_deref(),
            self.transformation.as_deref(),
        );
        self.hash == expected
    }
}

// =========================================================================
// SQLite sidecar schema
// =========================================================================

/// SQL DDL for the provenance sidecar schema.
///
/// Two tables:
///
/// * `verisimdb_provenance_log` — append-only log of every entry.
///   Mirrors `codegen/overlay.rs::generate_provenance_table` (kept in
///   sync — see ADR-0008 dialect-split work, #45).
/// * `verisimdb_provenance_chain_head` — per-entity pointer to the
///   tip of its chain, used by `append_provenance` to look up the
///   `previous_hash` in O(1) without scanning the log.
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
    CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_head (\
        entity_id TEXT PRIMARY KEY,\
        head_hash TEXT NOT NULL\
    );";

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
) -> rusqlite::Result<String> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let prev_hash: String = tx
        .query_row(
            "SELECT head_hash FROM verisimdb_provenance_chain_head WHERE entity_id = ?1",
            [entity_id],
            |row| row.get(0),
        )
        .unwrap_or_default();

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

    tx.execute(
        "INSERT INTO verisimdb_provenance_log \
         (hash, previous_hash, entity_id, table_name, operation, actor, timestamp, \
          before_snapshot, transformation) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            hash,
            prev_hash,
            entity_id,
            table_name,
            operation,
            actor,
            timestamp.to_rfc3339(),
            before_snapshot,
            transformation,
        ],
    )?;

    tx.execute(
        "INSERT OR REPLACE INTO verisimdb_provenance_chain_head (entity_id, head_hash) \
         VALUES (?1, ?2)",
        params![entity_id, hash],
    )?;

    tx.commit()?;
    Ok(hash)
}

/// Verify that the chain for `entity_id` is internally consistent.
///
/// Walks the log in timestamp order; for each entry, recomputes the
/// hash from its stored fields and checks (a) the recomputed hash
/// matches the stored hash, (b) the `previous_hash` field matches the
/// hash of the prior entry in the walk (or `""` for genesis).
///
/// Returns `Ok(true)` iff the entire chain verifies; `Ok(false)` on the
/// first mismatch. Any SQL error propagates.
pub fn verify_chain(conn: &Connection, entity_id: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT hash, previous_hash, entity_id, operation, actor, timestamp, \
                before_snapshot, transformation \
         FROM verisimdb_provenance_log \
         WHERE entity_id = ?1 \
         ORDER BY timestamp ASC, hash ASC",
    )?;

    let rows = stmt.query_map([entity_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    })?;

    let mut expected_prev = String::new();
    for row in rows {
        let (
            stored_hash,
            stored_prev,
            entity_id,
            operation,
            actor,
            ts_str,
            before_snapshot,
            transformation,
        ) = row?;

        if stored_prev != expected_prev {
            return Ok(false);
        }

        let timestamp = match DateTime::parse_from_rfc3339(&ts_str) {
            Ok(t) => t.with_timezone(&Utc),
            Err(_) => return Ok(false),
        };

        let recomputed = ProvenanceEntry::compute_hash(
            &stored_prev,
            &entity_id,
            &operation,
            &actor,
            &timestamp,
            before_snapshot.as_deref(),
            transformation.as_deref(),
        );

        if recomputed != stored_hash {
            return Ok(false);
        }

        expected_prev = stored_hash;
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
        assert_eq!(count, 2, "expected 2 provenance tables");
    }

    #[test]
    fn genesis_entry_chains_from_empty() {
        let mut conn = open_sidecar();
        let hash = append_provenance(
            &mut conn,
            "e1",
            "users",
            "insert",
            "alice",
            None,
            None,
        )
        .unwrap();
        assert!(!hash.is_empty());

        let prev: String = conn
            .query_row(
                "SELECT previous_hash FROM verisimdb_provenance_log WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(prev, "", "genesis must chain from empty previous_hash");

        let head: String = conn
            .query_row(
                "SELECT head_hash FROM verisimdb_provenance_chain_head WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(head, hash, "chain head must point at the new entry");
    }

    #[test]
    fn sequential_appends_chain_correctly() {
        let mut conn = open_sidecar();
        let h1 = append_provenance(
            &mut conn, "e1", "users", "insert", "alice", None, None,
        )
        .unwrap();
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
        let h3 = append_provenance(
            &mut conn, "e1", "users", "delete", "bob", None, None,
        )
        .unwrap();
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);

        let head: String = conn
            .query_row(
                "SELECT head_hash FROM verisimdb_provenance_chain_head WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(head, h3);

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
