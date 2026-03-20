// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Temporal versioning sidecar.
// Records every state change for point-in-time queries and rollback.

use serde::{Deserialize, Serialize};

/// A versioned snapshot of an entity at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVersion {
    pub entity_id: String,
    pub version: u64,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub snapshot: serde_json::Value,
    pub operation: String,
}
