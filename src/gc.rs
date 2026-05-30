// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// `verisimiser gc` — purge sidecar rows older than the retention bound
// declared in `[retention]`. Closes #50 (V-L2-P1).
//
// Only the SQLite sidecar backend is implemented in this initial cut;
// other backends return a typed error so users see the limitation
// explicitly instead of silently no-op'ing.

use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use rusqlite::Connection;
use serde::Serialize;

use crate::manifest::Manifest;

/// Number of rows purged per dimension by [`run_gc`].
#[derive(Debug, Clone, Serialize, Default)]
pub struct GcReport {
    /// Resolved sidecar path that was operated on.
    pub sidecar: String,
    /// `true` if no changes were applied (`--dry-run`).
    pub dry_run: bool,
    /// Rows deleted from `verisimdb_provenance_log` (or "would delete" in dry-run).
    pub provenance_deleted: usize,
    /// Rows deleted from `verisimdb_temporal_versions` (superseded rows only;
    /// `valid_to IS NULL` is always kept).
    pub temporal_deleted: usize,
    /// Rows deleted from `verisimdb_lineage_graph`.
    pub lineage_deleted: usize,
}

impl GcReport {
    /// Total rows purged across all dimensions.
    pub fn total(&self) -> usize {
        self.provenance_deleted + self.temporal_deleted + self.lineage_deleted
    }
}

/// Purge sidecar rows older than the retention bound. `dry_run = true`
/// reports what would be deleted without changing the DB.
///
/// Returns `Err` if the sidecar storage is not SQLite (unsupported in
/// this cut) or if the file is unreachable.
pub fn run_gc(manifest: &Manifest, dry_run: bool) -> Result<GcReport> {
    if manifest.sidecar.storage != "sqlite" {
        bail!(
            "verisimiser gc currently only supports the SQLite sidecar backend; \
             [sidecar].storage is {:?}",
            manifest.sidecar.storage
        );
    }

    let sidecar_path = &manifest.sidecar.path;
    let conn = Connection::open(sidecar_path)
        .with_context(|| format!("opening sidecar at {}", sidecar_path))?;

    let retention = &manifest.retention;
    let mut report = GcReport {
        sidecar: sidecar_path.clone(),
        dry_run,
        ..Default::default()
    };

    if retention.provenance_days > 0 {
        report.provenance_deleted = purge_by_age(
            &conn,
            "verisimdb_provenance_log",
            "timestamp",
            retention.provenance_days,
            dry_run,
            None,
        )?;
    }
    if retention.temporal_days > 0 {
        // Only purge superseded versions — never the current one.
        report.temporal_deleted = purge_by_age(
            &conn,
            "verisimdb_temporal_versions",
            "valid_from",
            retention.temporal_days,
            dry_run,
            Some("valid_to IS NOT NULL"),
        )?;
    }
    if retention.lineage_days > 0 {
        report.lineage_deleted = purge_by_age(
            &conn,
            "verisimdb_lineage_graph",
            "created_at",
            retention.lineage_days,
            dry_run,
            None,
        )?;
    }

    Ok(report)
}

/// Delete rows where `<ts_column> < cutoff`. When `dry_run` is true,
/// counts matching rows but does not delete. `extra_where` is appended
/// with `AND (...)` so callers can scope the purge (e.g. exclude the
/// current temporal version).
///
/// `table`, `ts_column`, and `extra_where` are *trusted* — they come
/// from the codegen layer's identifier set, not user input. They are
/// inlined into the SQL because rusqlite cannot bind identifiers.
fn purge_by_age(
    conn: &Connection,
    table: &str,
    ts_column: &str,
    days: u32,
    dry_run: bool,
    extra_where: Option<&str>,
) -> Result<usize> {
    let cutoff = (Utc::now() - Duration::days(days as i64)).to_rfc3339();
    let extra = extra_where
        .map(|w| format!(" AND ({})", w))
        .unwrap_or_default();
    if dry_run {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {ts_column} < ?{extra}");
        let count: i64 = conn
            .query_row(&sql, [&cutoff], |row| row.get(0))
            .with_context(|| format!("counting purge candidates in {table}"))?;
        Ok(count as usize)
    } else {
        let sql = format!("DELETE FROM {table} WHERE {ts_column} < ?{extra}");
        let n = conn
            .execute(&sql, [&cutoff])
            .with_context(|| format!("deleting old rows from {table}"))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::run_gc;
    use crate::manifest::{Manifest, RetentionConfig, SidecarConfig};
    use rusqlite::Connection;

    /// Build a Manifest with a temp SQLite sidecar, retention set as given.
    fn fixture(sidecar_path: &str, retention: RetentionConfig, storage: &str) -> Manifest {
        let mut m: Manifest = toml::from_str(
            "[database]\n\
             backend = \"sqlite\"\n",
        )
        .unwrap();
        m.sidecar = SidecarConfig {
            storage: storage.to_string(),
            path: sidecar_path.to_string(),
        };
        m.retention = retention;
        m
    }

    /// Create the three sidecar tables and seed rows of varying ages.
    fn seed_db(path: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE verisimdb_provenance_log (
                 hash TEXT PRIMARY KEY,
                 timestamp TEXT NOT NULL
             );
             CREATE TABLE verisimdb_temporal_versions (
                 entity_id TEXT NOT NULL,
                 table_name TEXT NOT NULL,
                 version INTEGER NOT NULL,
                 valid_from TEXT NOT NULL,
                 valid_to TEXT,
                 PRIMARY KEY (entity_id, table_name, version)
             );
             CREATE TABLE verisimdb_lineage_graph (
                 edge_id TEXT PRIMARY KEY,
                 created_at TEXT NOT NULL
             );",
        )
        .unwrap();

        // Provenance: 1 old, 1 fresh
        conn.execute(
            "INSERT INTO verisimdb_provenance_log VALUES (?, ?)",
            ["old", "2020-01-01T00:00:00+00:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO verisimdb_provenance_log VALUES (?, ?)",
            ["new", "9999-01-01T00:00:00+00:00"],
        )
        .unwrap();

        // Temporal: 1 old superseded, 1 old current, 1 fresh superseded
        conn.execute(
            "INSERT INTO verisimdb_temporal_versions VALUES ('e1','t',1,'2020-01-01T00:00:00+00:00','2020-06-01T00:00:00+00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO verisimdb_temporal_versions VALUES ('e2','t',1,'2020-01-01T00:00:00+00:00',NULL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO verisimdb_temporal_versions VALUES ('e3','t',1,'9999-01-01T00:00:00+00:00','9999-06-01T00:00:00+00:00')",
            [],
        ).unwrap();

        // Lineage: 1 old, 1 fresh
        conn.execute(
            "INSERT INTO verisimdb_lineage_graph VALUES (?, ?)",
            ["old", "2020-01-01T00:00:00+00:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO verisimdb_lineage_graph VALUES (?, ?)",
            ["new", "9999-01-01T00:00:00+00:00"],
        )
        .unwrap();
    }

    #[test]
    fn gc_dry_run_counts_but_does_not_delete() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("sidecar.db");
        let sidecar_str = sidecar.to_str().unwrap();
        seed_db(sidecar_str);
        let m = fixture(
            sidecar_str,
            RetentionConfig {
                provenance_days: 30,
                temporal_days: 30,
                lineage_days: 30,
            },
            "sqlite",
        );
        let report = run_gc(&m, true).unwrap();
        assert!(report.dry_run);
        assert_eq!(report.provenance_deleted, 1, "old provenance row");
        // Only superseded ("valid_to NOT NULL") + old should be purged.
        // e1 qualifies (old + superseded). e2 is old but current. e3 is fresh.
        assert_eq!(report.temporal_deleted, 1, "old superseded temporal row");
        assert_eq!(report.lineage_deleted, 1, "old lineage row");

        // Verify nothing was actually deleted.
        let conn = Connection::open(sidecar_str).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM verisimdb_provenance_log", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 2, "dry-run must not delete");
    }

    #[test]
    fn gc_apply_deletes_old_rows() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("sidecar.db");
        let sidecar_str = sidecar.to_str().unwrap();
        seed_db(sidecar_str);
        let m = fixture(
            sidecar_str,
            RetentionConfig {
                provenance_days: 30,
                temporal_days: 30,
                lineage_days: 30,
            },
            "sqlite",
        );
        let report = run_gc(&m, false).unwrap();
        assert!(!report.dry_run);
        assert_eq!(report.total(), 3);

        let conn = Connection::open(sidecar_str).unwrap();
        let provenance_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM verisimdb_provenance_log", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(provenance_count, 1, "fresh provenance kept");

        // The current temporal version (e2, valid_to IS NULL) must survive
        // even though it is old enough to qualify on valid_from.
        let temporal_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_temporal_versions",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(temporal_count, 2);
        let current_survived: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_temporal_versions WHERE entity_id='e2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(current_survived, 1, "current temporal version must survive");
    }

    #[test]
    fn gc_retention_zero_is_forever() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("sidecar.db");
        let sidecar_str = sidecar.to_str().unwrap();
        seed_db(sidecar_str);
        let m = fixture(sidecar_str, RetentionConfig::default(), "sqlite");
        let report = run_gc(&m, false).unwrap();
        assert_eq!(report.total(), 0, "retention=0 should purge nothing");
    }

    #[test]
    fn gc_rejects_non_sqlite_backend() {
        // `postgres` is a valid generate-time dialect, but gc is SQLite-only
        // and must refuse rather than silently no-op. (The `json` value was
        // dropped as a storage option entirely in V-L2-F2 / #112.)
        let m = fixture("/dev/null", RetentionConfig::default(), "postgres");
        let err = run_gc(&m, true).unwrap_err();
        assert!(
            err.to_string().contains("only supports the SQLite sidecar"),
            "expected explicit unsupported-backend error; got: {err}"
        );
    }
}
