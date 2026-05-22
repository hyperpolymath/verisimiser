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
        assert!(
            report.passed,
            "expected pass; checks: {:?}",
            report.checks
        );
        assert!(report.failed_count() == 0);
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
        assert!(
            span_re,
            "error must include filename:line:col; got: {msg}"
        );
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
    let col = prefix
        .bytes()
        .rev()
        .take_while(|b| *b != b'\n')
        .count()
        + 1;
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
    println!("Created {} for {} backend", path, database);
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
storage = "{sidecar_storage}"
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
        sidecar_path = sidecar.path,
        provenance_days = retention.provenance_days,
        temporal_days = retention.temporal_days,
        lineage_days = retention.lineage_days,
    )
}

#[cfg(test)]
mod init_template_tests {
    use super::{render_manifest_template, Manifest, OctadConfig};

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
        backend: manifest.database.effective_backend().to_string(),
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
