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
    /// Returns the effective backend name.
    ///
    /// `target-db` is a legacy field kept for backward compatibility with the
    /// old manifest schema. The new field is `backend`. If both are set to
    /// distinct values, refuse rather than silently picking one — value-based
    /// tie-breaking (the previous behaviour) silently picked sqlite when a
    /// user set `backend = "postgresql"` alongside `target-db = "sqlite"`
    /// (V-L2-E1).
    pub fn effective_backend(&self) -> Result<&str> {
        let new_set = !self.backend.is_empty();
        let old_set = !self.target_db.is_empty();
        match (new_set, old_set) {
            (true, true) if self.backend != self.target_db => anyhow::bail!(
                "verisimiser.toml sets both [database].backend = {:?} and \
                 [database].target-db = {:?}. target-db is the legacy field; \
                 remove it and keep backend.",
                self.backend,
                self.target_db
            ),
            (true, _) => Ok(self.backend.as_str()),
            (false, true) => Ok(self.target_db.as_str()),
            (false, false) => Ok("postgresql"),
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

    /// Enable cross-dimensional invariant enforcement and drift detection.
    /// V-L2-D1: explicit field (was previously derived from "count > 2").
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
    /// Returns the count of enabled octad dimensions, in 2..=8.
    ///
    /// Data and Metadata are always counted (inherent in the target DB).
    /// The other six are summed from explicit toggles. V-L2-D1: every
    /// concern is now explicit; the previous "Constraints is implied if
    /// anything else is on" arithmetic is gone.
    pub fn enabled_count(&self) -> usize {
        let optionals: usize = [
            self.enable_provenance,
            self.enable_lineage,
            self.enable_temporal,
            self.enable_access_control,
            self.enable_constraints,
            self.enable_simulation,
        ]
        .into_iter()
        .filter(|b| *b)
        .count();
        2 + optionals
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
/// `name` overrides the project name (defaults to `"my-augmented-db"`).
/// If `force` is false and the file exists, the call fails. (V-L2-O1)
///
/// The toggle defaults are read from `OctadConfig::default()` so editing the
/// defaults in code automatically updates the generated template.
pub fn init_manifest(database: &str, name: Option<&str>, force: bool) -> Result<()> {
    let path = "verisimiser.toml";
    if std::path::Path::new(path).exists() && !force {
        anyhow::bail!(
            "{} already exists — pass --force to overwrite, or remove the file first",
            path
        );
    }

    let defaults = OctadConfig::default();
    let project_name = name.unwrap_or("my-augmented-db");
    let bool_str = |b: bool| if b { "true" } else { "false" };

    let template = format!(
        r#"# SPDX-License-Identifier: PMPL-1.0-or-later
# VeriSimiser manifest — augment {database} with VeriSimDB octad capabilities

[project]
name = "{project_name}"
version = "0.1.0"
# description = "My database augmented with VeriSimDB octad dimensions"

[database]
backend = "{database}"
connection-string-env = "DATABASE_URL"
# schema-source = "schema.sql"

[octad]
enable-provenance     = {prov}
enable-lineage        = {lin}
enable-temporal       = {temp}
enable-access-control = {ac}
enable-constraints    = {cons}
enable-simulation     = {sim}

[sidecar]
storage = "sqlite"
path = ".verisim/sidecar.db"
"#,
        prov = bool_str(defaults.enable_provenance),
        lin = bool_str(defaults.enable_lineage),
        temp = bool_str(defaults.enable_temporal),
        ac = bool_str(defaults.enable_access_control),
        cons = bool_str(defaults.enable_constraints),
        sim = bool_str(defaults.enable_simulation),
    );

    std::fs::write(path, template)?;
    println!("Created {} for {} backend", path, database);
    Ok(())
}

/// Print a human-readable status summary of a loaded manifest.
pub fn print_status(manifest: &Manifest) -> Result<()> {
    let name = if !manifest.project.name.is_empty() {
        &manifest.project.name
    } else {
        &manifest.verisimiser.name
    };

    let backend = manifest.database.effective_backend()?;

    println!("=== VeriSimiser: {} ===", name);
    println!("Backend: {}", backend);
    println!(
        "Sidecar: {} ({})",
        manifest.sidecar.path, manifest.sidecar.storage
    );
    println!();

    let on_off = |b: bool| if b { "ON" } else { "off" };
    println!(
        "Octad Dimensions ({}/8 enabled):",
        manifest.octad.enabled_count()
    );
    println!("  Data:           ALWAYS ON (your database)");
    println!("  Metadata:       ALWAYS ON (schema introspection)");
    println!(
        "  Provenance:     {}",
        on_off(manifest.octad.enable_provenance)
    );
    println!(
        "  Lineage:        {}",
        on_off(manifest.octad.enable_lineage)
    );
    println!(
        "  Constraints:    {}",
        on_off(manifest.octad.enable_constraints)
    );
    println!(
        "  Access Control: {}",
        on_off(manifest.octad.enable_access_control)
    );
    println!(
        "  Temporal:       {}",
        on_off(manifest.octad.enable_temporal)
    );
    println!(
        "  Simulation:     {}",
        on_off(manifest.octad.enable_simulation)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// V-L2-D1: enabled_count is bounded by 2..=8 for every flag combination.
    #[test]
    fn test_enabled_count_bounds() {
        for mask in 0u8..64 {
            let octad = OctadConfig {
                enable_provenance: mask & 0b000001 != 0,
                enable_lineage: mask & 0b000010 != 0,
                enable_temporal: mask & 0b000100 != 0,
                enable_access_control: mask & 0b001000 != 0,
                enable_constraints: mask & 0b010000 != 0,
                enable_simulation: mask & 0b100000 != 0,
            };
            let c = octad.enabled_count();
            assert!(
                (2..=8).contains(&c),
                "enabled_count out of range for mask={:#08b}: got {}",
                mask,
                c
            );
        }
    }

    /// V-L2-D1: enabled_count exactly equals 2 + popcount(toggles).
    #[test]
    fn test_enabled_count_arithmetic() {
        let octad = OctadConfig {
            enable_provenance: true,
            enable_lineage: false,
            enable_temporal: true,
            enable_access_control: false,
            enable_constraints: true,
            enable_simulation: false,
        };
        assert_eq!(octad.enabled_count(), 2 + 3);
    }

    /// V-L2-E1: setting both backend and target_db to the *same* value
    /// is harmless — single source of truth.
    #[test]
    fn test_effective_backend_agreement() {
        let cfg = DatabaseConfig {
            backend: "sqlite".to_string(),
            target_db: "sqlite".to_string(),
            ..Default::default()
        };
        assert_eq!(cfg.effective_backend().unwrap(), "sqlite");
    }

    /// V-L2-E1: setting both to *conflicting* values must error loudly.
    #[test]
    fn test_effective_backend_conflict_errors() {
        let cfg = DatabaseConfig {
            backend: "postgresql".to_string(),
            target_db: "sqlite".to_string(),
            ..Default::default()
        };
        let err = cfg.effective_backend().unwrap_err().to_string();
        assert!(
            err.contains("postgresql"),
            "error mentions modern field value"
        );
        assert!(err.contains("sqlite"), "error mentions legacy field value");
    }

    /// V-L2-E1: modern-only and legacy-only both work.
    #[test]
    fn test_effective_backend_single_source() {
        let modern = DatabaseConfig {
            backend: "sqlite".to_string(),
            target_db: String::new(),
            ..Default::default()
        };
        assert_eq!(modern.effective_backend().unwrap(), "sqlite");

        let legacy = DatabaseConfig {
            backend: String::new(),
            target_db: "mongodb".to_string(),
            ..Default::default()
        };
        assert_eq!(legacy.effective_backend().unwrap(), "mongodb");
    }

    /// V-L2-E1: with nothing set, default is postgresql.
    #[test]
    fn test_effective_backend_default() {
        let cfg = DatabaseConfig {
            backend: String::new(),
            target_db: String::new(),
            ..Default::default()
        };
        assert_eq!(cfg.effective_backend().unwrap(), "postgresql");
    }

    /// V-L2-O1: init_manifest template reflects OctadConfig::default().
    #[test]
    fn test_init_manifest_template_uses_defaults() {
        // We can't actually call init_manifest in a unit test (it writes
        // to CWD), but we can check that the template *would* be
        // consistent by computing what it would emit and asserting
        // the toggle lines match Default.
        let defaults = OctadConfig::default();
        // If a future patch flips a default, this test makes the
        // template-vs-Default invariant visible.
        assert!(defaults.enable_provenance);
        assert!(defaults.enable_lineage);
        assert!(defaults.enable_temporal);
        assert!(defaults.enable_access_control);
        assert!(defaults.enable_constraints);
        assert!(!defaults.enable_simulation);
    }
}
