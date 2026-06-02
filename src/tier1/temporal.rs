// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Temporal versioning sidecar.
//
// Records every state change for point-in-time queries and rollback.
// Companion to `tier1::provenance` — the same target-write event
// produces a provenance entry (who/what/when) AND a temporal version
// entry (the row's full state). Each lives in its own table inside the
// shared sidecar database.
//
// V-L1-C2 (#47): `append_version` + `read_at` + `rollback_to`, plus
// the partial UNIQUE INDEX (from V-L2-H1 / #41) that enforces "at most
// one current version per (entity_id, table_name)" at the storage
// layer so two concurrent writers can't both leave a `valid_to IS
// NULL` row hanging around.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

// =========================================================================
// Public types
// =========================================================================

/// A versioned snapshot of an entity at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVersion {
    pub entity_id: String,
    pub version: u64,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub snapshot: serde_json::Value,
    pub operation: String,
}

// =========================================================================
// SQLite sidecar schema
// =========================================================================

/// SQL DDL for the temporal sidecar schema. Mirrors
/// `codegen/overlay.rs::generate_temporal_table` (kept in sync — see
/// V-L2-F1 / #45 dialect-split work).
pub const SIDECAR_DDL: &str = "\
    CREATE TABLE IF NOT EXISTS verisimdb_temporal_versions (\
        entity_id  TEXT NOT NULL,\
        table_name TEXT NOT NULL,\
        version    INTEGER NOT NULL,\
        valid_from TEXT NOT NULL,\
        valid_to   TEXT,\
        snapshot   TEXT NOT NULL,\
        operation  TEXT NOT NULL,\
        PRIMARY KEY (entity_id, table_name, version),\
        CHECK (valid_to IS NULL OR valid_to >= valid_from)\
    );\
    CREATE UNIQUE INDEX IF NOT EXISTS idx_temporal_current \
        ON verisimdb_temporal_versions(entity_id, table_name) \
        WHERE valid_to IS NULL;";

/// Create the temporal sidecar table + partial-unique index in `conn`
/// if they don't already exist. Idempotent — safe to call on every
/// open of an existing sidecar.
pub fn init_sidecar_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SIDECAR_DDL)
}

// =========================================================================
// Append a new version
// =========================================================================

/// Append a new version of `(entity_id, table_name)` to the temporal
/// log.
///
/// Three-step transactional update:
///
/// 1. Read `MAX(version)` for the entity/table — next version is
///    `prev + 1` (or `1` for genesis).
/// 2. Close out the previous current row by setting its `valid_to` to
///    `now`. The partial UNIQUE INDEX on `(entity_id, table_name)
///    WHERE valid_to IS NULL` makes step 2 mandatory — without it,
///    step 3's insert would violate the index.
/// 3. Insert the new row with `valid_to = NULL`.
///
/// Wrapped in `BEGIN IMMEDIATE` so concurrent writers serialise
/// through SQLite's write lock and the version sequence stays
/// monotonic.
///
/// `snapshot` is opaque to this module — pass whatever serialised
/// representation the caller uses (typical: JSON of the row's full
/// state at the moment of the write).
pub fn append_version(
    conn: &mut Connection,
    entity_id: &str,
    table_name: &str,
    snapshot: &str,
    operation: &str,
) -> rusqlite::Result<u64> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let prev_version: i64 = tx.query_row(
        "SELECT COALESCE(MAX(version), 0) \
             FROM verisimdb_temporal_versions \
             WHERE entity_id = ?1 AND table_name = ?2",
        params![entity_id, table_name],
        |row| row.get(0),
    )?;
    let next_version = prev_version + 1;

    let now = Utc::now();
    let now_str = now.to_rfc3339();

    // Close out the previous current row, if any.
    tx.execute(
        "UPDATE verisimdb_temporal_versions \
         SET valid_to = ?1 \
         WHERE entity_id = ?2 AND table_name = ?3 AND valid_to IS NULL",
        params![now_str, entity_id, table_name],
    )?;

    // Insert the new current row.
    tx.execute(
        "INSERT INTO verisimdb_temporal_versions \
         (entity_id, table_name, version, valid_from, valid_to, snapshot, operation) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
        params![
            entity_id,
            table_name,
            next_version,
            now_str,
            snapshot,
            operation,
        ],
    )?;

    tx.commit()?;
    Ok(next_version as u64)
}

// =========================================================================
// Point-in-time reads
// =========================================================================

/// Read the snapshot of `(entity_id, table_name)` as it existed at
/// time `t`. Returns `None` if the entity didn't exist (or had been
/// deleted) at that instant.
///
/// "Existed at time t" means: there is a version whose
/// `valid_from <= t` and whose `valid_to` is either `NULL` (still
/// current) or `> t` (not yet superseded). When multiple versions
/// match (which shouldn't happen given the partial unique index, but
/// in case of out-of-order writes) the highest-numbered version is
/// returned.
pub fn read_at(
    conn: &Connection,
    entity_id: &str,
    table_name: &str,
    t: &DateTime<Utc>,
) -> rusqlite::Result<Option<String>> {
    let t_str = t.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT snapshot FROM verisimdb_temporal_versions \
         WHERE entity_id = ?1 AND table_name = ?2 \
           AND valid_from <= ?3 \
           AND (valid_to IS NULL OR valid_to > ?3) \
         ORDER BY version DESC \
         LIMIT 1",
    )?;
    let result = stmt
        .query_row(params![entity_id, table_name, t_str], |row| {
            row.get::<_, String>(0)
        })
        .ok();
    Ok(result)
}

/// Read the current (latest) snapshot of `(entity_id, table_name)`.
/// Returns `None` if the entity has never been recorded or has been
/// closed out without a successor.
pub fn read_current(
    conn: &Connection,
    entity_id: &str,
    table_name: &str,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT snapshot FROM verisimdb_temporal_versions \
         WHERE entity_id = ?1 AND table_name = ?2 AND valid_to IS NULL \
         LIMIT 1",
    )?;
    let result = stmt
        .query_row(params![entity_id, table_name], |row| {
            row.get::<_, String>(0)
        })
        .ok();
    Ok(result)
}

// =========================================================================
// Rollback
// =========================================================================

/// Roll back `(entity_id, table_name)` to the snapshot stored at
/// `target_version`.
///
/// Implementation: fetch the snapshot at `target_version`, then call
/// `append_version` with that snapshot and `operation = "rollback"`.
/// This preserves the audit trail — the rollback itself is a versioned
/// event, not an in-place mutation — so the chain remains append-only.
///
/// Returns the version number assigned to the newly-appended rollback
/// row. Returns an error if `target_version` doesn't exist for this
/// entity/table.
pub fn rollback_to(
    conn: &mut Connection,
    entity_id: &str,
    table_name: &str,
    target_version: u64,
) -> rusqlite::Result<u64> {
    let snapshot: String = conn.query_row(
        "SELECT snapshot FROM verisimdb_temporal_versions \
         WHERE entity_id = ?1 AND table_name = ?2 AND version = ?3",
        params![entity_id, table_name, target_version as i64],
        |row| row.get(0),
    )?;
    append_version(conn, entity_id, table_name, &snapshot, "rollback")
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
        init_sidecar_schema(&conn).unwrap();
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='verisimdb_temporal_versions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn genesis_append_starts_at_version_one() {
        let mut conn = open_sidecar();
        let v = append_version(&mut conn, "e1", "users", "{\"name\":\"Alice\"}", "insert").unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn sequential_appends_are_monotonic_and_close_previous() {
        let mut conn = open_sidecar();
        let v1 = append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        let v2 = append_version(&mut conn, "e1", "users", "{\"v\":2}", "update").unwrap();
        let v3 = append_version(&mut conn, "e1", "users", "{\"v\":3}", "update").unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
        assert_eq!(v3, 3);

        // The previous two versions have valid_to set; only the last
        // is current (partial unique index enforces this).
        let closed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_temporal_versions \
                 WHERE entity_id='e1' AND valid_to IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(closed, 2);

        let current: String = conn
            .query_row(
                "SELECT snapshot FROM verisimdb_temporal_versions \
                 WHERE entity_id='e1' AND valid_to IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(current, "{\"v\":3}");
    }

    #[test]
    fn read_current_returns_latest_snapshot() {
        let mut conn = open_sidecar();
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        append_version(&mut conn, "e1", "users", "{\"v\":2}", "update").unwrap();
        let current = read_current(&conn, "e1", "users").unwrap();
        assert_eq!(current.as_deref(), Some("{\"v\":2}"));
    }

    #[test]
    fn read_current_returns_none_for_unknown_entity() {
        let conn = open_sidecar();
        let current = read_current(&conn, "missing", "users").unwrap();
        assert!(current.is_none());
    }

    #[test]
    fn read_at_returns_snapshot_at_or_before_time() {
        let mut conn = open_sidecar();
        // Insert; sleep a tiny bit so the next valid_from is distinct.
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        let t1 = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Update.
        append_version(&mut conn, "e1", "users", "{\"v\":2}", "update").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let t2 = Utc::now();

        // At t1, the entity should look like v=1 (the v=2 update has
        // valid_from later than t1).
        let snap_at_t1 = read_at(&conn, "e1", "users", &t1).unwrap();
        assert_eq!(snap_at_t1.as_deref(), Some("{\"v\":1}"));

        // At t2, the current state is v=2.
        let snap_at_t2 = read_at(&conn, "e1", "users", &t2).unwrap();
        assert_eq!(snap_at_t2.as_deref(), Some("{\"v\":2}"));
    }

    #[test]
    fn read_at_returns_none_before_first_version() {
        let mut conn = open_sidecar();
        let before = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(20));
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        let snap = read_at(&conn, "e1", "users", &before).unwrap();
        assert!(
            snap.is_none(),
            "no version exists at a time before any insert"
        );
    }

    #[test]
    fn rollback_appends_new_version_with_old_snapshot() {
        let mut conn = open_sidecar();
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        append_version(&mut conn, "e1", "users", "{\"v\":2}", "update").unwrap();
        append_version(&mut conn, "e1", "users", "{\"v\":3}", "update").unwrap();

        // Roll back to version 1.
        let new_v = rollback_to(&mut conn, "e1", "users", 1).unwrap();
        assert_eq!(new_v, 4, "rollback creates a new version, not in-place");

        let current = read_current(&conn, "e1", "users").unwrap();
        assert_eq!(current.as_deref(), Some("{\"v\":1}"));

        let op_at_v4: String = conn
            .query_row(
                "SELECT operation FROM verisimdb_temporal_versions \
                 WHERE entity_id='e1' AND version=4",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(op_at_v4, "rollback");
    }

    #[test]
    fn rollback_unknown_version_errors() {
        let mut conn = open_sidecar();
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        assert!(rollback_to(&mut conn, "e1", "users", 99).is_err());
    }

    /// Property-style: 50 sequential appends produce strictly
    /// monotonic version numbers `1..=50` with no gaps or duplicates.
    /// The acceptance criterion in #47 calls for "property test for
    /// monotonic version numbers" — this is the deterministic version;
    /// proptest randomisation is filed as a follow-up.
    #[test]
    fn fifty_appends_yield_monotonic_versions() {
        let mut conn = open_sidecar();
        let mut seen = Vec::new();
        for i in 0..50 {
            let snap = format!("{{\"v\":{i}}}");
            let v = append_version(&mut conn, "e1", "users", &snap, "update").unwrap();
            seen.push(v);
        }
        let expected: Vec<u64> = (1..=50).collect();
        assert_eq!(seen, expected);

        // Storage layer agrees: 50 rows, all but one closed out.
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_temporal_versions WHERE entity_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 50);

        let current: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_temporal_versions \
                 WHERE entity_id='e1' AND valid_to IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(current, 1, "partial UNIQUE index keeps exactly one current");
    }

    /// Distinct entities maintain independent version sequences.
    #[test]
    fn distinct_entities_have_independent_versions() {
        let mut conn = open_sidecar();
        append_version(&mut conn, "e1", "users", "{\"v\":1}", "insert").unwrap();
        append_version(&mut conn, "e2", "users", "{\"v\":1}", "insert").unwrap();
        let v2_e1 = append_version(&mut conn, "e1", "users", "{\"v\":2}", "update").unwrap();
        let v2_e2 = append_version(&mut conn, "e2", "users", "{\"v\":2}", "update").unwrap();
        assert_eq!(v2_e1, 2);
        assert_eq!(v2_e2, 2);
    }
}
