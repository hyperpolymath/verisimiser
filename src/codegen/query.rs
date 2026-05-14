// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Query interceptor generator for VeriSimiser.
//
// Generates SQL views and helper queries that enrich native database queries
// with octad dimension data from the sidecar. The interceptor sits between
// the application and the database, transparently adding provenance, lineage,
// temporal, and access control information to query results.
//
// Design principle: the interceptor NEVER modifies the target database.
// All enrichment happens via sidecar JOINs or post-query augmentation.

use crate::abi::DatabaseBackend;
use crate::codegen::parser::{ParsedSchema, TableDef};
use crate::manifest::OctadConfig;

// ---------------------------------------------------------------------------
// Query interceptor generation
// ---------------------------------------------------------------------------

/// A generated query interceptor for a single table.
///
/// Contains the SQL views and helper queries needed to enrich queries
/// against this table with octad dimension data.
#[derive(Debug, Clone)]
pub struct TableInterceptor {
    /// The target table name this interceptor wraps.
    pub table_name: String,
    /// SQL to create a provenance-enriched view (if provenance is enabled).
    pub provenance_view: Option<String>,
    /// SQL to create a temporally-aware view (if temporal is enabled).
    pub temporal_view: Option<String>,
    /// SQL to query the lineage graph for this table (if lineage is enabled).
    pub lineage_query: Option<String>,
    /// SQL to apply access control filtering (if access control is enabled).
    pub access_filter: Option<String>,
}

/// Generate query interceptors for all tables in the parsed schema.
///
/// For each table, generates SQL views that join the target table with
/// sidecar dimension tables, allowing transparent octad enrichment.
///
/// # Arguments
/// * `schema` - The parsed schema of the target database.
/// * `octad` - The octad configuration controlling which dimensions to enrich.
/// * `backend` - The target database backend (affects SQL dialect).
///
/// # Returns
/// A vector of `TableInterceptor` structs, one per table.
pub fn generate_interceptors(
    schema: &ParsedSchema,
    octad: &OctadConfig,
    backend: DatabaseBackend,
) -> Vec<TableInterceptor> {
    schema
        .tables
        .iter()
        .map(|table| generate_table_interceptor(table, octad, backend))
        .collect()
}

/// Generate a query interceptor for a single table.
fn generate_table_interceptor(
    table: &TableDef,
    octad: &OctadConfig,
    backend: DatabaseBackend,
) -> TableInterceptor {
    // Determine the primary key expression for JOINs.
    let pk_columns: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.as_str())
        .collect();

    // Build a composite entity_id expression from PK columns.
    let entity_id_expr = build_entity_id_expr(&pk_columns, &table.name, backend);

    let provenance_view = if octad.enable_provenance {
        Some(generate_provenance_view(table, &entity_id_expr, backend))
    } else {
        None
    };

    let temporal_view = if octad.enable_temporal {
        Some(generate_temporal_view(table, &entity_id_expr, backend))
    } else {
        None
    };

    let lineage_query = if octad.enable_lineage {
        Some(generate_lineage_query(&table.name))
    } else {
        None
    };

    let access_filter = if octad.enable_access_control {
        Some(generate_access_filter(&table.name, backend))
    } else {
        None
    };

    TableInterceptor {
        table_name: table.name.clone(),
        provenance_view,
        temporal_view,
        lineage_query,
        access_filter,
    }
}

/// Build a SQL expression that computes a composite entity_id from PK columns.
///
/// For single-column PKs: just CAST the column to TEXT.
/// For composite PKs: concatenate with '::' separator.
/// If no PK is defined, use ROWID (SQLite) or ctid (PostgreSQL).
fn build_entity_id_expr(pk_columns: &[&str], table_name: &str, backend: DatabaseBackend) -> String {
    if pk_columns.is_empty() {
        // No PK defined — fall back to internal row identifier.
        match backend {
            DatabaseBackend::SQLite => format!("{}.rowid", table_name),
            DatabaseBackend::PostgreSQL => format!("{}.ctid::text", table_name),
            DatabaseBackend::MongoDB => "CAST(_id AS TEXT)".to_string(),
        }
    } else if pk_columns.len() == 1 {
        format!("CAST({}.{} AS TEXT)", table_name, pk_columns[0])
    } else {
        // Composite PK: concatenate columns with '::' separator.
        let parts: Vec<String> = pk_columns
            .iter()
            .map(|col| format!("CAST({}.{} AS TEXT)", table_name, col))
            .collect();
        match backend {
            DatabaseBackend::PostgreSQL => parts.join(" || '::' || "),
            DatabaseBackend::SQLite => parts.join(" || '::' || "),
            DatabaseBackend::MongoDB => parts.join(" + '::' + "),
        }
    }
}

/// Generate a SQL view that enriches a table's rows with their latest
/// provenance information (last operation, actor, and timestamp).
fn generate_provenance_view(
    table: &TableDef,
    entity_id_expr: &str,
    _backend: DatabaseBackend,
) -> String {
    let table_name = &table.name;
    let column_list: Vec<String> = table
        .columns
        .iter()
        .map(|c| format!("    {}.{}", table_name, c.name))
        .collect();

    let comment = format!(
        "-- Provenance-enriched view for '{}'\n\
         -- Joins each row with its latest provenance entry from the sidecar.\n",
        table_name
    );

    // Use a subquery to get the latest provenance entry per entity.
    format!(
        "{comment}\
         CREATE VIEW IF NOT EXISTS verisimdb_{table_name}_with_provenance AS\n\
         SELECT\n\
         {columns},\n\
         \x20   prov.operation   AS _verisimdb_last_operation,\n\
         \x20   prov.actor       AS _verisimdb_last_actor,\n\
         \x20   prov.timestamp   AS _verisimdb_last_modified,\n\
         \x20   prov.hash        AS _verisimdb_provenance_hash\n\
         FROM {table_name}\n\
         LEFT JOIN (\n\
         \x20   SELECT entity_id, operation, actor, timestamp, hash\n\
         \x20   FROM verisimdb_provenance_log\n\
         \x20   WHERE table_name = '{table_name}'\n\
         \x20   AND timestamp = (\n\
         \x20       SELECT MAX(p2.timestamp)\n\
         \x20       FROM verisimdb_provenance_log p2\n\
         \x20       WHERE p2.entity_id = verisimdb_provenance_log.entity_id\n\
         \x20       AND p2.table_name = '{table_name}'\n\
         \x20   )\n\
         ) prov ON prov.entity_id = ({entity_id_expr});\n\n",
        columns = column_list.join(",\n"),
    )
}

/// Generate a SQL view for temporal point-in-time queries.
///
/// This view shows the current version of each entity alongside its
/// version number and valid_from timestamp.
fn generate_temporal_view(
    table: &TableDef,
    entity_id_expr: &str,
    _backend: DatabaseBackend,
) -> String {
    let table_name = &table.name;
    let column_list: Vec<String> = table
        .columns
        .iter()
        .map(|c| format!("    {}.{}", table_name, c.name))
        .collect();

    format!(
        "-- Temporal-enriched view for '{}'\n\
         -- Joins each row with its current version metadata.\n\
         CREATE VIEW IF NOT EXISTS verisimdb_{table_name}_with_temporal AS\n\
         SELECT\n\
         {columns},\n\
         \x20   tv.version    AS _verisimdb_version,\n\
         \x20   tv.valid_from AS _verisimdb_valid_from,\n\
         \x20   tv.operation  AS _verisimdb_version_operation\n\
         FROM {table_name}\n\
         LEFT JOIN verisimdb_temporal_versions tv\n\
         \x20   ON tv.entity_id = ({entity_id_expr})\n\
         \x20   AND tv.table_name = '{table_name}'\n\
         \x20   AND tv.valid_to IS NULL;\n\n",
        table_name,
        columns = column_list.join(",\n"),
    )
}

/// Generate a parameterised query template for lineage graph traversal.
///
/// Returns upstream and downstream traversal queries for entities in the
/// given table.
fn generate_lineage_query(table_name: &str) -> String {
    format!(
        "-- Lineage queries for '{}'\n\
         \n\
         -- Upstream: what data was this entity derived from?\n\
         -- SELECT * FROM verisimdb_lineage_graph\n\
         -- WHERE target_entity = :entity_id AND target_table = '{table_name}';\n\
         \n\
         -- Downstream: what entities depend on this entity?\n\
         -- SELECT * FROM verisimdb_lineage_graph\n\
         -- WHERE source_entity = :entity_id AND source_table = '{table_name}';\n\n",
        table_name,
    )
}

/// Generate a parameterised access control filter for a table.
///
/// This generates a WHERE clause fragment that can be injected into
/// queries to enforce row-level access control based on the requesting
/// principal.
fn generate_access_filter(table_name: &str, _backend: DatabaseBackend) -> String {
    format!(
        "-- Access control filter for '{}'\n\
         -- Apply this as a WHERE clause addition to enforce row-level security.\n\
         --\n\
         -- Example usage (parameterised):\n\
         -- SELECT * FROM {table_name}\n\
         -- WHERE ... AND EXISTS (\n\
         --     SELECT 1 FROM verisimdb_access_policies\n\
         --     WHERE target_table = '{table_name}'\n\
         --     AND principal = :current_principal\n\
         --     AND access_level IN ('read', 'admin')\n\
         --     AND active = 1\n\
         --     AND (condition IS NULL OR :row_matches_condition)\n\
         -- );\n\n",
        table_name,
    )
}

/// Render all interceptors as a single SQL string.
///
/// This is the main entry point for writing the interceptor output to a file.
pub fn render_interceptors(interceptors: &[TableInterceptor]) -> String {
    let mut output = String::new();
    output.push_str("-- SPDX-License-Identifier: PMPL-1.0-or-later\n");
    output.push_str("-- VeriSimiser query interceptors (auto-generated)\n\n");

    for interceptor in interceptors {
        output.push_str(&format!(
            "-- ==========================================================\n\
             -- Table: {}\n\
             -- ==========================================================\n\n",
            interceptor.table_name
        ));

        if let Some(ref view) = interceptor.provenance_view {
            output.push_str(view);
        }
        if let Some(ref view) = interceptor.temporal_view {
            output.push_str(view);
        }
        if let Some(ref query) = interceptor.lineage_query {
            output.push_str(query);
        }
        if let Some(ref filter) = interceptor.access_filter {
            output.push_str(filter);
        }
    }

    output
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::parser::{ColumnDef, ParsedSchema, TableDef};

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
                    ColumnDef {
                        name: "body".to_string(),
                        sql_type: "TEXT".to_string(),
                        is_primary_key: false,
                        is_not_null: false,
                    },
                ],
            }],
            source: None,
        }
    }

    #[test]
    fn test_generate_interceptors_all_dimensions() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: true,
            enable_lineage: true,
            enable_temporal: true,
            enable_access_control: true,
            enable_constraints: true,
            enable_simulation: false,
        };
        let interceptors = generate_interceptors(&schema, &octad, DatabaseBackend::SQLite);

        assert_eq!(interceptors.len(), 1);
        let interceptor = &interceptors[0];
        assert_eq!(interceptor.table_name, "posts");
        assert!(interceptor.provenance_view.is_some());
        assert!(interceptor.temporal_view.is_some());
        assert!(interceptor.lineage_query.is_some());
        assert!(interceptor.access_filter.is_some());
    }

    #[test]
    fn test_generate_interceptors_minimal() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: false,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let interceptors = generate_interceptors(&schema, &octad, DatabaseBackend::SQLite);

        assert_eq!(interceptors.len(), 1);
        let interceptor = &interceptors[0];
        assert!(interceptor.provenance_view.is_none());
        assert!(interceptor.temporal_view.is_none());
        assert!(interceptor.lineage_query.is_none());
        assert!(interceptor.access_filter.is_none());
    }

    #[test]
    fn test_provenance_view_references_table() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: true,
            enable_lineage: false,
            enable_temporal: false,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let interceptors = generate_interceptors(&schema, &octad, DatabaseBackend::SQLite);

        let view = interceptors[0].provenance_view.as_ref().expect("TODO: handle error");
        assert!(view.contains("verisimdb_posts_with_provenance"));
        assert!(view.contains("posts.id"));
        assert!(view.contains("posts.title"));
        assert!(view.contains("verisimdb_provenance_log"));
    }

    #[test]
    fn test_temporal_view_references_table() {
        let schema = test_schema();
        let octad = OctadConfig {
            enable_provenance: false,
            enable_lineage: false,
            enable_temporal: true,
            enable_access_control: false,
            enable_constraints: false,
            enable_simulation: false,
        };
        let interceptors = generate_interceptors(&schema, &octad, DatabaseBackend::SQLite);

        let view = interceptors[0].temporal_view.as_ref().expect("TODO: handle error");
        assert!(view.contains("verisimdb_posts_with_temporal"));
        assert!(view.contains("verisimdb_temporal_versions"));
        assert!(view.contains("valid_to IS NULL"));
    }

    #[test]
    fn test_render_interceptors_produces_sql() {
        let schema = test_schema();
        let octad = OctadConfig::default();
        let interceptors = generate_interceptors(&schema, &octad, DatabaseBackend::PostgreSQL);
        let rendered = render_interceptors(&interceptors);

        assert!(rendered.contains("SPDX-License-Identifier"));
        assert!(rendered.contains("Table: posts"));
    }

    #[test]
    fn test_entity_id_expr_composite_pk() {
        let expr =
            build_entity_id_expr(&["post_id", "tag_id"], "post_tags", DatabaseBackend::SQLite);
        assert!(expr.contains("post_tags.post_id"));
        assert!(expr.contains("post_tags.tag_id"));
        assert!(expr.contains("'::'"));
    }

    #[test]
    fn test_entity_id_expr_no_pk() {
        let expr = build_entity_id_expr(&[], "orphan", DatabaseBackend::SQLite);
        assert!(expr.contains("rowid"));

        let expr_pg = build_entity_id_expr(&[], "orphan", DatabaseBackend::PostgreSQL);
        assert!(expr_pg.contains("ctid"));
    }
}
