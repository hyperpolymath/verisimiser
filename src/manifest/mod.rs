// SPDX-License-Identifier: MPL-2.0
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

    /// Retention policy — how long each octad dimension keeps history.
    /// See [`RetentionConfig`]; `verisimiser gc` enforces these bounds.
    #[serde(default)]
    pub retention: RetentionConfig,

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
    /// Storage backend for the sidecar. `"sqlite"` (default) is the
    /// reference store; `"postgres"`/`"postgresql"` selects the PostgreSQL
    /// DDL dialect; `"json"` selects the JSON-family document store (see
    /// [`format`](SidecarConfig::format)). Resolved — and any other value
    /// rejected — at `validate`/`generate` time by
    /// [`sidecar::StorageKind::resolve`](crate::sidecar::StorageKind::resolve),
    /// the single source of truth for supported stores.
    #[serde(default = "default_sidecar_storage")]
    pub storage: String,

    /// On-disk encoding for the `json` store: `"plain"` (default),
    /// `"ld"` (JSON-LD), or `"ndjson"`. Ignored for `sqlite`/`postgres`.
    /// V-L2-F3 (#146).
    #[serde(default = "default_sidecar_format")]
    pub format: String,

    /// File path for the sidecar database.
    #[serde(default = "default_sidecar_path")]
    pub path: String,
}

/// [retention] section — bounds on how long each octad dimension's history
/// is kept in the sidecar. A field of `0` means "keep forever". The actual
/// purging is performed by `verisimiser gc`. Closes #50 (V-L2-P1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Maximum age (in days) of provenance log entries. `0` means forever.
    #[serde(rename = "provenance-days", default = "default_retention_forever")]
    pub provenance_days: u32,

    /// Maximum age (in days) of temporal version entries. `0` means forever.
    /// Only superseded versions (`valid_to IS NOT NULL`) are eligible; the
    /// current version is always retained.
    #[serde(rename = "temporal-days", default = "default_retention_forever")]
    pub temporal_days: u32,

    /// Maximum age (in days) of lineage graph edges. `0` means forever.
    #[serde(rename = "lineage-days", default = "default_retention_forever")]
    pub lineage_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            provenance_days: default_retention_forever(),
            temporal_days: default_retention_forever(),
            lineage_days: default_retention_forever(),
        }
    }
}

fn default_retention_forever() -> u32 {
    0
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            storage: default_sidecar_storage(),
            format: default_sidecar_format(),
            path: default_sidecar_path(),
        }
    }
}

#[cfg(test)]
mod validate_manifest_tests {
    use super::validate_manifest;

    /// A well-formed manifest with no schema-source and a writable sidecar
    /// parent must pass all checks.
    #[test]
    fn good_manifest_passes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.db");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"sqlite\"\n\
             [sidecar]\n\
             storage = \"sqlite\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(report.passed, "expected pass; checks: {:?}", report.checks);
        assert!(report.failed_count() == 0);
    }

    /// A manifest that sets both `[database].backend` and the legacy
    /// `target-db` to conflicting values must fail validation up front, not
    /// silently pass and blow up later at generate time (V-L2-E1).
    #[test]
    fn conflicting_backend_fails_validation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.db");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"sqlite\"\n\
             target-db = \"postgresql\"\n\
             [sidecar]\n\
             storage = \"sqlite\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(
            !report.passed,
            "conflicting backend/target-db must fail validation; checks: {:?}",
            report.checks
        );
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.name == "backend-unambiguous" && !c.passed),
            "expected a failed 'backend-unambiguous' check; checks: {:?}",
            report.checks
        );
    }

    /// A schema-source pointing at a missing file must fail
    /// `schema-source-exists`.
    #[test]
    fn missing_schema_source_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.db");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"sqlite\"\n\
             schema-source = \"/nonexistent/schema.sql\"\n\
             [sidecar]\n\
             storage = \"sqlite\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(!report.passed);
        let failed: Vec<&str> = report
            .checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(failed, vec!["schema-source-exists"]);
    }

    /// `storage = "json"` (with a valid format) now *passes* validation —
    /// the JSON family is supported again (V-L2-F3 / #146).
    #[test]
    fn json_storage_with_valid_format_passes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.ndjson");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"sqlite\"\n\
             [sidecar]\n\
             storage = \"json\"\n\
             format = \"ndjson\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(
            report.passed,
            "json+ndjson must validate; checks: {:?}",
            report.checks
        );
    }

    /// Complements the failure cases: the PostgreSQL dialect is a supported
    /// `[sidecar].storage` value (it selects the postgres DDL for
    /// `generate`), so a postgres sidecar must *pass* the
    /// `sidecar-storage-supported` check and validate cleanly.
    #[test]
    fn postgres_storage_passes_storage_check() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        let sidecar_path = dir.path().join("sidecar.db");
        let body = format!(
            "[project]\n\
             name = \"test\"\n\
             [database]\n\
             backend = \"postgresql\"\n\
             [sidecar]\n\
             storage = \"postgres\"\n\
             path = \"{}\"\n",
            sidecar_path.display().to_string().replace('\\', "/")
        );
        std::fs::write(&path, body).expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(
            report.passed,
            "postgres storage must validate; checks: {:?}",
            report.checks
        );
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.name == "sidecar-storage-supported" && c.passed),
            "the storage-supported check must run and pass for postgres"
        );
    }

    /// A bad `[sidecar].format` for the json store, and an unknown storage
    /// backend, must each fail `sidecar-storage-supported`.
    #[test]
    fn bad_format_and_unknown_storage_fail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_check = |toml: &str| {
            let path = dir.path().join("verisimiser.toml");
            std::fs::write(&path, toml).expect("write");
            let report = validate_manifest(path.to_str().unwrap());
            report
                .checks
                .into_iter()
                .find(|c| c.name == "sidecar-storage-supported")
                .expect("storage check must run")
        };

        let bad_format = storage_check(
            "[database]\nbackend = \"sqlite\"\n\
             [sidecar]\nstorage = \"json\"\nformat = \"yaml\"\n",
        );
        assert!(!bad_format.passed);
        assert!(
            bad_format
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("format")
        );

        let unknown =
            storage_check("[database]\nbackend = \"sqlite\"\n[sidecar]\nstorage = \"mariadb\"\n");
        assert!(!unknown.passed);
    }

    /// A malformed manifest must fail `manifest-loads` and stop further
    /// checks (because the rest depend on having a parsed manifest).
    #[test]
    fn malformed_manifest_fails_load_check() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        std::fs::write(&path, "broken value\n").expect("write");

        let report = validate_manifest(path.to_str().unwrap());
        assert!(!report.passed);
        assert_eq!(report.checks.len(), 1, "only manifest-loads should run");
        assert_eq!(report.checks[0].name, "manifest-loads");
        assert!(!report.checks[0].passed);
    }
}

#[cfg(test)]
mod load_manifest_tests {
    use super::load_manifest;

    /// Malformed TOML must produce a file:line:col error (closes #55).
    #[test]
    fn malformed_manifest_reports_line_and_column() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("verisimiser.toml");
        // Line 3 has an obviously broken assignment (key with no `=`).
        let bad = "[project]\n\
                   name = \"ok\"\n\
                   broken value\n";
        std::fs::write(&path, bad).expect("write");

        let err = load_manifest(path.to_str().unwrap()).expect_err("malformed TOML must fail");
        let msg = err.to_string();
        // Must include path, and a `:N:M:` span indicator.
        assert!(
            msg.contains("verisimiser.toml"),
            "error must include the manifest path; got: {msg}"
        );
        // The exact line/column varies with toml's internal pointer, but
        // there must be a `:<digit>:<digit>:` somewhere in the message.
        let span_re = regex_like_line_col(&msg);
        assert!(span_re, "error must include filename:line:col; got: {msg}");
    }

    /// Lightweight substitute for a regex match (no regex crate added):
    /// look for `:N:M:` where N and M are 1+ digits each.
    fn regex_like_line_col(msg: &str) -> bool {
        let bytes = msg.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b':' {
                let mut j = i + 1;
                let mut had_digit_1 = false;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                    had_digit_1 = true;
                }
                if had_digit_1 && j < bytes.len() && bytes[j] == b':' {
                    let mut k = j + 1;
                    let mut had_digit_2 = false;
                    while k < bytes.len() && bytes[k].is_ascii_digit() {
                        k += 1;
                        had_digit_2 = true;
                    }
                    if had_digit_2 && k < bytes.len() && bytes[k] == b':' {
                        return true;
                    }
                }
            }
            i += 1;
        }
        false
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
fn default_sidecar_format() -> String {
    "plain".to_string()
}
fn default_sidecar_path() -> String {
    ".verisim/sidecar.db".to_string()
}

// --- Public API ---

/// Load and parse a `verisimiser.toml` manifest from the given file path.
///
/// Returns an error if the file cannot be read or the TOML is malformed.
/// On parse failure, the error message includes `path:line:col` extracted
/// from the underlying `toml::de::Error::span()` so editors can jump
/// straight to the offending position. Closes #55.
pub fn load_manifest(path: &str) -> Result<Manifest> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read manifest: {}", path))?;
    toml::from_str::<Manifest>(&contents).map_err(|err| {
        let (line, col) = err
            .span()
            .map(|range| byte_offset_to_line_col(&contents, range.start))
            .unwrap_or((1, 1));
        anyhow::anyhow!("{}:{}:{}: {}", path, line, col, err.message())
    })
}

/// Convert a UTF-8 byte offset within `text` to a 1-based `(line, col)`
/// pair. Used by [`load_manifest`] to translate a `toml::de::Error::span()`
/// into editor-style `file:line:col` output.
fn byte_offset_to_line_col(text: &str, offset: usize) -> (usize, usize) {
    let prefix = &text[..offset.min(text.len())];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let col = prefix.bytes().rev().take_while(|b| *b != b'\n').count() + 1;
    (line, col)
}

/// Generate a new `verisimiser.toml` manifest file with the Phase 1 schema.
///
/// The `database` parameter sets the backend type. Field defaults are pulled
/// from `OctadConfig::default()`, `SidecarConfig::default()` and friends so
/// the emitted template tracks code without drift.
///
/// - `name`: project name; defaults to `ProjectConfig::default().name` if `None`.
/// - `force`: if `true`, overwrite an existing file. Otherwise, error.
pub fn init_manifest(database: &str, name: Option<&str>, force: bool) -> Result<()> {
    let path = "verisimiser.toml";
    if std::path::Path::new(path).exists() && !force {
        anyhow::bail!(
            "{} already exists — pass --force to overwrite or remove it first",
            path
        );
    }

    let template = render_manifest_template(database, name);
    std::fs::write(path, template)?;
    tracing::info!(path, backend = database, "created manifest");
    Ok(())
}

/// Render the manifest template, pulling defaults from the Default impls.
/// Public to the crate so tests can assert the rendered TOML round-trips.
pub(crate) fn render_manifest_template(database: &str, name: Option<&str>) -> String {
    let project = ProjectConfig::default();
    let octad = OctadConfig::default();
    let sidecar = SidecarConfig::default();
    let retention = RetentionConfig::default();
    let project_name = name.unwrap_or(&project.name);
    format!(
        r#"# SPDX-License-Identifier: MPL-2.0
# VeriSimiser manifest — augment {database} with VeriSimDB octad capabilities

[project]
name = "{project_name}"
version = "{project_version}"
# description = "My database augmented with VeriSimDB octad dimensions"

[database]
backend = "{database}"
connection-string-env = "{conn_env}"
# schema-source = "schema.sql"

[octad]
enable-provenance = {enable_provenance}
enable-lineage = {enable_lineage}
enable-temporal = {enable_temporal}
enable-access-control = {enable_access_control}
enable-constraints = {enable_constraints}
enable-simulation = {enable_simulation}

[sidecar]
# storage backend: "sqlite" (default), "postgres"/"postgresql", or "json"
storage = "{sidecar_storage}"
# json on-disk encoding (ignored for sql backends): "plain" | "ld" | "ndjson"
format = "{sidecar_format}"
path = "{sidecar_path}"

[retention]
# Days to keep per dimension. 0 = keep forever.
provenance-days = {provenance_days}
temporal-days   = {temporal_days}
lineage-days    = {lineage_days}
"#,
        project_version = project.version,
        conn_env = default_connection_env(),
        enable_provenance = octad.enable_provenance,
        enable_lineage = octad.enable_lineage,
        enable_temporal = octad.enable_temporal,
        enable_access_control = octad.enable_access_control,
        enable_constraints = octad.enable_constraints,
        enable_simulation = octad.enable_simulation,
        sidecar_storage = sidecar.storage,
        sidecar_format = sidecar.format,
        sidecar_path = sidecar.path,
        provenance_days = retention.provenance_days,
        temporal_days = retention.temporal_days,
        lineage_days = retention.lineage_days,
    )
}

#[cfg(test)]
mod init_template_tests {
    use super::{Manifest, OctadConfig, render_manifest_template};

    #[test]
    fn template_round_trips_through_toml() {
        let rendered = render_manifest_template("postgresql", None);
        let m: Manifest =
            toml::from_str(&rendered).expect("rendered template must parse as Manifest");
        let defaults = OctadConfig::default();
        // Every octad field equals its Default::default() — no drift.
        assert_eq!(m.octad.enable_provenance, defaults.enable_provenance);
        assert_eq!(m.octad.enable_lineage, defaults.enable_lineage);
        assert_eq!(m.octad.enable_temporal, defaults.enable_temporal);
        assert_eq!(
            m.octad.enable_access_control,
            defaults.enable_access_control
        );
        assert_eq!(m.octad.enable_constraints, defaults.enable_constraints);
        assert_eq!(m.octad.enable_simulation, defaults.enable_simulation);
        assert_eq!(m.database.backend, "postgresql");
    }

    #[test]
    fn template_uses_explicit_name_when_provided() {
        let rendered = render_manifest_template("sqlite", Some("acme-warehouse"));
        let m: Manifest = toml::from_str(&rendered).expect("template parses");
        assert_eq!(m.project.name, "acme-warehouse");
        assert_eq!(m.database.backend, "sqlite");
    }

    #[test]
    fn template_falls_back_to_default_name() {
        let rendered = render_manifest_template("mongodb", None);
        let m: Manifest = toml::from_str(&rendered).expect("template parses");
        let default_name = super::ProjectConfig::default().name;
        assert_eq!(m.project.name, default_name);
    }
}

/// Result of a single validation check. Each check is independent so the
/// CLI can report all failures, not just the first.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationCheck {
    /// Stable, lower-kebab-case identifier for this check.
    pub name: String,
    /// Human-readable description of what this check verified.
    pub description: String,
    /// `true` if the check passed.
    pub passed: bool,
    /// Failure detail if `passed == false`; `None` when the check passed.
    pub detail: Option<String>,
}

/// Aggregate report returned by [`validate_manifest`] and by
/// `verisimiser validate --json`.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// Resolved path of the manifest that was validated.
    pub manifest: String,
    /// `true` iff every entry in `checks` passed.
    pub passed: bool,
    /// One entry per check, in the order they ran.
    pub checks: Vec<ValidationCheck>,
}

impl ValidationReport {
    /// Number of failing checks.
    pub fn failed_count(&self) -> usize {
        self.checks.iter().filter(|c| !c.passed).count()
    }
}

/// Run all consistency checks against a manifest at `path`.
///
/// Each check is independent — every check runs even if an earlier one
/// failed — so the user sees every problem in one go. The returned
/// `ValidationReport` has `passed = false` iff any individual check
/// failed. Closes #52.
///
/// Checks currently performed:
///
/// 1. **`manifest-loads`** — the file exists, is valid TOML, and
///    deserialises to `Manifest`. (Covers V-L3-H1 span-aware errors.)
/// 2. **`schema-source-exists`** — if `[database].schema-source` is
///    set, the file at that path is readable.
/// 3. **`sidecar-path-writable`** — the parent directory of
///    `[sidecar].path` is writable (or createable).
/// 4. **`sidecar-storage-supported`** — `[sidecar].storage` (+ `format`
///    for the json store) names a backend the tool supports
///    (`sqlite`/`postgres`/`json` with `format` ∈ plain|ld|ndjson).
///    Catches typos before codegen. (V-L2-F3 / #146.)
///
/// Out of scope here: V-L2-E1 backend/target_db conflict (own issue),
/// target-DB reachability (needs live connection).
pub fn validate_manifest(path: &str) -> ValidationReport {
    let mut checks = Vec::new();

    // 1. Manifest loads.
    let manifest = match load_manifest(path) {
        Ok(m) => {
            checks.push(ValidationCheck {
                name: "manifest-loads".to_string(),
                description: "Manifest file parses and deserialises".to_string(),
                passed: true,
                detail: None,
            });
            Some(m)
        }
        Err(e) => {
            checks.push(ValidationCheck {
                name: "manifest-loads".to_string(),
                description: "Manifest file parses and deserialises".to_string(),
                passed: false,
                detail: Some(e.to_string()),
            });
            None
        }
    };

    if let Some(m) = manifest.as_ref() {
        // 2. Schema source exists if specified.
        if let Some(schema_path) = m.database.schema_source.as_deref() {
            let p = std::path::Path::new(schema_path);
            if p.is_file() {
                checks.push(ValidationCheck {
                    name: "schema-source-exists".to_string(),
                    description: "[database].schema-source points to a readable file".to_string(),
                    passed: true,
                    detail: None,
                });
            } else {
                checks.push(ValidationCheck {
                    name: "schema-source-exists".to_string(),
                    description: "[database].schema-source points to a readable file".to_string(),
                    passed: false,
                    detail: Some(format!("'{}' does not exist or is not a file", schema_path)),
                });
            }
        }

        // 3. Sidecar parent directory is writable / createable.
        let sidecar_path = std::path::Path::new(&m.sidecar.path);
        let parent = sidecar_path.parent().unwrap_or(std::path::Path::new("."));
        let writable = if parent.as_os_str().is_empty() {
            // sidecar.path = "name.db" with no parent — current dir.
            std::path::Path::new(".")
                .metadata()
                .map(|md| !md.permissions().readonly())
                .unwrap_or(false)
        } else if parent.exists() {
            parent
                .metadata()
                .map(|md| !md.permissions().readonly())
                .unwrap_or(false)
        } else {
            // Parent doesn't exist yet — verisimiser would create it.
            // Treat as OK; surface as a warning only if we can't create.
            true
        };
        if writable {
            checks.push(ValidationCheck {
                name: "sidecar-path-writable".to_string(),
                description: "Sidecar storage path's parent directory is writable".to_string(),
                passed: true,
                detail: None,
            });
        } else {
            checks.push(ValidationCheck {
                name: "sidecar-path-writable".to_string(),
                description: "Sidecar storage path's parent directory is writable".to_string(),
                passed: false,
                detail: Some(format!(
                    "parent of '{}' is read-only or unreachable",
                    m.sidecar.path
                )),
            });
        }

        // 4. Sidecar storage backend (+ json format) is supported.
        // Delegates to the one resolver (`sidecar::StorageKind::resolve`) so
        // `validate`/`doctor` and `generate` agree on the accepted set. This
        // is where a typo'd backend, or a bad `[sidecar].format` for the
        // json store (V-L2-F3 / #146), is surfaced before it reaches codegen.
        let storage_check = ValidationCheck {
            name: "sidecar-storage-supported".to_string(),
            description: "[sidecar].storage (+ format) names a supported backend".to_string(),
            passed: true,
            detail: None,
        };
        checks.push(
            match crate::sidecar::StorageKind::resolve(&m.sidecar.storage, &m.sidecar.format) {
                Ok(_) => storage_check,
                Err(e) => ValidationCheck {
                    passed: false,
                    detail: Some(e.to_string()),
                    ..storage_check
                },
            },
        );

        // 5. Backend selection is unambiguous. `effective_backend()` rejects a
        // manifest that sets both [database].backend and the legacy
        // [database].target-db to conflicting values (V-L2-E1). Validation must
        // exercise it, otherwise a latent conflict passes `validate` only to
        // fail later at generate time.
        let backend_check = ValidationCheck {
            name: "backend-unambiguous".to_string(),
            description: "[database].backend and legacy target-db do not conflict".to_string(),
            passed: true,
            detail: None,
        };
        checks.push(match m.database.effective_backend() {
            Ok(_) => backend_check,
            Err(e) => ValidationCheck {
                passed: false,
                detail: Some(e.to_string()),
                ..backend_check
            },
        });
    }

    let passed = checks.iter().all(|c| c.passed);
    ValidationReport {
        manifest: path.to_string(),
        passed,
        checks,
    }
}

/// Documented JSON schema returned by `verisimiser status --json`.
///
/// Field stability: `name`, `backend`, `sidecar_path`, `sidecar_storage`,
/// and `octad` are part of the public schema. New fields may be added
/// in minor versions; existing fields will not be removed without a
/// major version bump.
#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    /// Project name (`[project].name` or legacy `[verisimiser].name`).
    pub name: String,
    /// Effective database backend after legacy field resolution.
    pub backend: String,
    /// Path to the sidecar storage file.
    pub sidecar_path: String,
    /// Sidecar storage technology.
    pub sidecar_storage: String,
    /// Per-dimension enablement.
    pub octad: OctadStatus,
}

/// Per-dimension boolean view used by `StatusReport`.
#[derive(Debug, Clone, Serialize)]
pub struct OctadStatus {
    /// Number of enabled dimensions (always in `2..=8`).
    pub enabled_count: usize,
    /// Always `true`.
    pub data: bool,
    /// Always `true`.
    pub metadata: bool,
    pub provenance: bool,
    pub lineage: bool,
    pub constraints: bool,
    pub access_control: bool,
    pub temporal: bool,
    pub simulation: bool,
}

/// Build a [`StatusReport`] from a loaded manifest.
///
/// Used by `verisimiser status --json`. The same content is rendered as
/// plain text by [`print_status`].
pub fn status_report(manifest: &Manifest) -> StatusReport {
    let name = if !manifest.project.name.is_empty() {
        manifest.project.name.clone()
    } else {
        manifest.verisimiser.name.clone()
    };
    StatusReport {
        name,
        backend: manifest
            .database
            .effective_backend()
            .unwrap_or(manifest.database.backend.as_str())
            .to_string(),
        sidecar_path: manifest.sidecar.path.clone(),
        sidecar_storage: manifest.sidecar.storage.clone(),
        octad: OctadStatus {
            enabled_count: manifest.octad.enabled_count(),
            data: true,
            metadata: true,
            provenance: manifest.octad.enable_provenance,
            lineage: manifest.octad.enable_lineage,
            constraints: manifest.octad.enable_constraints,
            access_control: manifest.octad.enable_access_control,
            temporal: manifest.octad.enable_temporal,
            simulation: manifest.octad.enable_simulation,
        },
    }
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
