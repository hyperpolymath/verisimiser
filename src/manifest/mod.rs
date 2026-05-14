// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Manifest module for VeriSimiser.
//
// The manifest (`verisimiser.toml`) describes the user's database, which octad
// dimensions to enable, and where the sidecar database should be stored.
// Sections: [project], [database], [octad], [sidecar], [tier1] (legacy), [tier2] (legacy).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level manifest structure parsed from `verisimiser.toml`.
///
/// Supports both the new Phase 1 schema ([project], [database], [octad], [sidecar])
/// and the legacy schema ([verisimiser], [database], [tier1], [tier2]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Project metadata section — name, version, description.
    #[serde(default)]
    pub project: ProjectConfig,

    /// Database connection and backend configuration.
    pub database: DatabaseConfig,

    /// Octad dimension toggles — which of the 8 VeriSimDB dimensions are enabled.
    #[serde(default)]
    pub octad: OctadConfig,

    /// Sidecar storage configuration — where the octad data lives.
    #[serde(default)]
    pub sidecar: SidecarConfig,

    // --- Legacy fields for backward compatibility ---
    /// Legacy top-level [verisimiser] section.
    #[serde(default)]
    pub verisimiser: VeriSimiserConfig,

    /// Legacy Tier 1 config (maps to octad provenance/temporal/drift).
    #[serde(default)]
    pub tier1: Tier1Config,

    /// Legacy Tier 2 config (maps to octad simulation + future overlays).
    #[serde(default)]
    pub tier2: Tier2Config,
}

/// [project] section — metadata about this verisimiser instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Human-readable name for this augmented database project.
    pub name: String,
    /// Semantic version of this configuration.
    #[serde(default = "default_version")]
    pub version: String,
    /// Optional description for documentation and status output.
    #[serde(default)]
    pub description: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "my-augmented-db".to_string(),
            version: default_version(),
            description: None,
        }
    }
}

/// [database] section — which database backend to augment and how to connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database backend type: "postgresql", "sqlite", or "mongodb".
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Environment variable name holding the connection string.
    /// Using an env var prevents secrets from leaking into config files.
    #[serde(rename = "connection-string-env", default = "default_connection_env")]
    pub connection_string_env: String,

    /// Path to a SQL schema file describing the database structure.
    /// Used by codegen to generate the sidecar overlay tables.
    #[serde(rename = "schema-source", default)]
    pub schema_source: Option<String>,

    // --- Legacy field ---
    /// Legacy target-db field (maps to `backend`).
    #[serde(rename = "target-db", default)]
    pub target_db: String,

    /// Legacy connection-string field (direct connection string, not env var).
    #[serde(rename = "connection-string", default)]
    pub connection_string: Option<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            connection_string_env: default_connection_env(),
            schema_source: None,
            target_db: String::new(),
            connection_string: None,
        }
    }
}

impl DatabaseConfig {
    /// Returns the effective backend name, considering legacy `target_db` field.
    pub fn effective_backend(&self) -> &str {
        if !self.backend.is_empty() && self.backend != "postgresql" {
            &self.backend
        } else if !self.target_db.is_empty() {
            &self.target_db
        } else {
            &self.backend
        }
    }
}

/// [octad] section — toggles for each of the 8 VeriSimDB dimensions.
///
/// The octad dimensions are:
///   1. Data         — the original database (always present, not toggled)
///   2. Metadata     — schema and type information (always present)
///   3. Provenance   — SHA-256 hash-chain origin tracking
///   4. Lineage      — DAG of data derivation relationships
///   5. Constraints  — invariant enforcement across dimensions
///   6. Access Control — policy-based row/column permissions
///   7. Temporal     — version history and point-in-time queries
///   8. Simulation   — what-if branching and sandbox queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctadConfig {
    /// Enable provenance tracking via SHA-256 hash chains.
    #[serde(rename = "enable-provenance", default = "default_true")]
    pub enable_provenance: bool,

    /// Enable lineage graph tracking (DAG of data derivations).
    #[serde(rename = "enable-lineage", default = "default_true")]
    pub enable_lineage: bool,

    /// Enable temporal versioning (point-in-time queries, rollback).
    #[serde(rename = "enable-temporal", default = "default_true")]
    pub enable_temporal: bool,

    /// Enable access control policies (row/column-level).
    #[serde(rename = "enable-access-control", default = "default_true")]
    pub enable_access_control: bool,

    /// Enable cross-dimensional invariant enforcement.
    #[serde(rename = "enable-constraints", default = "default_true")]
    pub enable_constraints: bool,

    /// Enable simulation/sandbox mode (what-if queries on branched data).
    #[serde(rename = "enable-simulation", default)]
    pub enable_simulation: bool,
}

impl Default for OctadConfig {
    fn default() -> Self {
        Self {
            enable_provenance: true,
            enable_lineage: true,
            enable_temporal: true,
            enable_access_control: true,
            enable_constraints: true,
            enable_simulation: false,
        }
    }
}

impl OctadConfig {
    /// Returns the count of enabled octad dimensions.
    ///
    /// Data and metadata are always enabled (the two inherent dimensions);
    /// the other six are toggled via their `enable_*` fields. Result is
    /// guaranteed to be in `2..=8`.
    pub fn enabled_count(&self) -> usize {
        let mut count = 2; // data + metadata are always present
        if self.enable_provenance {
            count += 1;
        }
        if self.enable_lineage {
            count += 1;
        }
        if self.enable_temporal {
            count += 1;
        }
        if self.enable_access_control {
            count += 1;
        }
        if self.enable_constraints {
            count += 1;
        }
        if self.enable_simulation {
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod octad_tests {
    use super::OctadConfig;

    /// `enabled_count` must always fall in `2..=8` regardless of which
    /// togglable dimensions are on. Exhaustively check all 2^6 = 64
    /// combinations of the six togglable flags.
    #[test]
    fn enabled_count_is_in_range_2_to_8() {
        for bits in 0u8..(1 << 6) {
            let cfg = OctadConfig {
                enable_provenance: bits & 0b000001 != 0,
                enable_lineage: bits & 0b000010 != 0,
                enable_temporal: bits & 0b000100 != 0,
                enable_access_control: bits & 0b001000 != 0,
                enable_constraints: bits & 0b010000 != 0,
                enable_simulation: bits & 0b100000 != 0,
            };
            let n = cfg.enabled_count();
            assert!(
                (2..=8).contains(&n),
                "bits={bits:06b} produced enabled_count={n}, expected 2..=8"
            );
        }
    }

    #[test]
    fn enabled_count_with_all_off_is_two() {
        let cfg = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: false,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        assert_eq!(cfg.enabled_count(), 2);
    }

    #[test]
    fn enabled_count_with_all_on_is_eight() {
        let cfg = OctadConfig {
            enable_provenance: true,
            enable_lineage: true,
            enable_temporal: true,
            enable_access_control: true,
            enable_constraints: true,
            enable_simulation: true,
        };
        assert_eq!(cfg.enabled_count(), 8);
    }
}

/// [sidecar] section — where the octad dimension data is physically stored.
///
/// The sidecar is a separate database that holds provenance logs, lineage graphs,
/// temporal versions, and access policies. It never writes to your target database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    /// Storage backend for the sidecar: "sqlite" (default) or "json".
    #[serde(default = "default_sidecar_storage")]
    pub storage: String,

    /// File path for the sidecar database.
    #[serde(default = "default_sidecar_path")]
    pub path: String,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            storage: default_sidecar_storage(),
            path: default_sidecar_path(),
        }
    }
}

// --- Legacy config structs (backward compatibility) ---

/// Legacy [verisimiser] section.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VeriSimiserConfig {
    #[serde(default)]
    pub name: String,
}

/// Legacy [tier1] section — maps to octad provenance/temporal/drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier1Config {
    #[serde(rename = "drift-detection", default = "default_true")]
    pub drift_detection: bool,
    #[serde(default = "default_true")]
    pub provenance: bool,
    #[serde(rename = "temporal-versioning", default = "default_true")]
    pub temporal_versioning: bool,
}

impl Default for Tier1Config {
    fn default() -> Self {
        Self {
            drift_detection: true,
            provenance: true,
            temporal_versioning: true,
        }
    }
}

/// Legacy [tier2] section — maps to augmentation overlays.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tier2Config {
    #[serde(default)]
    pub graph: bool,
    #[serde(default)]
    pub vector: bool,
    #[serde(default)]
    pub tensor: bool,
    #[serde(default)]
    pub semantic: bool,
    #[serde(default)]
    pub document: bool,
    #[serde(default)]
    pub spatial: bool,
}

// --- Default value functions ---

fn default_true() -> bool {
    true
}
fn default_version() -> String {
    "0.1.0".to_string()
}
fn default_backend() -> String {
    "postgresql".to_string()
}
fn default_connection_env() -> String {
    "DATABASE_URL".to_string()
}
fn default_sidecar_storage() -> String {
    "sqlite".to_string()
}
fn default_sidecar_path() -> String {
    ".verisim/sidecar.db".to_string()
}

// --- Public API ---

/// Load and parse a `verisimiser.toml` manifest from the given file path.
///
/// Returns an error if the file cannot be read or the TOML is malformed.
pub fn load_manifest(path: &str) -> Result<Manifest> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read manifest: {}", path))?;
    toml::from_str(&contents).with_context(|| format!("Failed to parse manifest: {}", path))
}

/// Generate a new `verisimiser.toml` manifest file with the Phase 1 schema.
///
/// The `database` parameter sets the backend type (postgresql, sqlite, mongodb).
/// Fails if the file already exists to prevent accidental overwrites.
pub fn init_manifest(database: &str) -> Result<()> {
    let path = "verisimiser.toml";
    if std::path::Path::new(path).exists() {
        anyhow::bail!("{} already exists — remove it first to reinitialise", path);
    }

    // Simulation is unimplemented across all backends; placeholder "false".
    let enable_simulation = "false";

    let template = format!(
        r#"# SPDX-License-Identifier: PMPL-1.0-or-later
# VeriSimiser manifest — augment {database} with VeriSimDB octad capabilities

[project]
name = "my-augmented-db"
version = "0.1.0"
# description = "My database augmented with VeriSimDB octad dimensions"

[database]
backend = "{database}"
connection-string-env = "DATABASE_URL"
# schema-source = "schema.sql"

[octad]
enable-provenance = true
enable-lineage = true
enable-temporal = true
enable-access-control = true
enable-constraints = true
enable-simulation = {enable_simulation}

[sidecar]
storage = "sqlite"
path = ".verisim/sidecar.db"
"#
    );

    std::fs::write(path, template)?;
    println!("Created {} for {} backend", path, database);
    Ok(())
}

/// Print a human-readable status summary of a loaded manifest.
pub fn print_status(manifest: &Manifest) {
    let name = if !manifest.project.name.is_empty() {
        &manifest.project.name
    } else {
        &manifest.verisimiser.name
    };

    let backend = manifest.database.effective_backend();

    println!("=== VeriSimiser: {} ===", name);
    println!("Backend: {}", backend);
    println!(
        "Sidecar: {} ({})",
        manifest.sidecar.path, manifest.sidecar.storage
    );
    println!();

    println!(
        "Octad Dimensions ({}/8 enabled):",
        manifest.octad.enabled_count()
    );
    println!("  Data:           ALWAYS ON (your database)");
    println!("  Metadata:       ALWAYS ON (schema introspection)");
    println!(
        "  Provenance:     {}",
        if manifest.octad.enable_provenance {
            "ON"
        } else {
            "off"
        }
    );
    println!(
        "  Lineage:        {}",
        if manifest.octad.enable_lineage {
            "ON"
        } else {
            "off"
        }
    );
    println!(
        "  Constraints:    {}",
        if manifest.octad.enable_constraints {
            "ON"
        } else {
            "off"
        }
    );
    println!(
        "  Access Control: {}",
        if manifest.octad.enable_access_control {
            "ON"
        } else {
            "off"
        }
    );
    println!(
        "  Temporal:       {}",
        if manifest.octad.enable_temporal {
            "ON"
        } else {
            "off"
        }
    );
    println!(
        "  Simulation:     {}",
        if manifest.octad.enable_simulation {
            "ON"
        } else {
            "off"
        }
    );
}
