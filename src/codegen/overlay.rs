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
// Overlay generation
// ---------------------------------------------------------------------------

/// Generate the complete sidecar schema DDL for all enabled octad dimensions.
///
/// The generated SQL is SQLite-compatible (the default sidecar backend).
/// Each table is created with `IF NOT EXISTS` so the schema can be applied
/// idempotently.
///
/// # Arguments
/// * `schema` - The parsed schema of the target database (used to reference
///   table names in provenance and temporal tracking).
/// * `octad` - The octad configuration from the manifest, controlling which
///   dimension tables to generate.
///
/// # Returns
/// A string containing the complete DDL for the sidecar database.
pub fn generate_sidecar_schema(schema: &ParsedSchema, octad: &OctadConfig) -> String {
    let mut ddl = String::new();

    ddl.push_str("-- SPDX-License-Identifier: PMPL-1.0-or-later\n");
    ddl.push_str("-- VeriSimiser sidecar schema (auto-generated)\n");
    ddl.push_str("-- Do not edit manually; regenerate with `verisimiser init`.\n\n");

    // Metadata table: tracks which target tables are being augmented.
    ddl.push_str(&generate_metadata_table(schema));

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

    ddl
}

/// Generate the metadata table that tracks which target tables are augmented.
///
/// This table is always created regardless of octad configuration, because
/// the Data and Metadata dimensions are always active.
fn generate_metadata_table(schema: &ParsedSchema) -> String {
    let mut ddl = String::new();

    ddl.push_str("-- Metadata: tracks augmented target tables\n");
    ddl.push_str(
        "CREATE TABLE IF NOT EXISTS verisimdb_metadata (\n\
         \x20   table_name   TEXT PRIMARY KEY,\n\
         \x20   column_count INTEGER NOT NULL,\n\
         \x20   pk_columns   TEXT NOT NULL,   -- comma-separated list of PK column names\n\
         \x20   discovered_at TEXT NOT NULL    -- ISO 8601 timestamp\n\
         );\n\n",
    );

    // Generate INSERT statements for each discovered table.
    if !schema.tables.is_empty() {
        ddl.push_str("-- Seed metadata from parsed schema\n");
        for table in &schema.tables {
            let pk_cols: Vec<&str> = table
                .columns
                .iter()
                .filter(|c| c.is_primary_key)
                .map(|c| c.name.as_str())
                .collect();
            let pk_str = pk_cols.join(",");
            ddl.push_str(&format!(
                "INSERT OR IGNORE INTO verisimdb_metadata (table_name, column_count, pk_columns, discovered_at)\n\
                 \x20   VALUES ('{}', {}, '{}', datetime('now'));\n",
                table.name,
                table.columns.len(),
                pk_str,
            ));
        }
        ddl.push('\n');
    }

    ddl
}

/// Generate the provenance log table for the Provenance dimension.
///
/// Stores a SHA-256 hash-chained audit trail of all data modifications.
/// Each row chains to its predecessor via `previous_hash`, forming an
/// append-only, tamper-evident log.
fn generate_provenance_table() -> String {
    "-- Provenance: SHA-256 hash-chained audit trail\n\
     CREATE TABLE IF NOT EXISTS verisimdb_provenance_log (\n\
     \x20   hash          TEXT PRIMARY KEY,\n\
     \x20   previous_hash TEXT NOT NULL,\n\
     \x20   entity_id     TEXT NOT NULL,\n\
     \x20   table_name    TEXT NOT NULL,\n\
     \x20   operation     TEXT NOT NULL,  -- insert, update, delete, transform\n\
     \x20   actor         TEXT NOT NULL,\n\
     \x20   timestamp     TEXT NOT NULL,  -- ISO 8601\n\
     \x20   before_snapshot TEXT,          -- JSON of entity state before operation\n\
     \x20   transformation  TEXT           -- description of transformation applied\n\
     );\n\
     CREATE INDEX IF NOT EXISTS idx_provenance_entity ON verisimdb_provenance_log(entity_id);\n\
     CREATE INDEX IF NOT EXISTS idx_provenance_table  ON verisimdb_provenance_log(table_name);\n\n"
        .to_string()
}

/// Generate the lineage graph table for the Lineage dimension.
///
/// Stores directed edges representing data derivation relationships.
/// Together, these edges form a DAG that can be traversed to answer
/// "where did this data come from?" and "what is affected if this changes?"
fn generate_lineage_table() -> String {
    "-- Lineage: data derivation DAG\n\
     CREATE TABLE IF NOT EXISTS verisimdb_lineage_graph (\n\
     \x20   edge_id         TEXT PRIMARY KEY,\n\
     \x20   source_entity   TEXT NOT NULL,\n\
     \x20   source_table    TEXT NOT NULL,\n\
     \x20   target_entity   TEXT NOT NULL,\n\
     \x20   target_table    TEXT NOT NULL,\n\
     \x20   derivation_type TEXT NOT NULL,  -- copy, transform, aggregate, join, filter\n\
     \x20   description     TEXT,\n\
     \x20   created_at      TEXT NOT NULL   -- ISO 8601\n\
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
    "-- Temporal: version history with point-in-time support\n\
     CREATE TABLE IF NOT EXISTS verisimdb_temporal_versions (\n\
     \x20   entity_id  TEXT NOT NULL,\n\
     \x20   table_name TEXT NOT NULL,\n\
     \x20   version    INTEGER NOT NULL,\n\
     \x20   valid_from TEXT NOT NULL,   -- ISO 8601\n\
     \x20   valid_to   TEXT,            -- ISO 8601, NULL if current\n\
     \x20   snapshot   TEXT NOT NULL,   -- JSON serialisation of entity state\n\
     \x20   operation  TEXT NOT NULL,   -- insert, update, rollback\n\
     \x20   PRIMARY KEY (entity_id, table_name, version)\n\
     );\n\
     CREATE INDEX IF NOT EXISTS idx_temporal_current ON verisimdb_temporal_versions(entity_id, table_name) WHERE valid_to IS NULL;\n\n"
        .to_string()
}

/// Generate the access policies table for the Access Control dimension.
///
/// Stores row-level and column-level access control policies that are
/// evaluated at query time to filter and redact data based on the
/// requesting principal's identity and roles.
fn generate_access_policy_table() -> String {
    "-- Access Control: row/column-level access policies\n\
     CREATE TABLE IF NOT EXISTS verisimdb_access_policies (\n\
     \x20   policy_id     TEXT PRIMARY KEY,\n\
     \x20   target_table  TEXT NOT NULL,\n\
     \x20   target_column TEXT,            -- NULL means whole-row policy\n\
     \x20   principal     TEXT NOT NULL,   -- user, role, or group identifier\n\
     \x20   access_level  TEXT NOT NULL,   -- read, write, admin, deny\n\
     \x20   condition     TEXT,            -- SQL-like filter condition\n\
     \x20   created_at    TEXT NOT NULL,   -- ISO 8601\n\
     \x20   active        INTEGER NOT NULL DEFAULT 1\n\
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
    "-- Simulation: what-if branching and sandbox queries\n\
     CREATE TABLE IF NOT EXISTS verisimdb_simulation_branches (\n\
     \x20   branch_id    TEXT PRIMARY KEY,\n\
     \x20   parent_branch TEXT,           -- NULL for root branch\n\
     \x20   name         TEXT NOT NULL,\n\
     \x20   description  TEXT,\n\
     \x20   created_at   TEXT NOT NULL,   -- ISO 8601\n\
     \x20   merged_at    TEXT,            -- ISO 8601, NULL if not merged\n\
     \x20   status       TEXT NOT NULL DEFAULT 'active'  -- active, merged, abandoned\n\
     );\n\n\
     CREATE TABLE IF NOT EXISTS verisimdb_simulation_deltas (\n\
     \x20   delta_id    TEXT PRIMARY KEY,\n\
     \x20   branch_id   TEXT NOT NULL REFERENCES verisimdb_simulation_branches(branch_id),\n\
     \x20   entity_id   TEXT NOT NULL,\n\
     \x20   table_name  TEXT NOT NULL,\n\
     \x20   operation   TEXT NOT NULL,    -- insert, update, delete\n\
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
            enable_simulation: true,
        };
        let ddl = generate_sidecar_schema(&schema, &octad);

        assert!(ddl.contains("verisimdb_metadata"));
        assert!(ddl.contains("verisimdb_provenance_log"));
        assert!(ddl.contains("verisimdb_lineage_graph"));
        assert!(ddl.contains("verisimdb_temporal_versions"));
        assert!(ddl.contains("verisimdb_access_policies"));
        assert!(ddl.contains("verisimdb_simulation_branches"));
    }

    #[test]
    fn test_generate_minimal_dimensions() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: false,
            enable_access_control: false,
            enable_simulation: false,
        };
        let ddl = generate_sidecar_schema(&schema, &octad);

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
        let ddl = generate_sidecar_schema(&schema, &octad);

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

    #[test]
    fn test_temporal_table_has_versioning() {
        let ddl = generate_temporal_table();
        assert!(ddl.contains("version"));
        assert!(ddl.contains("valid_from"));
        assert!(ddl.contains("valid_to"));
        assert!(ddl.contains("snapshot"));
    }
}
