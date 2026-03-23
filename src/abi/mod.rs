// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// ABI module for VeriSimiser.
//
// Defines the core domain types that form the Application Binary Interface for
// octad-augmented databases. These types are the canonical representations used
// across the Rust CLI, the Idris2 formal proofs, and the Zig FFI bridge.
//
// Idris2 proofs (in src/interface/abi/) verify:
//   - Drift detection correctness (no false negatives)
//   - Provenance chain integrity (hash chain is append-only and tamper-evident)
//   - Temporal version ordering (versions are totally ordered per entity)
//   - Sidecar isolation (Tier 1 never writes to the target database)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// OctadDimension — the 8 modalities of VeriSimDB
// ---------------------------------------------------------------------------

/// The eight dimensions of the VeriSimDB octad model.
///
/// Every piece of data in a VeriSimDB-augmented database exists simultaneously
/// across up to 8 dimensions. The first two (Data, Metadata) are inherent in
/// the target database; the remaining six are added by the verisimiser sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OctadDimension {
    /// The original data as stored in the target database.
    Data,
    /// Schema and type information extracted from the target database.
    Metadata,
    /// SHA-256 hash-chain tracking of who did what and when.
    Provenance,
    /// Directed acyclic graph of data derivation relationships.
    Lineage,
    /// Cross-dimensional invariant enforcement rules.
    Constraints,
    /// Policy-based row/column-level access permissions.
    AccessControl,
    /// Version history with point-in-time query and rollback support.
    Temporal,
    /// What-if branching and sandbox query execution.
    Simulation,
}

impl OctadDimension {
    /// Returns all 8 octad dimensions in canonical order.
    pub fn all() -> [OctadDimension; 8] {
        [
            OctadDimension::Data,
            OctadDimension::Metadata,
            OctadDimension::Provenance,
            OctadDimension::Lineage,
            OctadDimension::Constraints,
            OctadDimension::AccessControl,
            OctadDimension::Temporal,
            OctadDimension::Simulation,
        ]
    }

    /// Returns a human-readable label for this dimension.
    pub fn label(&self) -> &'static str {
        match self {
            OctadDimension::Data => "Data",
            OctadDimension::Metadata => "Metadata",
            OctadDimension::Provenance => "Provenance",
            OctadDimension::Lineage => "Lineage",
            OctadDimension::Constraints => "Constraints",
            OctadDimension::AccessControl => "Access Control",
            OctadDimension::Temporal => "Temporal",
            OctadDimension::Simulation => "Simulation",
        }
    }

    /// Returns true if this dimension is always present (Data, Metadata).
    pub fn is_inherent(&self) -> bool {
        matches!(self, OctadDimension::Data | OctadDimension::Metadata)
    }
}

impl std::fmt::Display for OctadDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// DatabaseBackend — supported target databases
// ---------------------------------------------------------------------------

/// Supported database backends that verisimiser can augment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DatabaseBackend {
    /// PostgreSQL (via logical replication, pg_notify, or triggers).
    PostgreSQL,
    /// SQLite (via sqlite3_update_hook or WAL monitoring).
    SQLite,
    /// MongoDB (via change streams).
    MongoDB,
}

impl DatabaseBackend {
    /// Parse a backend name from a string (case-insensitive).
    ///
    /// Returns `None` for unrecognised backend names.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "postgresql" | "postgres" | "pg" => Some(DatabaseBackend::PostgreSQL),
            "sqlite" | "sqlite3" => Some(DatabaseBackend::SQLite),
            "mongodb" | "mongo" => Some(DatabaseBackend::MongoDB),
            _ => None,
        }
    }

    /// Returns the canonical string name for this backend.
    pub fn name(&self) -> &'static str {
        match self {
            DatabaseBackend::PostgreSQL => "postgresql",
            DatabaseBackend::SQLite => "sqlite",
            DatabaseBackend::MongoDB => "mongodb",
        }
    }
}

impl std::fmt::Display for DatabaseBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// ProvenanceEntry — a single link in the provenance hash chain
// ---------------------------------------------------------------------------

/// A single entry in the provenance hash chain.
///
/// Each entry is cryptographically chained to its predecessor via SHA-256,
/// forming an append-only, tamper-evident log. This is the core data structure
/// for the Provenance octad dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEntry {
    /// SHA-256 hash of (previous_hash + entity_id + operation + timestamp).
    pub hash: String,
    /// Hash of the preceding entry (empty string for genesis records).
    pub previous_hash: String,
    /// Identifier of the entity this entry describes.
    pub entity_id: String,
    /// What happened: "insert", "update", "delete", "transform".
    pub operation: String,
    /// Who performed the operation (user ID, service name, or system identifier).
    pub actor: String,
    /// When the operation occurred (UTC).
    pub timestamp: DateTime<Utc>,
    /// Optional: serialised state of the entity before the operation.
    pub before_snapshot: Option<String>,
    /// Optional: description of the transformation applied.
    pub transformation: Option<String>,
}

impl ProvenanceEntry {
    /// Compute the SHA-256 hash for a provenance entry, chaining from the previous hash.
    ///
    /// The hash covers: previous_hash, entity_id, operation, and timestamp.
    /// This ensures that any tampering with the chain is detectable.
    pub fn compute_hash(
        previous_hash: &str,
        entity_id: &str,
        operation: &str,
        timestamp: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(previous_hash.as_bytes());
        hasher.update(entity_id.as_bytes());
        hasher.update(operation.as_bytes());
        hasher.update(timestamp.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify that this entry's hash is consistent with its contents.
    ///
    /// Returns `true` if the stored hash matches the recomputed hash.
    pub fn verify(&self) -> bool {
        let expected = Self::compute_hash(
            &self.previous_hash,
            &self.entity_id,
            &self.operation,
            &self.timestamp.to_rfc3339(),
        );
        self.hash == expected
    }

    /// Create a new genesis entry (first in the chain for an entity).
    pub fn genesis(entity_id: &str, actor: &str) -> Self {
        let timestamp = Utc::now();
        let hash = Self::compute_hash("", entity_id, "insert", &timestamp.to_rfc3339());
        Self {
            hash,
            previous_hash: String::new(),
            entity_id: entity_id.to_string(),
            operation: "insert".to_string(),
            actor: actor.to_string(),
            timestamp,
            before_snapshot: None,
            transformation: None,
        }
    }

    /// Create a new entry chained to this one.
    pub fn chain(&self, operation: &str, actor: &str) -> Self {
        let timestamp = Utc::now();
        let hash = Self::compute_hash(
            &self.hash,
            &self.entity_id,
            operation,
            &timestamp.to_rfc3339(),
        );
        Self {
            hash,
            previous_hash: self.hash.clone(),
            entity_id: self.entity_id.clone(),
            operation: operation.to_string(),
            actor: actor.to_string(),
            timestamp,
            before_snapshot: None,
            transformation: None,
        }
    }
}

// ---------------------------------------------------------------------------
// LineageEdge — a directed edge in the data lineage DAG
// ---------------------------------------------------------------------------

/// A directed edge in the data lineage graph.
///
/// Lineage tracks how data flows between entities: which entity was derived
/// from which other entity, and what transformation was applied. This forms
/// a DAG (directed acyclic graph) that can be traversed to answer questions
/// like "where did this data come from?" and "what downstream entities are
/// affected if this source changes?"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageEdge {
    /// Unique identifier for this edge.
    pub edge_id: String,
    /// Entity ID of the source (upstream) entity.
    pub source_entity: String,
    /// Entity ID of the target (downstream/derived) entity.
    pub target_entity: String,
    /// Type of derivation: "copy", "transform", "aggregate", "join", "filter".
    pub derivation_type: String,
    /// Optional: human-readable description of the transformation.
    pub description: Option<String>,
    /// When this lineage relationship was established.
    pub created_at: DateTime<Utc>,
}

impl LineageEdge {
    /// Create a new lineage edge between two entities.
    pub fn new(source_entity: &str, target_entity: &str, derivation_type: &str) -> Self {
        let edge_id = format!(
            "{}->{}@{}",
            source_entity,
            target_entity,
            Utc::now().timestamp_millis()
        );
        Self {
            edge_id,
            source_entity: source_entity.to_string(),
            target_entity: target_entity.to_string(),
            derivation_type: derivation_type.to_string(),
            description: None,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// TemporalVersion — a versioned snapshot of an entity at a point in time
// ---------------------------------------------------------------------------

/// A versioned snapshot of an entity at a specific point in time.
///
/// Temporal versions support point-in-time queries ("what did this entity
/// look like at 2026-01-15T14:30:00Z?") and rollback ("restore this entity
/// to version 3"). Each version records the full state, not just a diff,
/// for reliable reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVersion {
    /// Entity this version belongs to.
    pub entity_id: String,
    /// Monotonically increasing version number (1-based).
    pub version: u64,
    /// When this version became the current state.
    pub valid_from: DateTime<Utc>,
    /// When this version was superseded (None if still current).
    pub valid_to: Option<DateTime<Utc>>,
    /// Full serialised state of the entity at this version.
    pub snapshot: serde_json::Value,
    /// What operation created this version: "insert", "update", "rollback".
    pub operation: String,
}

impl TemporalVersion {
    /// Create the initial version (version 1) for a new entity.
    pub fn initial(entity_id: &str, snapshot: serde_json::Value) -> Self {
        Self {
            entity_id: entity_id.to_string(),
            version: 1,
            valid_from: Utc::now(),
            valid_to: None,
            snapshot,
            operation: "insert".to_string(),
        }
    }

    /// Create the next version, superseding this one.
    pub fn next_version(&self, snapshot: serde_json::Value, operation: &str) -> Self {
        Self {
            entity_id: self.entity_id.clone(),
            version: self.version + 1,
            valid_from: Utc::now(),
            valid_to: None,
            snapshot,
            operation: operation.to_string(),
        }
    }

    /// Returns true if this version is still current (valid_to is None).
    pub fn is_current(&self) -> bool {
        self.valid_to.is_none()
    }
}

// ---------------------------------------------------------------------------
// AccessPolicy — row/column-level access control policy
// ---------------------------------------------------------------------------

/// An access control policy governing who can see or modify specific data.
///
/// Policies are evaluated at query time to filter rows and redact columns
/// based on the requesting actor's identity and roles. This is the core
/// data structure for the Access Control octad dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    /// Unique identifier for this policy.
    pub policy_id: String,
    /// Which table or collection this policy applies to.
    pub target_table: String,
    /// Optional: specific column this policy applies to (None = whole row).
    pub target_column: Option<String>,
    /// The principal (user, role, or group) this policy grants/denies.
    pub principal: String,
    /// Access level: "read", "write", "admin", "deny".
    pub access_level: String,
    /// Optional: a SQL-like condition that further restricts the policy.
    /// For example: "department = 'engineering'" means this policy only
    /// applies to rows where department is 'engineering'.
    pub condition: Option<String>,
    /// When this policy was created.
    pub created_at: DateTime<Utc>,
    /// Whether this policy is currently active.
    pub active: bool,
}

impl AccessPolicy {
    /// Create a new access policy for a table.
    pub fn new(target_table: &str, principal: &str, access_level: &str) -> Self {
        let policy_id = format!(
            "pol-{}-{}-{}",
            target_table,
            principal,
            Utc::now().timestamp_millis()
        );
        Self {
            policy_id,
            target_table: target_table.to_string(),
            target_column: None,
            principal: principal.to_string(),
            access_level: access_level.to_string(),
            condition: None,
            created_at: Utc::now(),
            active: true,
        }
    }

    /// Create a column-level policy.
    pub fn for_column(
        target_table: &str,
        column: &str,
        principal: &str,
        access_level: &str,
    ) -> Self {
        let mut policy = Self::new(target_table, principal, access_level);
        policy.target_column = Some(column.to_string());
        policy
    }
}

// ---------------------------------------------------------------------------
// SidecarConfig — configuration for the sidecar storage
// ---------------------------------------------------------------------------

/// Configuration for the sidecar database that stores octad dimension data.
///
/// This mirrors the [sidecar] manifest section but as a runtime-usable struct
/// with validation and path resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    /// Storage backend: "sqlite" or "json".
    pub storage: String,
    /// File path for the sidecar database.
    pub path: String,
}

impl SidecarConfig {
    /// Create a default SQLite sidecar configuration.
    pub fn default_sqlite() -> Self {
        Self {
            storage: "sqlite".to_string(),
            path: ".verisimdb/sidecar.db".to_string(),
        }
    }

    /// Ensure the sidecar directory exists, creating it if necessary.
    pub fn ensure_directory(&self) -> std::io::Result<()> {
        if let Some(parent) = std::path::Path::new(&self.path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_octad_dimension_count() {
        assert_eq!(OctadDimension::all().len(), 8);
    }

    #[test]
    fn test_octad_inherent_dimensions() {
        assert!(OctadDimension::Data.is_inherent());
        assert!(OctadDimension::Metadata.is_inherent());
        assert!(!OctadDimension::Provenance.is_inherent());
    }

    #[test]
    fn test_database_backend_parsing() {
        assert_eq!(
            DatabaseBackend::from_str("postgresql"),
            Some(DatabaseBackend::PostgreSQL)
        );
        assert_eq!(
            DatabaseBackend::from_str("postgres"),
            Some(DatabaseBackend::PostgreSQL)
        );
        assert_eq!(
            DatabaseBackend::from_str("pg"),
            Some(DatabaseBackend::PostgreSQL)
        );
        assert_eq!(
            DatabaseBackend::from_str("sqlite"),
            Some(DatabaseBackend::SQLite)
        );
        assert_eq!(
            DatabaseBackend::from_str("mongodb"),
            Some(DatabaseBackend::MongoDB)
        );
        assert_eq!(DatabaseBackend::from_str("mysql"), None);
    }

    #[test]
    fn test_provenance_chain_integrity() {
        let genesis = ProvenanceEntry::genesis("entity-1", "system");
        assert!(genesis.verify());
        assert!(genesis.previous_hash.is_empty());

        let update = genesis.chain("update", "user-alice");
        assert!(update.verify());
        assert_eq!(update.previous_hash, genesis.hash);
    }

    #[test]
    fn test_provenance_tamper_detection() {
        let mut entry = ProvenanceEntry::genesis("entity-1", "system");
        // Tamper with the entity_id after hash computation.
        entry.entity_id = "entity-2".to_string();
        assert!(!entry.verify(), "Tampered entry should fail verification");
    }

    #[test]
    fn test_temporal_version_chain() {
        let v1 = TemporalVersion::initial("post-1", serde_json::json!({"title": "Hello"}));
        assert_eq!(v1.version, 1);
        assert!(v1.is_current());

        let v2 = v1.next_version(serde_json::json!({"title": "Hello World"}), "update");
        assert_eq!(v2.version, 2);
        assert!(v2.is_current());
    }

    #[test]
    fn test_lineage_edge_creation() {
        let edge = LineageEdge::new("posts", "post_summaries", "aggregate");
        assert_eq!(edge.source_entity, "posts");
        assert_eq!(edge.target_entity, "post_summaries");
        assert_eq!(edge.derivation_type, "aggregate");
    }

    #[test]
    fn test_access_policy_creation() {
        let policy = AccessPolicy::new("posts", "role:editor", "write");
        assert_eq!(policy.target_table, "posts");
        assert_eq!(policy.principal, "role:editor");
        assert!(policy.active);
        assert!(policy.target_column.is_none());

        let col_policy = AccessPolicy::for_column("users", "email", "role:admin", "read");
        assert_eq!(col_policy.target_column, Some("email".to_string()));
    }
}
