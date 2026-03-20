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
