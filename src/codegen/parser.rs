// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Schema parser for VeriSimiser.
//
// Parses SQL DDL (CREATE TABLE statements) into an intermediate representation
// of tables and columns. This IR is then consumed by the overlay and query
// generators to produce sidecar schemas and query interceptors.
//
// Supported dialects: PostgreSQL, SQLite. MongoDB schemas are inferred from
// sample documents rather than DDL.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema IR (intermediate representation)
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
    /// Table name as declared in the DDL.
    pub name: String,
    /// Columns belonging to this table, in declaration order.
    pub columns: Vec<ColumnDef>,
}

/// A single column definition within a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column name as declared in the DDL.
    pub name: String,
    /// SQL type as a string (e.g., "INTEGER", "TEXT", "VARCHAR(255)").
    pub sql_type: String,
    /// Whether this column is part of the primary key.
    pub is_primary_key: bool,
    /// Whether this column has a NOT NULL constraint.
    pub is_not_null: bool,
}

// ---------------------------------------------------------------------------
// Parser implementation
// ---------------------------------------------------------------------------

/// Parse a SQL DDL string into a `ParsedSchema`.
///
/// This is a lightweight parser that handles common CREATE TABLE patterns.
/// It does not aim to be a full SQL parser — just enough to extract table
/// names, column names, types, and primary key designations for overlay
/// generation.
///
/// # Supported patterns
/// - `CREATE TABLE name (columns...);`
/// - `CREATE TABLE IF NOT EXISTS name (columns...);`
/// - Column-level PRIMARY KEY and NOT NULL constraints
/// - Inline `PRIMARY KEY (col1, col2)` table constraints
///
/// # Limitations
/// - Does not parse CHECK, UNIQUE, FOREIGN KEY, or DEFAULT constraints
/// - Does not handle quoted identifiers with special characters
/// - Does not parse ALTER TABLE or CREATE INDEX statements
pub fn parse_sql_schema(ddl: &str) -> Result<ParsedSchema> {
    let mut tables = Vec::new();

    // Normalise whitespace for easier matching: collapse runs of whitespace
    // (including newlines) into single spaces.
    let normalised = ddl
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Strip single-line SQL comments.
            if let Some(pos) = trimmed.find("--") {
                &trimmed[..pos]
            } else {
                trimmed
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Split on semicolons to get individual statements.
    for statement in normalised.split(';') {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }

        // Match CREATE TABLE statements (case-insensitive).
        let upper = stmt.to_uppercase();
        if !upper.starts_with("CREATE TABLE") {
            continue;
        }

        if let Some(table) = parse_create_table(stmt)? {
            tables.push(table);
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

/// Parse a single CREATE TABLE statement into a `TableDef`.
///
/// Returns `Ok(None)` if the statement cannot be parsed (non-fatal).
fn parse_create_table(statement: &str) -> Result<Option<TableDef>> {
    // Extract the table name: everything between "CREATE TABLE [IF NOT EXISTS]" and "(".
    let upper = statement.to_uppercase();

    // Find where the column list starts.
    let paren_start = match statement.find('(') {
        Some(pos) => pos,
        None => return Ok(None),
    };

    // Extract the table name portion.
    let _before_paren = statement[..paren_start].trim();
    let name_part = if upper.contains("IF NOT EXISTS") {
        // Skip past "CREATE TABLE IF NOT EXISTS".
        let idx = upper.find("IF NOT EXISTS").unwrap() + "IF NOT EXISTS".len();
        statement[idx..paren_start].trim()
    } else {
        // Skip past "CREATE TABLE".
        let idx = upper.find("TABLE").unwrap() + "TABLE".len();
        statement[idx..paren_start].trim()
    };

    let table_name = name_part
        .trim_matches('"')
        .trim_matches('`')
        .trim()
        .to_string();

    if table_name.is_empty() {
        return Ok(None);
    }

    // Extract the column list: everything between the first "(" and the last ")".
    let paren_end = match statement.rfind(')') {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let column_list_str = &statement[paren_start + 1..paren_end];

    // Track which columns are declared as primary key via table-level constraint.
    let mut table_pk_columns: Vec<String> = Vec::new();
    let mut columns: Vec<ColumnDef> = Vec::new();

    // Split on commas, but respect parentheses nesting (e.g., VARCHAR(255)).
    for part in split_respecting_parens(column_list_str) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let upper_part = trimmed.to_uppercase();

        // Check for table-level PRIMARY KEY constraint.
        if upper_part.starts_with("PRIMARY KEY") {
            if let Some(pk_start) = trimmed.find('(') {
                if let Some(pk_end) = trimmed.rfind(')') {
                    let pk_cols = &trimmed[pk_start + 1..pk_end];
                    for col in pk_cols.split(',') {
                        table_pk_columns.push(
                            col.trim()
                                .trim_matches('"')
                                .trim_matches('`')
                                .to_lowercase(),
                        );
                    }
                }
            }
            continue;
        }

        // Skip other table-level constraints (FOREIGN KEY, UNIQUE, CHECK, CONSTRAINT).
        if upper_part.starts_with("FOREIGN KEY")
            || upper_part.starts_with("UNIQUE")
            || upper_part.starts_with("CHECK")
            || upper_part.starts_with("CONSTRAINT")
        {
            continue;
        }

        // Parse as a column definition.
        if let Some(col) = parse_column_def(trimmed) {
            columns.push(col);
        }
    }

    // Apply table-level PRIMARY KEY to matching columns.
    for col in &mut columns {
        if table_pk_columns.contains(&col.name.to_lowercase()) {
            col.is_primary_key = true;
        }
    }

    Ok(Some(TableDef {
        name: table_name,
        columns,
    }))
}

/// Parse a single column definition string into a `ColumnDef`.
///
/// Expected format: `column_name TYPE [constraints...]`
fn parse_column_def(definition: &str) -> Option<ColumnDef> {
    let tokens: Vec<&str> = definition.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    let name = tokens[0]
        .trim_matches('"')
        .trim_matches('`')
        .to_string();

    // The SQL type is the second token (possibly with parenthesised length).
    // We need to reconstruct it if it was split by whitespace inside parens.
    let rest = &definition[definition.find(tokens[1]).unwrap_or(0)..];
    let upper_rest = rest.to_uppercase();

    // Extract the type: take tokens until we hit a constraint keyword.
    let constraint_keywords = [
        "PRIMARY", "NOT", "NULL", "DEFAULT", "UNIQUE", "CHECK", "REFERENCES",
        "AUTOINCREMENT", "AUTO_INCREMENT", "GENERATED", "SERIAL",
    ];
    let mut type_parts: Vec<&str> = Vec::new();
    for token in &tokens[1..] {
        if constraint_keywords.contains(&token.to_uppercase().as_str()) {
            break;
        }
        type_parts.push(token);
    }
    let sql_type = type_parts.join(" ");

    let is_primary_key = upper_rest.contains("PRIMARY KEY");
    let is_not_null = upper_rest.contains("NOT NULL") || is_primary_key;

    Some(ColumnDef {
        name,
        sql_type,
        is_primary_key,
        is_not_null,
    })
}

/// Split a string on commas while respecting parentheses nesting.
///
/// This ensures that `VARCHAR(255)` or `NUMERIC(10, 2)` are not split
/// at the comma inside the parentheses.
fn split_respecting_parens(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
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
}
