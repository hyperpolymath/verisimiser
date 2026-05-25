// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Integration tests for VeriSimiser Phase 1.
//
// These tests exercise the full pipeline: manifest loading, schema parsing,
// overlay generation, query interceptor generation, and ABI type correctness.

use verisimiser::abi::{
    AccessPolicy, DatabaseBackend, LineageEdge, OctadDimension, ProvenanceEntry, TemporalVersion,
};
use verisimiser::codegen::{overlay, parser, query};
use verisimiser::manifest::{self, OctadConfig};

// ---------------------------------------------------------------------------
// Test 1: Full pipeline — parse schema, generate overlay, generate interceptors
// ---------------------------------------------------------------------------

#[test]
fn test_full_pipeline_blog_schema() {
    // A realistic blog database schema with 3 tables.
    let blog_ddl = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email VARCHAR(320) NOT NULL,
            created_at TIMESTAMP NOT NULL
        );

        CREATE TABLE posts (
            id INTEGER PRIMARY KEY,
            author_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            body TEXT,
            published BOOLEAN NOT NULL,
            created_at TIMESTAMP NOT NULL
        );

        CREATE TABLE comments (
            id INTEGER PRIMARY KEY,
            post_id INTEGER NOT NULL,
            author_id INTEGER NOT NULL,
            body TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL
        );
    "#;

    // Step 1: Parse the schema.
    let schema = parser::parse_sql_schema(blog_ddl).unwrap();
    assert_eq!(schema.tables.len(), 3, "Should parse 3 tables");
    assert_eq!(schema.tables[0].name, "users");
    assert_eq!(schema.tables[1].name, "posts");
    assert_eq!(schema.tables[2].name, "comments");

    // Verify column counts.
    assert_eq!(
        schema.tables[0].columns.len(),
        4,
        "users should have 4 columns"
    );
    assert_eq!(
        schema.tables[1].columns.len(),
        6,
        "posts should have 6 columns"
    );
    assert_eq!(
        schema.tables[2].columns.len(),
        5,
        "comments should have 5 columns"
    );

    // Step 2: Generate sidecar overlay with all dimensions enabled.
    let octad = OctadConfig {
        enable_provenance: true,
        enable_lineage: true,
        enable_temporal: true,
        enable_access_control: true,
        enable_constraints: true,
        enable_simulation: false,
    };
    let overlay_ddl =
        overlay::generate_sidecar_schema(&schema, &octad, overlay::SqlDialect::Sqlite)
            .expect("schema is valid");

    // Verify all expected sidecar tables are present.
    assert!(
        overlay_ddl.contains("verisimdb_metadata"),
        "Should contain metadata table"
    );
    assert!(
        overlay_ddl.contains("verisimdb_provenance_log"),
        "Should contain provenance table"
    );
    assert!(
        overlay_ddl.contains("verisimdb_lineage_graph"),
        "Should contain lineage table"
    );
    assert!(
        overlay_ddl.contains("verisimdb_temporal_versions"),
        "Should contain temporal table"
    );
    assert!(
        overlay_ddl.contains("verisimdb_access_policies"),
        "Should contain access policies table"
    );

    // Verify metadata seeds reference all 3 tables.
    assert!(
        overlay_ddl.contains("'users'"),
        "Metadata should reference users"
    );
    assert!(
        overlay_ddl.contains("'posts'"),
        "Metadata should reference posts"
    );
    assert!(
        overlay_ddl.contains("'comments'"),
        "Metadata should reference comments"
    );

    // Step 3: Generate query interceptors.
    let interceptors = query::generate_interceptors(&schema, &octad, DatabaseBackend::SQLite);
    assert_eq!(
        interceptors.len(),
        3,
        "Should generate interceptors for 3 tables"
    );

    // Verify each table has the expected interceptor components.
    for interceptor in &interceptors {
        assert!(
            interceptor.provenance_view.is_some(),
            "{}: should have provenance view",
            interceptor.table_name
        );
        assert!(
            interceptor.temporal_view.is_some(),
            "{}: should have temporal view",
            interceptor.table_name
        );
        assert!(
            interceptor.lineage_query.is_some(),
            "{}: should have lineage query",
            interceptor.table_name
        );
        assert!(
            interceptor.access_filter.is_some(),
            "{}: should have access filter",
            interceptor.table_name
        );
    }

    // Step 4: Render interceptors to SQL and verify output.
    let rendered = query::render_interceptors(&interceptors);
    assert!(rendered.contains("verisimdb_users_with_provenance"));
    assert!(rendered.contains("verisimdb_posts_with_temporal"));
    assert!(rendered.contains("verisimdb_comments_with_provenance"));
}

// ---------------------------------------------------------------------------
// Test 2: Manifest loading with Phase 1 schema
// ---------------------------------------------------------------------------

#[test]
fn test_manifest_load_phase1_schema() {
    let toml_content = r#"
[project]
name = "test-blog-db"
version = "0.1.0"
description = "Integration test manifest"

[database]
backend = "sqlite"
connection-string-env = "BLOG_DB_URL"
schema-source = "schema.sql"

[octad]
enable-provenance = true
enable-lineage = true
enable-temporal = true
enable-access-control = false
enable-simulation = false

[sidecar]
storage = "sqlite"
path = ".verisim/test-sidecar.db"
"#;

    let manifest: manifest::Manifest = toml::from_str(toml_content).unwrap();

    assert_eq!(manifest.project.name, "test-blog-db");
    assert_eq!(manifest.project.version, "0.1.0");
    assert_eq!(manifest.database.backend, "sqlite");
    assert_eq!(manifest.database.connection_string_env, "BLOG_DB_URL");
    assert_eq!(
        manifest.database.schema_source,
        Some("schema.sql".to_string())
    );

    assert!(manifest.octad.enable_provenance);
    assert!(manifest.octad.enable_lineage);
    assert!(manifest.octad.enable_temporal);
    assert!(!manifest.octad.enable_access_control);
    assert!(!manifest.octad.enable_simulation);

    assert_eq!(manifest.sidecar.storage, "sqlite");
    assert_eq!(manifest.sidecar.path, ".verisim/test-sidecar.db");

    // Enabled count: data(1) + metadata(1) + provenance(1) + lineage(1) + temporal(1) + constraints(1) = 6
    assert_eq!(manifest.octad.enabled_count(), 6);
}

// ---------------------------------------------------------------------------
// Test 3: Manifest backward compatibility with legacy schema
// ---------------------------------------------------------------------------

#[test]
fn test_manifest_backward_compatibility() {
    let legacy_toml = r#"
[verisimiser]
name = "legacy-db"

[database]
target-db = "postgresql"

[tier1]
drift-detection = true
provenance = true
temporal-versioning = true

[tier2]
graph = false
vector = false
"#;

    let manifest: manifest::Manifest = toml::from_str(legacy_toml).unwrap();

    assert_eq!(manifest.verisimiser.name, "legacy-db");
    assert_eq!(manifest.database.target_db, "postgresql");
    assert_eq!(manifest.database.effective_backend().unwrap(), "postgresql");
    assert!(manifest.tier1.provenance);
    assert!(manifest.tier1.temporal_versioning);
    assert!(manifest.tier1.drift_detection);
}

// ---------------------------------------------------------------------------
// Test 4: Provenance hash chain integrity across multiple operations
// ---------------------------------------------------------------------------

#[test]
fn test_provenance_chain_integrity_multi_step() {
    // Simulate a real sequence: create entity, update twice, verify chain.
    let genesis = ProvenanceEntry::genesis("post-42", "system-init");
    assert!(genesis.verify(), "Genesis entry should be valid");
    assert!(genesis.previous_hash.is_empty());
    assert_eq!(genesis.operation, "insert");

    let update1 = genesis.chain("update", "user-alice");
    assert!(update1.verify(), "First update should be valid");
    assert_eq!(update1.previous_hash, genesis.hash);

    let update2 = update1.chain("update", "user-bob");
    assert!(update2.verify(), "Second update should be valid");
    assert_eq!(update2.previous_hash, update1.hash);

    let delete = update2.chain("delete", "user-alice");
    assert!(delete.verify(), "Delete entry should be valid");
    assert_eq!(delete.previous_hash, update2.hash);

    // Verify chain linkage: genesis -> update1 -> update2 -> delete
    assert_ne!(genesis.hash, update1.hash);
    assert_ne!(update1.hash, update2.hash);
    assert_ne!(update2.hash, delete.hash);

    // Tamper detection: post-V-L2-C1 the hash covers actor, so a
    // tamper to actor alone now breaks verification (closes #30 / V-L2-C4).
    let mut tampered = update1.clone();
    tampered.actor = "evil-mallory".to_string();
    assert!(
        !tampered.verify(),
        "Tampering with actor must break verification"
    );
    // Modifying a hash-covered field is also detected.
    let mut tampered_op = update1.clone();
    tampered_op.operation = "delete".to_string();
    assert!(
        !tampered_op.verify(),
        "tampering with operation must break verify"
    );

    let mut tampered_snap = update1.clone();
    tampered_snap.before_snapshot = Some("{}".into());
    assert!(
        !tampered_snap.verify(),
        "before_snapshot is part of the hash; tampering with it must break verify"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Temporal versioning lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_temporal_version_lifecycle() {
    let v1 = TemporalVersion::initial(
        "post-1",
        serde_json::json!({"title": "Draft", "body": "..."}),
    );
    assert_eq!(v1.version, 1);
    assert!(v1.is_current());
    assert_eq!(v1.operation, "insert");

    let v2 = v1.next_version(
        serde_json::json!({"title": "Published", "body": "Hello world"}),
        "update",
    );
    assert_eq!(v2.version, 2);
    assert!(v2.is_current());
    assert_eq!(v2.entity_id, "post-1");

    let v3 = v2.next_version(
        serde_json::json!({"title": "Draft", "body": "..."}),
        "rollback",
    );
    assert_eq!(v3.version, 3);
    assert_eq!(v3.operation, "rollback");

    // Version numbers are strictly monotonic.
    assert!(v1.version < v2.version);
    assert!(v2.version < v3.version);
}

// ---------------------------------------------------------------------------
// Test 6: Octad dimensions enumeration and properties
// ---------------------------------------------------------------------------

#[test]
fn test_octad_dimensions_enumeration() {
    let dims = OctadDimension::all();
    assert_eq!(dims.len(), 8, "Octad must have exactly 8 dimensions");

    // Check that inherent dimensions are correctly identified.
    let inherent: Vec<_> = dims.iter().filter(|d| d.is_inherent()).collect();
    assert_eq!(inherent.len(), 2, "Exactly 2 inherent dimensions");
    assert_eq!(*inherent[0], OctadDimension::Data);
    assert_eq!(*inherent[1], OctadDimension::Metadata);

    // Check that each dimension has a non-empty label.
    for dim in &dims {
        assert!(!dim.label().is_empty(), "{:?} should have a label", dim);
    }

    // Check Display implementation.
    assert_eq!(format!("{}", OctadDimension::Provenance), "Provenance");
    assert_eq!(
        format!("{}", OctadDimension::AccessControl),
        "Access Control"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Database backend parsing and name round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_database_backend_round_trip() {
    let backends = [
        ("postgresql", DatabaseBackend::PostgreSQL),
        ("postgres", DatabaseBackend::PostgreSQL),
        ("pg", DatabaseBackend::PostgreSQL),
        ("sqlite", DatabaseBackend::SQLite),
        ("sqlite3", DatabaseBackend::SQLite),
        ("mongodb", DatabaseBackend::MongoDB),
        ("mongo", DatabaseBackend::MongoDB),
    ];

    for (input, expected) in &backends {
        let parsed = DatabaseBackend::from_str(input);
        assert_eq!(
            parsed,
            Some(*expected),
            "Failed to parse '{}' as {:?}",
            input,
            expected
        );
    }

    // Canonical names round-trip.
    assert_eq!(DatabaseBackend::PostgreSQL.name(), "postgresql");
    assert_eq!(DatabaseBackend::SQLite.name(), "sqlite");
    assert_eq!(DatabaseBackend::MongoDB.name(), "mongodb");

    // Unknown backends return None.
    assert_eq!(DatabaseBackend::from_str("mysql"), None);
    assert_eq!(DatabaseBackend::from_str("redis"), None);
}

// ---------------------------------------------------------------------------
// Test 8: Lineage and access policy creation
// ---------------------------------------------------------------------------

#[test]
fn test_lineage_and_access_policy_types() {
    // Lineage edge.
    let edge = LineageEdge::new("users", "user_stats", "aggregate");
    assert_eq!(edge.source_entity, "users");
    assert_eq!(edge.target_entity, "user_stats");
    assert_eq!(edge.derivation_type, "aggregate");
    assert!(edge.description.is_none());

    // Access policy — table-level.
    let policy = AccessPolicy::new("posts", "role:editor", "write");
    assert_eq!(policy.target_table, "posts");
    assert_eq!(policy.principal, "role:editor");
    assert_eq!(policy.access_level, "write");
    assert!(policy.active);
    assert!(policy.target_column.is_none());
    assert!(policy.condition.is_none());

    // Access policy — column-level.
    let col_policy = AccessPolicy::for_column("users", "email", "role:admin", "read");
    assert_eq!(col_policy.target_column, Some("email".to_string()));
    assert_eq!(col_policy.access_level, "read");
}

// ---------------------------------------------------------------------------
// Test 9: End-to-end file-based workflow with tempfile
// ---------------------------------------------------------------------------

#[test]
fn test_end_to_end_file_workflow() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();

    // Write a schema file.
    let schema_path = dir.path().join("schema.sql");
    {
        let mut f = std::fs::File::create(&schema_path).unwrap();
        writeln!(
            f,
            "CREATE TABLE articles (id INTEGER PRIMARY KEY, title TEXT NOT NULL, content TEXT);"
        )
        .unwrap();
    }

    // Write a manifest file. Note: on Windows, schema_path uses backslashes
    // which are escape characters in TOML basic strings — emit the path as a
    // TOML literal string (single-quoted) to dodge escape interpretation.
    let manifest_path = dir.path().join("verisimiser.toml");
    {
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(
            f,
            r#"
[project]
name = "test-articles"

[database]
backend = "sqlite"
connection-string-env = "TEST_DB"
schema-source = '{}'

[octad]
enable-provenance = true
enable-lineage = false
enable-temporal = true
enable-access-control = false
enable-simulation = false

[sidecar]
storage = "sqlite"
path = ".verisim/test.db"
"#,
            schema_path.display()
        )
        .unwrap();
    }

    // Load the manifest.
    let manifest = manifest::load_manifest(manifest_path.to_str().unwrap()).unwrap();
    assert_eq!(manifest.project.name, "test-articles");
    assert_eq!(manifest.database.backend, "sqlite");

    // Parse the schema from file.
    let schema = parser::parse_schema_file(schema_path.to_str().unwrap()).unwrap();
    assert_eq!(schema.tables.len(), 1);
    assert_eq!(schema.tables[0].name, "articles");

    // Generate overlay.
    let overlay_ddl =
        overlay::generate_sidecar_schema(&schema, &manifest.octad, overlay::SqlDialect::Sqlite)
            .expect("schema is valid");
    assert!(overlay_ddl.contains("verisimdb_provenance_log"));
    assert!(overlay_ddl.contains("verisimdb_temporal_versions"));
    assert!(
        !overlay_ddl.contains("verisimdb_lineage_graph"),
        "Lineage is disabled"
    );
    assert!(
        !overlay_ddl.contains("verisimdb_access_policies"),
        "Access control is disabled"
    );

    // Generate interceptors.
    let interceptors =
        query::generate_interceptors(&schema, &manifest.octad, DatabaseBackend::SQLite);
    assert_eq!(interceptors.len(), 1);
    assert!(interceptors[0].provenance_view.is_some());
    assert!(interceptors[0].temporal_view.is_some());
    assert!(interceptors[0].lineage_query.is_none());
    assert!(interceptors[0].access_filter.is_none());
}
