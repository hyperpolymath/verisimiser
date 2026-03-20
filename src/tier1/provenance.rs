// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Provenance tracking via SHA-256 hash chains.
// Write-path observer: records what happened, never changes what happened.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single link in the provenance hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    /// Hash of this record (SHA-256 of previous_hash + entity_id + operation + timestamp).
    pub hash: String,
    /// Hash of the previous record in the chain (empty string for genesis).
    pub previous_hash: String,
    /// Entity this record is about.
    pub entity_id: String,
    /// What happened: "create", "update", "delete", "transform".
    pub operation: String,
    /// Who did it (user, service, or system identifier).
    pub actor: String,
    /// When it happened.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Optional: what the entity looked like before (for updates/deletes).
    pub before_snapshot: Option<String>,
    /// Optional: transformation description (for derived data).
    pub transformation: Option<String>,
}

impl ProvenanceRecord {
    /// Compute the hash for this record, chaining from the previous hash.
    pub fn compute_hash(previous_hash: &str, entity_id: &str, operation: &str, timestamp: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(previous_hash.as_bytes());
        hasher.update(entity_id.as_bytes());
        hasher.update(operation.as_bytes());
        hasher.update(timestamp.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify that this record's hash is consistent with its contents.
    pub fn verify(&self) -> bool {
        let expected = Self::compute_hash(
            &self.previous_hash,
            &self.entity_id,
            &self.operation,
            &self.timestamp.to_rfc3339(),
        );
        self.hash == expected
    }
}
