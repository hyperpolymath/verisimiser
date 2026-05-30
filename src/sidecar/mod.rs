// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Sidecar storage backend selection.
//
// `[sidecar].storage` (+ `[sidecar].format` for the json family) resolves
// to a [`StorageKind`]: `Sqlite`, `Postgres`, or `Json(JsonFormat)`. This
// module is the single source of truth for which storage values are
// accepted; `validate`/`doctor`, `generate`, `drift`, and `gc` all
// dispatch on it.
//
// V-L2-F3 (#146): re-opens the JSON sidecar capability that was dropped in
// V-L2-F2 (#112/#144), now as a deliberately-scoped *family* — plain JSON,
// JSON-LD, and NDJSON — with full parity to the runtime operations the
// SQLite path implements today. The JSON store itself lives in [`json`].

pub mod json;
pub mod lock;

use crate::codegen::overlay::SqlDialect;

/// On-disk encoding for the `json` sidecar store. The format is purely a
/// codec over the shared [`json::SidecarData`] model — every octad
/// operation is written once and is format-independent.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JsonFormat {
    /// One JSON object keyed by table name, each holding an array of rows.
    Plain,
    /// JSON-LD: `@context` + `@graph` of typed (`@type`/`@id`) nodes.
    Ld,
    /// Newline-delimited JSON: one `{"_table": …, …}` record per line.
    Ndjson,
}

impl JsonFormat {
    /// Parse a `[sidecar].format` value (case-insensitive). An empty
    /// string is treated as the default (`plain`) so `storage = "json"`
    /// with no explicit `format` still resolves.
    pub fn parse(format: &str) -> anyhow::Result<Self> {
        match format.to_lowercase().as_str() {
            "" | "plain" | "json" => Ok(JsonFormat::Plain),
            "ld" | "json-ld" | "jsonld" => Ok(JsonFormat::Ld),
            "ndjson" | "nd-json" | "jsonl" | "jsonlines" => Ok(JsonFormat::Ndjson),
            other => anyhow::bail!(
                "unsupported [sidecar].format {other:?}; supported values are \
                 \"plain\" (default), \"ld\" (JSON-LD), and \"ndjson\"."
            ),
        }
    }

    /// Canonical lower-case token for this format.
    pub fn as_str(self) -> &'static str {
        match self {
            JsonFormat::Plain => "plain",
            JsonFormat::Ld => "ld",
            JsonFormat::Ndjson => "ndjson",
        }
    }

    /// File extension for an emitted scaffold (`generate`).
    pub fn extension(self) -> &'static str {
        match self {
            JsonFormat::Plain => "json",
            JsonFormat::Ld => "jsonld",
            JsonFormat::Ndjson => "ndjson",
        }
    }
}

/// The resolved sidecar storage backend.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StorageKind {
    /// SQLite sidecar (the reference store).
    Sqlite,
    /// PostgreSQL sidecar (SQL dialect; same overlay schema).
    Postgres,
    /// JSON-family document store in the given on-disk [`JsonFormat`].
    Json(JsonFormat),
}

impl StorageKind {
    /// Resolve `[sidecar].storage` (+ `[sidecar].format` for `json`) to a
    /// backend. Case-insensitive; `format` is only consulted for `json`.
    ///
    /// This is the canonical validator for `[sidecar]` storage selection —
    /// `validate`, `generate`, `drift`, and `gc` all defer to it so they
    /// agree on the accepted set.
    pub fn resolve(storage: &str, format: &str) -> anyhow::Result<Self> {
        match storage.to_lowercase().as_str() {
            "sqlite" => Ok(StorageKind::Sqlite),
            "postgres" | "postgresql" => Ok(StorageKind::Postgres),
            "json" => Ok(StorageKind::Json(JsonFormat::parse(format)?)),
            other => anyhow::bail!(
                "unsupported [sidecar].storage {other:?}; supported values are \
                 \"sqlite\" (default), \"postgres\"/\"postgresql\", and \"json\" \
                 (with [sidecar].format = plain|ld|ndjson)."
            ),
        }
    }

    /// The SQL dialect for SQL-backed kinds; `None` for [`StorageKind::Json`].
    /// Lets `generate` reuse the existing `codegen::overlay` DDL path for
    /// SQL stores and branch to the JSON codec otherwise.
    pub fn sql_dialect(self) -> Option<SqlDialect> {
        match self {
            StorageKind::Sqlite => Some(SqlDialect::Sqlite),
            StorageKind::Postgres => Some(SqlDialect::Postgres),
            StorageKind::Json(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_sql_backends_case_insensitively() {
        assert_eq!(
            StorageKind::resolve("sqlite", "").unwrap(),
            StorageKind::Sqlite
        );
        assert_eq!(
            StorageKind::resolve("Postgres", "").unwrap(),
            StorageKind::Postgres
        );
        assert_eq!(
            StorageKind::resolve("POSTGRESQL", "plain").unwrap(),
            StorageKind::Postgres
        );
        // format is ignored for SQL backends.
        assert_eq!(
            StorageKind::resolve("sqlite", "ndjson").unwrap(),
            StorageKind::Sqlite
        );
    }

    #[test]
    fn resolves_json_family_with_format() {
        assert_eq!(
            StorageKind::resolve("json", "").unwrap(),
            StorageKind::Json(JsonFormat::Plain),
            "json with no format defaults to plain"
        );
        assert_eq!(
            StorageKind::resolve("json", "plain").unwrap(),
            StorageKind::Json(JsonFormat::Plain)
        );
        assert_eq!(
            StorageKind::resolve("JSON", "JSON-LD").unwrap(),
            StorageKind::Json(JsonFormat::Ld)
        );
        assert_eq!(
            StorageKind::resolve("json", "ndjson").unwrap(),
            StorageKind::Json(JsonFormat::Ndjson)
        );
    }

    #[test]
    fn rejects_unknown_storage_and_format() {
        let storage_err = StorageKind::resolve("mariadb", "plain")
            .unwrap_err()
            .to_string();
        assert!(storage_err.contains("unsupported") && storage_err.contains("json"));

        let format_err = StorageKind::resolve("json", "yaml")
            .unwrap_err()
            .to_string();
        assert!(
            format_err.contains("unsupported") && format_err.contains("ndjson"),
            "bad format must list supported formats, got: {format_err}"
        );
    }

    #[test]
    fn sql_dialect_is_none_for_json() {
        assert!(
            StorageKind::resolve("json", "ld")
                .unwrap()
                .sql_dialect()
                .is_none()
        );
        assert_eq!(
            StorageKind::resolve("sqlite", "").unwrap().sql_dialect(),
            Some(SqlDialect::Sqlite)
        );
        assert_eq!(
            StorageKind::resolve("postgres", "").unwrap().sql_dialect(),
            Some(SqlDialect::Postgres)
        );
    }

    #[test]
    fn format_tokens_and_extensions() {
        assert_eq!(JsonFormat::Plain.as_str(), "plain");
        assert_eq!(JsonFormat::Ld.as_str(), "ld");
        assert_eq!(JsonFormat::Ndjson.as_str(), "ndjson");
        assert_eq!(JsonFormat::Plain.extension(), "json");
        assert_eq!(JsonFormat::Ld.extension(), "jsonld");
        assert_eq!(JsonFormat::Ndjson.extension(), "ndjson");
    }
}
