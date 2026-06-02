// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Schema parser for VeriSimiser.
//
// Parses SQL DDL (CREATE TABLE statements) into an intermediate representation
// of tables and columns. This IR is then consumed by the overlay and query
// generators to produce sidecar schemas and query interceptors.
//
// Backed by the `sqlparser` crate (V-L2-A1, #38): the previous hand-rolled
// uppercase-and-split scanner misclassified CHECK constraints,
// schema-qualified names, GENERATED columns, quoted identifiers containing
// whitespace, and any DDL with semicolons inside comments. We try the
// PostgreSQL dialect first, then SQLite, then a permissive generic dialect,
// and walk `Statement::CreateTable` into the stable IR below.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlparser::ast::{ColumnOption, Statement, TableConstraint};
use sqlparser::dialect::{Dialect, GenericDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

// ---------------------------------------------------------------------------
// Schema IR (intermediate representation) — unchanged public shape
// ---------------------------------------------------------------------------

/// A parsed database schema containing all discovered tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSchema {
    /// The tables discovered in the schema source.
    pub tables: Vec<TableDef>,
    /// The original source file path (if parsed from a file).
    pub source: Option<String>,
}

/// A single table definition extracted from DDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDef {
    /// Table name as declared in the DDL. For schema-qualified names
    /// (`schema.table`) this is the bare table identifier, with the
    /// quoting stripped (e.g. `"my table"` → `my table`).
    pub name: String,
    /// Columns belonging to this table, in declaration order.
    pub columns: Vec<ColumnDef>,
}

/// A single column definition within a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column name as declared in the DDL (quoting stripped).
    pub name: String,
    /// SQL type rendered canonically by `sqlparser` (e.g. `INTEGER`,
    /// `TEXT`, `VARCHAR(255)`, `NUMERIC(10,2)`).
    pub sql_type: String,
    /// Whether this column is part of the primary key (column-level
    /// `PRIMARY KEY` or a table-level `PRIMARY KEY (..)` listing it).
    pub is_primary_key: bool,
    /// Whether this column has a NOT NULL constraint (primary-key
    /// columns are implicitly NOT NULL).
    pub is_not_null: bool,
}

// ---------------------------------------------------------------------------
// Parser implementation
// ---------------------------------------------------------------------------

/// Parse SQL DDL with the first dialect that accepts it: PostgreSQL, then
/// SQLite, then a permissive generic dialect (covers `SERIAL`,
/// `AUTOINCREMENT`, generated columns, and dialect-specific types).
fn parse_statements(ddl: &str) -> Result<Vec<Statement>> {
    let pg = PostgreSqlDialect {};
    let sqlite = SQLiteDialect {};
    let generic = GenericDialect {};
    let dialects: [&dyn Dialect; 3] = [&pg, &sqlite, &generic];

    let mut last_err = None;
    for dialect in dialects {
        match Parser::parse_sql(dialect, ddl) {
            Ok(statements) => return Ok(statements),
            Err(e) => last_err = Some(e),
        }
    }
    Err(anyhow::anyhow!(
        "failed to parse SQL DDL with the PostgreSQL, SQLite, or generic \
         dialects: {}",
        last_err.expect("at least one dialect is always attempted")
    ))
}

/// Walk a parsed `CREATE TABLE` into the `TableDef` IR.
fn table_from_create(ct: sqlparser::ast::CreateTable) -> TableDef {
    // Schema-qualified names: the table is the last identifier segment.
    let name = ct
        .name
        .0
        .last()
        .map(|ident| ident.value.clone())
        .unwrap_or_default();

    // Table-level PRIMARY KEY (col, ...) — collect the named columns.
    let mut table_pk: Vec<String> = Vec::new();
    for constraint in &ct.constraints {
        if let TableConstraint::PrimaryKey { columns, .. } = constraint {
            for col in columns {
                table_pk.push(col.value.to_lowercase());
            }
        }
        // FOREIGN KEY / UNIQUE / CHECK table constraints carry no column
        // IR of their own — intentionally ignored, not mis-parsed.
    }

    let mut columns = Vec::new();
    for col in ct.columns {
        let col_name = col.name.value.clone();
        let sql_type = col.data_type.to_string();

        let mut is_primary_key = false;
        let mut is_not_null = false;
        for opt in &col.options {
            match &opt.option {
                // Column-level PRIMARY KEY is a unique option flagged primary.
                ColumnOption::Unique { is_primary, .. } if *is_primary => {
                    is_primary_key = true;
                }
                ColumnOption::NotNull => is_not_null = true,
                // CHECK / DEFAULT / GENERATED / REFERENCES etc. do not
                // affect the (name, type, pk, not-null) IR.
                _ => {}
            }
        }

        if table_pk.contains(&col_name.to_lowercase()) {
            is_primary_key = true;
        }
        if is_primary_key {
            is_not_null = true;
        }

        columns.push(ColumnDef {
            name: col_name,
            sql_type,
            is_primary_key,
            is_not_null,
        });
    }

    TableDef { name, columns }
}

/// Parse a SQL DDL string into a `ParsedSchema`.
///
/// Only `CREATE TABLE` statements contribute to the IR; other statements
/// (CREATE INDEX, ALTER TABLE, comments) are parsed for correctness but
/// produce no tables.
pub fn parse_sql_schema(ddl: &str) -> Result<ParsedSchema> {
    let statements = parse_statements(ddl)?;
    let mut tables = Vec::new();
    for stmt in statements {
        if let Statement::CreateTable(ct) = stmt {
            tables.push(table_from_create(ct));
        }
    }
    Ok(ParsedSchema {
        tables,
        source: None,
    })
}

/// Parse a SQL DDL file from disk into a `ParsedSchema`.
///
/// Reads the file contents and delegates to `parse_sql_schema`.
pub fn parse_schema_file(path: &str) -> Result<ParsedSchema> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read schema file: {}", path))?;
    let mut schema = parse_sql_schema(&contents)?;
    schema.source = Some(path.to_string());
    Ok(schema)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let ddl = r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                body TEXT,
                created_at TIMESTAMP NOT NULL
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "posts");
        assert_eq!(schema.tables[0].columns.len(), 4);

        let id_col = &schema.tables[0].columns[0];
        assert_eq!(id_col.name, "id");
        assert!(id_col.is_primary_key);
        assert!(id_col.is_not_null);

        let body_col = &schema.tables[0].columns[2];
        assert_eq!(body_col.name, "body");
        assert!(!body_col.is_primary_key);
        assert!(!body_col.is_not_null);
    }

    #[test]
    fn test_parse_multiple_tables() {
        let ddl = r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                author_id INTEGER NOT NULL,
                title TEXT NOT NULL
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables.len(), 2);
        assert_eq!(schema.tables[0].name, "users");
        assert_eq!(schema.tables[1].name, "posts");
    }

    #[test]
    fn test_parse_table_level_pk() {
        let ddl = r#"
            CREATE TABLE post_tags (
                post_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (post_id, tag_id)
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables[0].columns.len(), 2);
        assert!(schema.tables[0].columns[0].is_primary_key);
        assert!(schema.tables[0].columns[1].is_primary_key);
    }

    #[test]
    fn test_parse_if_not_exists() {
        let ddl = "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT);";
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "settings");
    }

    #[test]
    fn test_parse_empty_schema() {
        let ddl = "-- just a comment\n";
        let schema = parse_sql_schema(ddl).unwrap();
        assert!(schema.tables.is_empty());
    }

    #[test]
    fn test_parse_varchar_with_length() {
        let ddl = "CREATE TABLE users (name VARCHAR(255) NOT NULL, email VARCHAR(320));";
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables[0].columns[0].sql_type, "VARCHAR(255)");
        assert_eq!(schema.tables[0].columns[1].sql_type, "VARCHAR(320)");
    }

    // --- #38 acceptance: cases the hand-rolled scanner got wrong ---

    #[test]
    fn test_schema_qualified_name() {
        // The old scanner kept the schema prefix in the table name.
        let ddl = "CREATE TABLE analytics.events (id BIGINT PRIMARY KEY, kind TEXT);";
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "events");
        assert!(schema.tables[0].columns[0].is_primary_key);
    }

    #[test]
    fn test_quoted_identifier_with_whitespace() {
        let ddl = r#"CREATE TABLE "audit log" ("user name" TEXT NOT NULL, ts TIMESTAMP);"#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables[0].name, "audit log");
        assert_eq!(schema.tables[0].columns[0].name, "user name");
        assert!(schema.tables[0].columns[0].is_not_null);
        assert_eq!(schema.tables[0].columns.len(), 2);
    }

    #[test]
    fn test_check_constraint_does_not_corrupt_columns() {
        // The old scanner split on the comma inside CHECK(...) and produced
        // bogus columns; here the CHECK must be ignored cleanly.
        let ddl = r#"
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                balance NUMERIC(12,2) NOT NULL CHECK (balance >= 0),
                status TEXT,
                CHECK (status IN ('open', 'closed'))
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables[0].columns.len(), 3);
        assert_eq!(schema.tables[0].columns[1].name, "balance");
        assert_eq!(schema.tables[0].columns[1].sql_type, "NUMERIC(12,2)");
        assert!(schema.tables[0].columns[1].is_not_null);
        assert_eq!(schema.tables[0].columns[2].name, "status");
    }

    #[test]
    fn test_generated_column() {
        let ddl = r#"
            CREATE TABLE rectangles (
                w INTEGER NOT NULL,
                h INTEGER NOT NULL,
                area INTEGER GENERATED ALWAYS AS (w * h) STORED
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables[0].columns.len(), 3);
        let area = &schema.tables[0].columns[2];
        assert_eq!(area.name, "area");
        assert_eq!(area.sql_type, "INTEGER");
    }

    #[test]
    fn test_semicolon_inside_comment_is_not_a_statement_break() {
        let ddl = r#"
            CREATE TABLE t (
                id INTEGER PRIMARY KEY -- a trailing ; inside a comment
            );
        "#;
        let schema = parse_sql_schema(ddl).unwrap();
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "t");
        assert_eq!(schema.tables[0].columns.len(), 1);
    }

    #[test]
    fn test_invalid_sql_is_an_error_not_a_silent_empty_schema() {
        let err = parse_sql_schema("CREATE TABLE (((").unwrap_err();
        assert!(
            err.to_string().contains("failed to parse SQL DDL"),
            "unexpected error: {err}"
        );
    }
}
