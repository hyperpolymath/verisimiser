// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Sidecar overlay schema generator for VeriSimiser.
//
// Given a parsed database schema and octad configuration, this module generates
// the SQL DDL for the sidecar database tables. These tables store the octad
// dimension data (provenance logs, lineage graphs, temporal versions, access
// policies) without modifying the target database.
//
// Generated tables:
//   - verisimdb_provenance_log:   Hash-chained audit trail
//   - verisimdb_lineage_graph:    Data derivation DAG edges
//   - verisimdb_temporal_versions: Point-in-time entity snapshots
//   - verisimdb_access_policies:  Row/column access control rules

use crate::codegen::parser::ParsedSchema;
use crate::manifest::OctadConfig;

// ---------------------------------------------------------------------------
// Identifier validation (V-L2-G1)
// ---------------------------------------------------------------------------

/// Permitted identifier shape for any user-controlled name that flows into
/// generated DDL: leading ASCII letter or underscore, then ASCII letters,
/// digits, or underscores. This is a deliberately conservative subset of
/// SQL's quoted-identifier rules — it rejects names that would be valid
/// under quoting but make our `format!()`-based DDL emission unsafe.
///
/// Returns `Err` with the offending identifier quoted so the user can
/// rename or alias the source table.
fn validate_identifier(name: &str) -> std::result::Result<&str, String> {
    if name.is_empty() {
        return Err("identifier is empty".into());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "identifier {:?} must start with an ASCII letter or underscore",
            name
        ));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "identifier {:?} contains invalid character {:?}; \
                 only ASCII letters, digits, and underscores are allowed \
                 in identifiers that flow into generated DDL (V-L2-G1)",
                name, c
            ));
        }
    }
    Ok(name)
}

/// Convenience: validate and panic with a structured message if invalid.
/// Used in the few DDL-emitting paths that don't propagate errors.
fn must_validate_identifier(name: &str) -> &str {
    match validate_identifier(name) {
        Ok(n) => n,
        Err(e) => panic!("invalid identifier in generated DDL: {}", e),
    }
}

// ---------------------------------------------------------------------------
// SQL dialect (V-L2-F1, #45)
// ---------------------------------------------------------------------------

/// The SQL dialect the sidecar DDL is emitted for. Selected from the
/// manifest's `[sidecar].storage`. The table bodies are written in the
/// portable subset both engines accept (`CREATE TABLE IF NOT EXISTS`,
/// `CHECK`, partial unique indexes, `CURRENT_TIMESTAMP`); the only
/// genuinely dialect-divergent fragment is the metadata upsert
/// (`INSERT OR IGNORE` vs `INSERT … ON CONFLICT DO NOTHING`), which lives
/// in the [`sqlite`] / [`postgres`] modules.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SqlDialect {
    Sqlite,
    Postgres,
}

impl SqlDialect {
    /// Map a `[sidecar].storage` value to a dialect. `sqlite` →
    /// [`SqlDialect::Sqlite`]; `postgres`/`postgresql` →
    /// [`SqlDialect::Postgres`]. `json` and unknown values are rejected
    /// (the previous behaviour silently emitted SQLite DDL regardless,
    /// V-L2-F1). The JSON store is tracked separately by #112.
    pub fn from_storage(storage: &str) -> anyhow::Result<Self> {
        match storage.to_lowercase().as_str() {
            "sqlite" => Ok(SqlDialect::Sqlite),
            "postgres" | "postgresql" => Ok(SqlDialect::Postgres),
            "json" => anyhow::bail!(
                "[sidecar].storage = \"json\" is not implemented (it previously \
                 emitted SQLite DDL silently). Use \"sqlite\". The JSON sidecar \
                 store is tracked by hyperpolymath/verisimiser#112."
            ),
            other => anyhow::bail!(
                "unknown [sidecar].storage {other:?}; supported: \"sqlite\" \
                 (\"postgres\" for a PostgreSQL sidecar; \"json\" is #112)."
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Overlay generation
// ---------------------------------------------------------------------------

/// Generate the complete sidecar schema DDL for all enabled octad
/// dimensions, in the requested SQL `dialect`.
///
/// Each table is created with `IF NOT EXISTS` so the schema can be applied
/// idempotently. Dispatches to [`sqlite::generate`] / [`postgres::generate`].
///
/// # Arguments
/// * `schema` - The parsed schema of the target database (used to reference
///   table names in provenance and temporal tracking).
/// * `octad` - The octad configuration from the manifest, controlling which
///   dimension tables to generate.
/// * `dialect` - The sidecar SQL dialect (see [`SqlDialect::from_storage`]).
///
/// # Returns
/// `Ok(String)` with the complete DDL on success. `Err` if any table or
/// column name in `schema` is not a valid SQL identifier per
/// [`crate::codegen::ident::validate_identifier`] — guards against SQL
/// injection via parsed schema input. Closes #39.
pub fn generate_sidecar_schema(
    schema: &ParsedSchema,
    octad: &OctadConfig,
    dialect: SqlDialect,
) -> anyhow::Result<String> {
    match dialect {
        SqlDialect::Sqlite => sqlite::generate(schema, octad),
        SqlDialect::Postgres => postgres::generate(schema, octad),
    }
}

/// Shared schema body: validate identifiers, then assemble every enabled
/// octad table. The metadata *seed* (the only dialect-divergent fragment)
/// is supplied by the caller via `seed`.
fn assemble(
    schema: &ParsedSchema,
    octad: &OctadConfig,
    seed: impl Fn(&ParsedSchema) -> String,
) -> anyhow::Result<String> {
    use crate::codegen::ident::validate_identifier;

    // Fail fast on any unsafe identifier flowing into generated DDL.
    for table in &schema.tables {
        validate_identifier(&table.name, "table name")?;
        for column in &table.columns {
            validate_identifier(&column.name, "column name")?;
        }
    }

    let mut ddl = String::new();

    ddl.push_str("-- SPDX-License-Identifier: PMPL-1.0-or-later\n");
    ddl.push_str("-- VeriSimiser sidecar schema (auto-generated)\n");
    ddl.push_str("-- Do not edit manually; regenerate with `verisimiser init`.\n\n");

    // Metadata table: tracks which target tables are being augmented.
    // The CREATE is portable; the seed upsert is dialect-specific.
    ddl.push_str(&metadata_table_ddl());
    ddl.push_str(&seed(schema));

    if octad.enable_provenance {
        ddl.push_str(&generate_provenance_table());
    }

    if octad.enable_lineage {
        ddl.push_str(&generate_lineage_table());
    }

    if octad.enable_temporal {
        ddl.push_str(&generate_temporal_table());
    }

    if octad.enable_access_control {
        ddl.push_str(&generate_access_policy_table());
    }

    if octad.enable_simulation {
        ddl.push_str(&generate_simulation_table());
    }

    Ok(ddl)
}

/// The metadata table CREATE — portable across SQLite and PostgreSQL.
///
/// This table is always created regardless of octad configuration, because
/// the Data and Metadata dimensions are always active. The per-table seed
/// rows are dialect-specific and emitted by [`sqlite`] / [`postgres`].
fn metadata_table_ddl() -> String {
    "-- Metadata: tracks augmented target tables\n\
     CREATE TABLE IF NOT EXISTS verisimdb_metadata (\n\
     \x20   table_name   TEXT PRIMARY KEY,\n\
     \x20   column_count INTEGER NOT NULL,\n\
     \x20   pk_columns   TEXT NOT NULL,   -- comma-separated list of PK column names\n\
     \x20   discovered_at TEXT NOT NULL    -- ISO 8601 timestamp\n\
     );\n\n"
        .to_string()
}

/// Per-table seed values, shared by both dialects. Returns
/// `(validated_table_name, column_count, validated_pk_csv)`.
///
/// V-L2-G1: every identifier flowing into the SQL string here is
/// validated. Anything that wouldn't match `^[A-Za-z_][A-Za-z0-9_]*$`
/// is rejected at codegen time rather than allowed to land in DDL
/// (where it would be an injection vector).
fn metadata_rows(schema: &ParsedSchema) -> Vec<(&str, usize, String)> {
    schema
        .tables
        .iter()
        .map(|table| {
            let name = must_validate_identifier(&table.name);
            let pk_csv = table
                .columns
                .iter()
                .filter(|c| c.is_primary_key)
                .map(|c| must_validate_identifier(c.name.as_str()))
                .collect::<Vec<_>>()
                .join(",");
            (name, table.columns.len(), pk_csv)
        })
        .collect()
}

/// SQLite-specific sidecar DDL emission (V-L2-F1, #45).
pub mod sqlite {
    use super::*;

    /// SQLite metadata seed: `INSERT OR IGNORE` + portable
    /// `CURRENT_TIMESTAMP` (was the SQLite-only `datetime('now')`).
    pub(super) fn metadata_seed(schema: &ParsedSchema) -> String {
        if schema.tables.is_empty() {
            return String::new();
        }
        let mut ddl = String::from("-- Seed metadata from parsed schema (SQLite)\n");
        for (name, ncols, pk_csv) in metadata_rows(schema) {
            ddl.push_str(&format!(
                "INSERT OR IGNORE INTO verisimdb_metadata (table_name, column_count, pk_columns, discovered_at)\n\
                 \x20   VALUES ('{}', {}, '{}', CURRENT_TIMESTAMP);\n",
                name, ncols, pk_csv,
            ));
        }
        ddl.push('\n');
        ddl
    }

    /// Generate the full SQLite sidecar schema.
    pub fn generate(schema: &ParsedSchema, octad: &OctadConfig) -> anyhow::Result<String> {
        assemble(schema, octad, metadata_seed)
    }
}

/// PostgreSQL-specific sidecar DDL emission (V-L2-F1, #45).
pub mod postgres {
    use super::*;

    /// PostgreSQL metadata seed: `INSERT … ON CONFLICT DO NOTHING`
    /// (SQLite's `INSERT OR IGNORE` is not valid PostgreSQL) + portable
    /// `CURRENT_TIMESTAMP`.
    pub(super) fn metadata_seed(schema: &ParsedSchema) -> String {
        if schema.tables.is_empty() {
            return String::new();
        }
        let mut ddl = String::from("-- Seed metadata from parsed schema (PostgreSQL)\n");
        for (name, ncols, pk_csv) in metadata_rows(schema) {
            ddl.push_str(&format!(
                "INSERT INTO verisimdb_metadata (table_name, column_count, pk_columns, discovered_at)\n\
                 \x20   VALUES ('{}', {}, '{}', CURRENT_TIMESTAMP)\n\
                 \x20   ON CONFLICT (table_name) DO NOTHING;\n",
                name, ncols, pk_csv,
            ));
        }
        ddl.push('\n');
        ddl
    }

    /// Generate the full PostgreSQL sidecar schema.
    pub fn generate(schema: &ParsedSchema, octad: &OctadConfig) -> anyhow::Result<String> {
        assemble(schema, octad, metadata_seed)
    }
}

/// Generate the provenance log table for the Provenance dimension.
///
/// Stores a SHA-256 hash-chained audit trail of all data modifications.
/// Each row chains to its predecessor via `previous_hash`, forming an
/// append-only, tamper-evident log (see
/// `docs/theory/provenance-threat-model.adoc`).
///
/// ADR-0010 (provenance forks are first-class): the `hash` PRIMARY KEY
/// is the duplicate guard (the preimage covers every tamper-relevant
/// field). We deliberately do **not** emit `UNIQUE(entity_id,
/// previous_hash)` (#32, superseded) — that rejects a divergent second
/// writer's legitimate history at insert time. Instead a **non-unique**
/// `idx_provenance_predecessor` makes fork *detection* O(log n), and the
/// chain tip is a *set* (`verisimdb_provenance_chain_heads`): one row
/// for a linear entity, several when it has legitimately forked. The
/// legacy single-head `verisimdb_provenance_chain_head` is kept one
/// release for non-destructive migration. Mirrors
/// `tier1::provenance::SIDECAR_DDL` (kept in sync).
fn generate_provenance_table() -> String {
    "-- Provenance: SHA-256 hash-chained audit trail (ADR-0010)\n\
     CREATE TABLE IF NOT EXISTS verisimdb_provenance_log (\n\
     \x20   hash          TEXT PRIMARY KEY,\n\
     \x20   previous_hash TEXT NOT NULL,\n\
     \x20   entity_id     TEXT NOT NULL,\n\
     \x20   table_name    TEXT NOT NULL,\n\
     \x20   operation     TEXT NOT NULL CHECK (operation IN ('insert','update','delete','transform')),  -- V-L2-J1\n\
     \x20   actor         TEXT NOT NULL,\n\
     \x20   timestamp     TEXT NOT NULL,  -- ISO 8601\n\
     \x20   before_snapshot TEXT,          -- JSON of entity state before operation\n\
     \x20   transformation  TEXT,          -- description of transformation applied\n\
     \x20   CHECK (operation IN ('insert','update','delete','transform'))\n\
     );\n\
     -- ADR-0010 #32 (superseded): NO UNIQUE(entity_id, previous_hash) —\n\
     -- a fork that cannot be written cannot be detected or audited. The\n\
     -- non-unique index below makes fork detection O(log n) instead.\n\
     CREATE INDEX IF NOT EXISTS idx_provenance_predecessor\n\
     \x20   ON verisimdb_provenance_log(entity_id, previous_hash);\n\
     CREATE INDEX IF NOT EXISTS idx_provenance_entity ON verisimdb_provenance_log(entity_id);\n\
     CREATE INDEX IF NOT EXISTS idx_provenance_table  ON verisimdb_provenance_log(table_name);\n\
     \n\
     -- ADR-0010 #31: chain-tip *set*. `append_provenance` keeps a\n\
     -- BEGIN IMMEDIATE write so racing duplicate appends on one node\n\
     -- still serialise; a linear append swaps its single tip, a\n\
     -- deliberate fork adds a tip without removing one.\n\
     CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_heads (\n\
     \x20   entity_id TEXT NOT NULL,\n\
     \x20   head_hash TEXT NOT NULL,\n\
     \x20   PRIMARY KEY (entity_id, head_hash)\n\
     );\n\
     -- Legacy single-head table: kept one release for non-destructive\n\
     -- migration (see tier1::provenance::SIDECAR_DDL). No DROP ships here.\n\
     CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_head (\n\
     \x20   entity_id  TEXT PRIMARY KEY,\n\
     \x20   head_hash  TEXT NOT NULL,\n\
     \x20   updated_at TEXT NOT NULL\n\
     );\n\n"
        .to_string()
}

/// Generate the lineage graph table for the Lineage dimension.
///
/// Stores directed edges representing data derivation relationships.
/// Together, these edges form a DAG that can be traversed to answer
/// "where did this data come from?" and "what is affected if this changes?"
fn generate_lineage_table() -> String {
    "-- Lineage: data derivation graph (DAG by intent; cycle prevention is\n\
     -- a runtime concern — see V-L1-G1 / V-L2-I2).\n\
     CREATE TABLE IF NOT EXISTS verisimdb_lineage_graph (\n\
     \x20   edge_id         TEXT PRIMARY KEY,\n\
     \x20   source_entity   TEXT NOT NULL,\n\
     \x20   source_table    TEXT NOT NULL,\n\
     \x20   target_entity   TEXT NOT NULL,\n\
     \x20   target_table    TEXT NOT NULL,\n\
     \x20   derivation_type TEXT NOT NULL\n\
     \x20       CHECK (derivation_type IN ('copy','transform','aggregate','join','filter')),  -- V-L2-J1\n\
     \x20   description     TEXT,\n\
     \x20   created_at      TEXT NOT NULL,  -- ISO 8601\n\
     \x20   -- V-L2-I1: self-edges are not derivations; rejected at DB level.\n\
     \x20   CHECK (NOT (source_entity = target_entity AND source_table = target_table))\n\
     );\n\
     CREATE INDEX IF NOT EXISTS idx_lineage_source ON verisimdb_lineage_graph(source_entity);\n\
     CREATE INDEX IF NOT EXISTS idx_lineage_target ON verisimdb_lineage_graph(target_entity);\n\n"
        .to_string()
}

/// Generate the temporal versions table for the Temporal dimension.
///
/// Stores full snapshots of entity state at each version, enabling
/// point-in-time queries and rollback. Each version records when it
/// became active (`valid_from`) and when it was superseded (`valid_to`).
fn generate_temporal_table() -> String {
    "-- Temporal: version history with point-in-time support.\n\
     -- V-L2-H1: the partial UNIQUE INDEX enforces exactly one\n\
     -- current row per (entity, table) — \"only one version is\n\
     -- valid right now\" was an application-layer invariant before;\n\
     -- now it's structural.\n\
     -- V-L2-J1: operation is a closed set.\n\
     -- V-L2-H2: valid_to (if set) must not predate valid_from.\n\
     CREATE TABLE IF NOT EXISTS verisimdb_temporal_versions (\n\
     \x20   entity_id  TEXT NOT NULL,\n\
     \x20   table_name TEXT NOT NULL,\n\
     \x20   version    INTEGER NOT NULL CHECK (version >= 1),\n\
     \x20   valid_from TEXT NOT NULL,   -- ISO 8601\n\
     \x20   valid_to   TEXT,            -- ISO 8601, NULL if current\n\
     \x20   snapshot   TEXT NOT NULL,   -- JSON serialisation of entity state\n\
     \x20   operation  TEXT NOT NULL CHECK (operation IN ('insert','update','rollback')),\n\
     \x20   PRIMARY KEY (entity_id, table_name, version),\n\
     \x20   CHECK (valid_to IS NULL OR valid_to >= valid_from)\n\
     );\n\
     CREATE UNIQUE INDEX IF NOT EXISTS ux_temporal_current\n\
     \x20   ON verisimdb_temporal_versions(entity_id, table_name)\n\
     \x20   WHERE valid_to IS NULL;\n\n"
        .to_string()
}

/// Generate the access policies table for the Access Control dimension.
///
/// Stores row-level and column-level access control policies that are
/// evaluated at query time to filter and redact data based on the
/// requesting principal's identity and roles.
fn generate_access_policy_table() -> String {
    "-- Access Control: row/column-level access policies.\n\
     -- V-L2-J1: access_level is a closed set.\n\
     CREATE TABLE IF NOT EXISTS verisimdb_access_policies (\n\
     \x20   policy_id     TEXT PRIMARY KEY,\n\
     \x20   target_table  TEXT NOT NULL,\n\
     \x20   target_column TEXT,            -- NULL means whole-row policy\n\
     \x20   principal     TEXT NOT NULL,   -- user, role, or group identifier\n\
     \x20   access_level  TEXT NOT NULL\n\
     \x20       CHECK (access_level IN ('read','write','admin','deny')),\n\
     \x20   condition     TEXT,            -- SQL-like filter condition (V-L1-H1)\n\
     \x20   created_at    TEXT NOT NULL,   -- ISO 8601\n\
     \x20   active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1))\n\
     );\n\
     CREATE INDEX IF NOT EXISTS idx_access_table ON verisimdb_access_policies(target_table);\n\
     CREATE INDEX IF NOT EXISTS idx_access_principal ON verisimdb_access_policies(principal);\n\n"
        .to_string()
}

/// Generate the simulation branches table for the Simulation dimension.
///
/// Stores branched copies of data for what-if analysis. Each branch
/// is isolated from the main data until explicitly merged.
fn generate_simulation_table() -> String {
    "-- Simulation: what-if branching and sandbox queries.\n\
     -- V-L2-J1: status is a closed set; parent_branch is a self-FK\n\
     -- (was previously declared but un-enforced).\n\
     CREATE TABLE IF NOT EXISTS verisimdb_simulation_branches (\n\
     \x20   branch_id    TEXT PRIMARY KEY,\n\
     \x20   parent_branch TEXT REFERENCES verisimdb_simulation_branches(branch_id),  -- NULL for root\n\
     \x20   name         TEXT NOT NULL,\n\
     \x20   description  TEXT,\n\
     \x20   created_at   TEXT NOT NULL,   -- ISO 8601\n\
     \x20   merged_at    TEXT,            -- ISO 8601, NULL if not merged\n\
     \x20   status       TEXT NOT NULL DEFAULT 'active'\n\
     \x20       CHECK (status IN ('active','merged','abandoned'))\n\
     );\n\n\
     CREATE TABLE IF NOT EXISTS verisimdb_simulation_deltas (\n\
     \x20   delta_id    TEXT PRIMARY KEY,\n\
     \x20   branch_id   TEXT NOT NULL REFERENCES verisimdb_simulation_branches(branch_id),\n\
     \x20   entity_id   TEXT NOT NULL,\n\
     \x20   table_name  TEXT NOT NULL,\n\
     \x20   operation   TEXT NOT NULL\n\
     \x20       CHECK (operation IN ('insert','update','delete')),  -- V-L2-J1\n\
     \x20   delta_data  TEXT NOT NULL,    -- JSON of the change\n\
     \x20   created_at  TEXT NOT NULL     -- ISO 8601\n\
     );\n\
     CREATE INDEX IF NOT EXISTS idx_sim_branch ON verisimdb_simulation_deltas(branch_id);\n\n"
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::parser::{ColumnDef, TableDef};

    /// Helper: create a minimal schema for testing.
    fn test_schema() -> ParsedSchema {
        ParsedSchema {
            tables: vec![TableDef {
                name: "posts".to_string(),
                columns: vec![
                    ColumnDef {
                        name: "id".to_string(),
                        sql_type: "INTEGER".to_string(),
                        is_primary_key: true,
                        is_not_null: true,
                    },
                    ColumnDef {
                        name: "title".to_string(),
                        sql_type: "TEXT".to_string(),
                        is_primary_key: false,
                        is_not_null: true,
                    },
                ],
            }],
            source: None,
        }
    }

    #[test]
    fn test_generate_all_dimensions() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: true,
            enable_lineage: true,
            enable_temporal: true,
            enable_access_control: true,
            enable_constraints: true,
            enable_simulation: true,
        };
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");

        assert!(ddl.contains("verisimdb_metadata"));
        assert!(ddl.contains("verisimdb_provenance_log"));
        assert!(ddl.contains("verisimdb_lineage_graph"));
        assert!(ddl.contains("verisimdb_temporal_versions"));
        assert!(ddl.contains("verisimdb_access_policies"));
        assert!(ddl.contains("verisimdb_simulation_branches"));
    }

    /// All four enum-shape columns must be CHECK-constrained, and
    /// simulation_branches.parent_branch must be a self-referencing FK
    /// (closes #43).
    #[test]
    fn test_overlay_has_enum_checks_and_fk() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: true,
            enable_lineage: true,
            enable_temporal: true,
            enable_access_control: true,
            enable_constraints: true,
            enable_simulation: true,
        };
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");

        // Self-referencing FK on parent_branch.
        assert!(
            ddl.contains("parent_branch TEXT REFERENCES verisimdb_simulation_branches(branch_id)"),
            "simulation_branches.parent_branch is missing the self-referencing FK"
        );
        // Enum CHECKs.
        assert!(
            ddl.contains("CHECK (status IN ('active','merged','abandoned'))"),
            "simulation_branches.status enum CHECK missing"
        );
        assert!(
            ddl.contains("CHECK (operation IN ('insert','update','delete','transform'))"),
            "provenance_log.operation enum CHECK missing"
        );
        assert!(
            ddl.contains("CHECK (access_level IN ('read','write','admin','deny'))"),
            "access_policies.access_level enum CHECK missing"
        );
        assert!(
            ddl.contains(
                "CHECK (derivation_type IN ('copy','transform','aggregate','join','filter'))"
            ),
            "lineage_graph.derivation_type enum CHECK missing"
        );
    }

    /// The "current version" partial index must be UNIQUE and the
    /// `valid_to >= valid_from` CHECK must be present (closes #41).
    /// Two concurrent writers must not be able to leave two rows with
    /// `valid_to IS NULL` for the same `(entity_id, table_name)`.
    #[test]
    fn test_temporal_table_has_unique_partial_index_and_valid_to_check() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: true,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");
        assert!(ddl.contains("verisimdb_temporal_versions"));
        assert!(
            ddl.contains("CREATE UNIQUE INDEX IF NOT EXISTS ux_temporal_current"),
            "temporal current-version index must be UNIQUE"
        );
        assert!(
            ddl.contains("ON verisimdb_temporal_versions(entity_id, table_name)")
                && ddl.contains("WHERE valid_to IS NULL"),
            "temporal current-version index must be partial on valid_to IS NULL"
        );
        assert!(
            ddl.contains("CHECK (valid_to IS NULL OR valid_to >= valid_from)"),
            "temporal valid_to ordering CHECK missing"
        );
    }

    /// Lineage edges must refuse self-loops at the storage layer
    /// (closes #42). The DAG claim in the README would be unenforced
    /// without this check.
    #[test]
    fn test_lineage_table_has_self_reference_check() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: true,
            enable_temporal: false,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");
        assert!(ddl.contains("verisimdb_lineage_graph"));
        // The exact CHECK clause must be present in the emitted DDL.
        assert!(
            ddl.contains(
                "CHECK (NOT (source_entity = target_entity AND source_table = target_table))"
            ),
            "lineage table is missing the self-reference CHECK constraint"
        );
    }

    #[test]
    fn test_generate_minimal_dimensions() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: false,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");

        // Metadata is always generated.
        assert!(ddl.contains("verisimdb_metadata"));
        // Nothing else should be generated.
        assert!(!ddl.contains("verisimdb_provenance_log"));
        assert!(!ddl.contains("verisimdb_lineage_graph"));
        assert!(!ddl.contains("verisimdb_temporal_versions"));
        assert!(!ddl.contains("verisimdb_access_policies"));
        assert!(!ddl.contains("verisimdb_simulation_branches"));
    }

    #[test]
    fn test_metadata_seeds_table_info() {
        let schema = test_schema();
        let octad = OctadConfig::default();
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("test schema must validate");

        assert!(ddl.contains("INSERT OR IGNORE INTO verisimdb_metadata"));
        assert!(ddl.contains("'posts'"));
        assert!(ddl.contains("'id'"));
    }

    #[test]
    fn test_provenance_table_has_hash_chain() {
        let ddl = generate_provenance_table();
        assert!(ddl.contains("hash"));
        assert!(ddl.contains("previous_hash"));
        assert!(ddl.contains("entity_id"));
        assert!(ddl.contains("actor"));
    }

    /// ADR-0010 (#32 superseded): forks are first-class. The fork guard
    /// is the `hash` PRIMARY KEY (duplicate-rejection); there must be
    /// NO `UNIQUE(entity_id, previous_hash)` (it would discard a
    /// divergent writer's legitimate history). A *non-unique*
    /// predecessor index provides O(log n) fork detection instead.
    #[test]
    fn test_provenance_table_fork_detection_index_is_not_unique() {
        let ddl = generate_provenance_table();
        assert!(
            !ddl.contains("ux_provenance_chain"),
            "the superseded UNIQUE(entity_id, previous_hash) must not be emitted"
        );
        assert!(
            !ddl.contains("CREATE UNIQUE INDEX IF NOT EXISTS ux_provenance"),
            "no unique provenance-chain index (ADR-0010)"
        );
        assert!(
            ddl.contains("idx_provenance_predecessor"),
            "non-unique fork-detection index must be present"
        );
        assert!(ddl.contains("(entity_id, previous_hash)"));
    }

    /// ADR-0010 #31: the chain tip is a *set* (multi-head); the legacy
    /// single-head table is retained one release for migration.
    #[test]
    fn test_provenance_table_has_multihead_and_legacy_head() {
        let ddl = generate_provenance_table();
        assert!(
            ddl.contains("verisimdb_provenance_chain_heads"),
            "multi-head set table must exist"
        );
        assert!(
            ddl.contains("verisimdb_provenance_chain_head ("),
            "legacy single-head table retained for migration"
        );
        assert!(ddl.contains("head_hash"));
        assert!(
            ddl.contains("PRIMARY KEY (entity_id, head_hash)"),
            "multi-head table keyed by (entity_id, head_hash)"
        );
    }

    #[test]
    fn test_temporal_table_has_versioning() {
        let ddl = generate_temporal_table();
        assert!(ddl.contains("version"));
        assert!(ddl.contains("valid_from"));
        assert!(ddl.contains("valid_to"));
        assert!(ddl.contains("snapshot"));
    }

    /// V-L2-H1: the partial UNIQUE INDEX enforces exactly-one-current.
    #[test]
    fn test_temporal_table_has_partial_unique_index() {
        let ddl = generate_temporal_table();
        assert!(ddl.contains("UNIQUE INDEX"));
        assert!(ddl.contains("ux_temporal_current"));
        assert!(ddl.contains("WHERE valid_to IS NULL"));
    }

    /// V-L2-H2: valid_to must not predate valid_from.
    #[test]
    fn test_temporal_table_has_valid_to_check() {
        let ddl = generate_temporal_table();
        assert!(ddl.contains("valid_to IS NULL OR valid_to >= valid_from"));
    }

    /// V-L2-I1: lineage self-edges are forbidden by CHECK.
    #[test]
    fn test_lineage_table_forbids_self_edges() {
        let ddl = generate_lineage_table();
        assert!(ddl.contains("NOT (source_entity = target_entity"));
    }

    /// V-L2-J1: simulation status is a closed set; parent_branch FK exists.
    #[test]
    fn test_simulation_table_constraints() {
        let ddl = generate_simulation_table();
        assert!(ddl.contains("REFERENCES verisimdb_simulation_branches(branch_id)"));
        assert!(ddl.contains("status IN ('active','merged','abandoned')"));
        assert!(ddl.contains("operation IN ('insert','update','delete')"));
    }

    /// V-L2-J1: provenance, lineage, access enum CHECKs.
    #[test]
    fn test_enum_checks() {
        let prov = generate_provenance_table();
        assert!(prov.contains("operation IN ('insert','update','delete','transform')"));

        let lin = generate_lineage_table();
        assert!(
            lin.contains("derivation_type IN ('copy','transform','aggregate','join','filter')")
        );

        let acc = generate_access_policy_table();
        assert!(acc.contains("access_level IN ('read','write','admin','deny')"));
    }

    /// V-L2-G1: identifier validator accepts safe names, rejects everything
    /// outside `^[A-Za-z_][A-Za-z0-9_]*$`. This is the codegen-side guard
    /// against SQL injection via table/column names.
    #[test]
    fn test_validate_identifier_accepts_safe() {
        for ok in &["posts", "Posts", "_x", "x_1", "Post_2026"] {
            assert!(
                validate_identifier(ok).is_ok(),
                "{:?} should be accepted",
                ok
            );
        }
    }

    #[test]
    fn test_validate_identifier_rejects_unsafe() {
        let attacks = [
            "",                         // empty
            "1posts",                   // leading digit
            "po sts",                   // space
            "posts;",                   // statement terminator
            "posts'); DROP TABLE x;--", // classic injection
            "posts\"",                  // quote
            "posts`",                   // backtick
            "posts/*",                  // comment open
            "schema.table",             // dotted
            "ünicode",                  // non-ASCII
        ];
        for attack in &attacks {
            assert!(
                validate_identifier(attack).is_err(),
                "{:?} should be rejected",
                attack
            );
        }
    }

    // --- #45 acceptance: per-dialect DDL + storage mapping ---

    #[test]
    fn test_sqlite_dialect_seed_and_portable_timestamp() {
        let schema = test_schema();
        let octad = OctadConfig::default();
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite)
            .expect("sqlite ddl");
        assert!(ddl.contains("INSERT OR IGNORE INTO verisimdb_metadata"));
        assert!(
            ddl.contains("CURRENT_TIMESTAMP"),
            "portable timestamp must replace datetime(now)"
        );
        assert!(
            !ddl.contains("datetime('now')"),
            "the SQLite-only datetime('now') must be gone (V-L2-F1)"
        );
        assert!(ddl.contains("'posts'") && ddl.contains("verisimdb_provenance_log"));
    }

    #[test]
    fn test_postgres_dialect_uses_on_conflict_not_or_ignore() {
        let schema = test_schema();
        let octad = OctadConfig::default();
        let ddl = generate_sidecar_schema(&schema, &octad, SqlDialect::Postgres)
            .expect("postgres ddl");
        assert!(
            ddl.contains("ON CONFLICT (table_name) DO NOTHING"),
            "postgres metadata upsert must use ON CONFLICT"
        );
        assert!(
            !ddl.contains("INSERT OR IGNORE"),
            "INSERT OR IGNORE is not valid PostgreSQL"
        );
        assert!(ddl.contains("CURRENT_TIMESTAMP") && !ddl.contains("datetime('now')"));
        assert!(ddl.contains("verisimdb_metadata") && ddl.contains("'posts'"));
    }

    #[test]
    fn test_both_dialects_share_the_octad_table_bodies() {
        let schema = test_schema();
        let octad = OctadConfig::default();
        let s = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite).unwrap();
        let p = generate_sidecar_schema(&schema, &octad, SqlDialect::Postgres).unwrap();
        for table in [
            "verisimdb_provenance_log",
            "verisimdb_lineage_graph",
            "verisimdb_temporal_versions",
            "verisimdb_access_policies",
        ] {
            assert!(s.contains(table), "sqlite missing {table}");
            assert!(p.contains(table), "postgres missing {table}");
        }
    }

    #[test]
    fn test_empty_schema_emits_no_seed_in_either_dialect() {
        let schema = ParsedSchema {
            tables: vec![],
            source: None,
        };
        let octad = OctadConfig::default();
        let s = generate_sidecar_schema(&schema, &octad, SqlDialect::Sqlite).unwrap();
        let p = generate_sidecar_schema(&schema, &octad, SqlDialect::Postgres).unwrap();
        assert!(!s.contains("INSERT OR IGNORE") && s.contains("verisimdb_metadata"));
        assert!(!s.contains("Seed metadata from parsed schema"));
        assert!(!p.contains("ON CONFLICT") && p.contains("verisimdb_metadata"));
        assert!(!p.contains("Seed metadata from parsed schema"));
    }

    #[test]
    fn test_storage_to_dialect_mapping() {
        assert_eq!(
            SqlDialect::from_storage("sqlite").unwrap(),
            SqlDialect::Sqlite
        );
        assert_eq!(
            SqlDialect::from_storage("postgres").unwrap(),
            SqlDialect::Postgres
        );
        assert_eq!(
            SqlDialect::from_storage("PostgreSQL").unwrap(),
            SqlDialect::Postgres
        );
        let json_err = SqlDialect::from_storage("json").unwrap_err().to_string();
        assert!(
            json_err.contains("not implemented") && json_err.contains("#112"),
            "json must be rejected with the #112 pointer, got: {json_err}"
        );
        assert!(SqlDialect::from_storage("mariadb").is_err());
    }
}
