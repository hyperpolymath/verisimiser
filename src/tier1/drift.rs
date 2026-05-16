// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Cross-modal drift detection.
// Monitors consistency across octad modality representations.
// Read-path observer: intercepts query results, never modifies them.

use serde::{Deserialize, Serialize};

/// The 8 categories of cross-modal drift that VeriSimDB detects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftCategory {
    /// Schema changes not reflected across modalities.
    Structural,
    /// Meaning divergence between representations.
    Semantic,
    /// Version skew between modalities.
    Temporal,
    /// Distribution shift in vector/tensor spaces.
    Statistical,
    /// Broken links between graph and document modalities.
    Referential,
    /// Transformation chain inconsistencies.
    Provenance,
    /// Coordinates inconsistent with other modalities.
    Spatial,
    /// Vector embeddings stale relative to source documents.
    Embedding,
}

/// A drift measurement for a single entity across modalities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    /// Entity identifier.
    pub entity_id: String,
    /// Overall drift score (0.0 = perfectly consistent, 1.0 = fully diverged).
    pub overall_score: f64,
    /// Per-category drift scores.
    pub categories: Vec<(DriftCategory, f64)>,
    /// Timestamp of measurement.
    pub measured_at: chrono::DateTime<chrono::Utc>,
}

/// Compute the Temporal drift score for one entity per ADR-0003 §3.1.
///
/// Reads the latest version per `table_name` for `entity_id` from
/// `verisimdb_temporal_versions`. Score is the max pairwise drift:
/// `|v_a - v_b| / max(v_a, v_b, 1)`, where `v_*` is the latest
/// version recorded under each modality (`table_name`).
///
/// Returns `Ok(None)` if the entity is recorded under fewer than two
/// `table_name`s — Temporal drift requires at least two modalities
/// to compare.
///
/// Closes #49. The function is intentionally narrow: one entity, one
/// category. A higher-level pass over all entities is the
/// responsibility of `verisimiser drift`.
pub fn detect_temporal_drift(
    conn: &rusqlite::Connection,
    entity_id: &str,
) -> rusqlite::Result<Option<DriftReport>> {
    let mut stmt = conn.prepare(
        "SELECT MAX(version) FROM verisimdb_temporal_versions \
         WHERE entity_id = ?1 GROUP BY table_name",
    )?;
    let versions: Vec<i64> = stmt
        .query_map([entity_id], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if versions.len() < 2 {
        return Ok(None);
    }

    let score = temporal_drift_score(&versions);
    Ok(Some(DriftReport {
        entity_id: entity_id.to_string(),
        overall_score: score,
        categories: vec![(DriftCategory::Temporal, score)],
        measured_at: chrono::Utc::now(),
    }))
}

/// Pure-Rust kernel of the Temporal drift score — extracted so unit
/// tests can exercise it without touching SQLite.
///
/// `versions` is the latest version per modality for a single entity.
/// Returns the max pairwise `|v_a - v_b| / max(v_a, v_b, 1)` over the
/// list, clamped to `[0.0, 1.0]`. With fewer than two versions the
/// function returns `0.0` (caller should generally short-circuit
/// before this — see [`detect_temporal_drift`]).
pub fn temporal_drift_score(versions: &[i64]) -> f64 {
    let mut max_score: f64 = 0.0;
    for i in 0..versions.len() {
        for j in (i + 1)..versions.len() {
            let a = versions[i];
            let b = versions[j];
            let diff = (a - b).abs() as f64;
            let denom = a.max(b).max(1) as f64;
            let s = (diff / denom).clamp(0.0, 1.0);
            if s > max_score {
                max_score = s;
            }
        }
    }
    max_score
}

#[cfg(test)]
mod temporal_drift_tests {
    use super::{DriftCategory, detect_temporal_drift, temporal_drift_score};
    use rusqlite::Connection;

    /// Identical versions → score 0.0.
    #[test]
    fn identical_versions_score_zero() {
        assert_eq!(temporal_drift_score(&[5, 5, 5]), 0.0);
    }

    /// `|10-9| / 10 = 0.1` — at threshold per ADR-0003.
    #[test]
    fn one_off_high_version_score_point_one() {
        assert!((temporal_drift_score(&[10, 9]) - 0.1).abs() < 1e-12);
    }

    /// `|5-4| / 5 = 0.2` — above threshold.
    #[test]
    fn drifted_score_above_threshold() {
        assert!((temporal_drift_score(&[5, 4]) - 0.2).abs() < 1e-12);
    }

    /// Maximally drifted: `|10-0| / 10 = 1.0`.
    #[test]
    fn one_zero_score_one() {
        assert_eq!(temporal_drift_score(&[10, 0]), 1.0);
    }

    /// Order doesn't matter.
    #[test]
    fn score_symmetric() {
        let a = temporal_drift_score(&[10, 4, 7]);
        let b = temporal_drift_score(&[7, 4, 10]);
        let c = temporal_drift_score(&[4, 7, 10]);
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    /// Score is the *max* pairwise drift, not the mean.
    #[test]
    fn score_is_max_pairwise() {
        // (10, 9): 0.1; (10, 0): 1.0; (9, 0): 1.0 — answer 1.0
        let score = temporal_drift_score(&[10, 9, 0]);
        assert!((score - 1.0).abs() < 1e-12);
    }

    /// Property: every output stays in `[0, 1]` regardless of input.
    #[test]
    fn score_clamped_to_unit_interval() {
        for case in [
            vec![1i64],
            vec![0, 0],
            vec![1000, 1],
            vec![i64::MAX, 1],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        ] {
            let s = temporal_drift_score(&case);
            assert!(
                (0.0..=1.0).contains(&s),
                "score {s} out of [0,1] for {case:?}"
            );
        }
    }

    /// `detect_temporal_drift` returns `None` for an entity recorded
    /// under fewer than two modalities — there is nothing to compare.
    #[test]
    fn detect_returns_none_below_two_modalities() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE verisimdb_temporal_versions (
                entity_id TEXT NOT NULL,
                table_name TEXT NOT NULL,
                version INTEGER NOT NULL,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                snapshot TEXT NOT NULL,
                operation TEXT NOT NULL,
                PRIMARY KEY (entity_id, table_name, version)
            );
            INSERT INTO verisimdb_temporal_versions
            VALUES ('e1','posts',1,'2026-01-01','2026-02-01','{}','insert'),
                   ('e1','posts',2,'2026-02-01',NULL,'{}','update');",
        )
        .unwrap();
        let report = detect_temporal_drift(&conn, "e1").unwrap();
        assert!(report.is_none(), "single-modality entity → None");
    }

    /// Worked example end-to-end: two modalities at versions 5 and 4
    /// → score 0.2, populated into a DriftReport whose categories list
    /// is exactly one `(Temporal, 0.2)` pair.
    #[test]
    fn detect_produces_drift_report_for_two_modalities() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE verisimdb_temporal_versions (
                entity_id TEXT NOT NULL,
                table_name TEXT NOT NULL,
                version INTEGER NOT NULL,
                valid_from TEXT NOT NULL,
                valid_to TEXT,
                snapshot TEXT NOT NULL,
                operation TEXT NOT NULL,
                PRIMARY KEY (entity_id, table_name, version)
            );
            INSERT INTO verisimdb_temporal_versions
            VALUES ('e1','posts',5,'2026-05-01',NULL,'{}','update'),
                   ('e1','posts_graph',4,'2026-04-01',NULL,'{}','update');",
        )
        .unwrap();
        let report = detect_temporal_drift(&conn, "e1").unwrap().unwrap();
        assert_eq!(report.entity_id, "e1");
        assert!((report.overall_score - 0.2).abs() < 1e-12);
        assert_eq!(report.categories.len(), 1);
        assert_eq!(report.categories[0].0, DriftCategory::Temporal);
        assert!((report.categories[0].1 - 0.2).abs() < 1e-12);
    }
}
