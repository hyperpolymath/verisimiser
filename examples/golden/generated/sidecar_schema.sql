-- SPDX-License-Identifier: MPL-2.0
-- VeriSimiser sidecar schema (auto-generated)
-- Do not edit manually; regenerate with `verisimiser init`.

-- Metadata: tracks augmented target tables
CREATE TABLE IF NOT EXISTS verisimdb_metadata (
    table_name   TEXT PRIMARY KEY,
    column_count INTEGER NOT NULL,
    pk_columns   TEXT NOT NULL,   -- comma-separated list of PK column names
    discovered_at TEXT NOT NULL    -- ISO 8601 timestamp
);

-- Seed metadata from parsed schema (SQLite)
INSERT OR IGNORE INTO verisimdb_metadata (table_name, column_count, pk_columns, discovered_at)
    VALUES ('users', 4, 'id', CURRENT_TIMESTAMP);
INSERT OR IGNORE INTO verisimdb_metadata (table_name, column_count, pk_columns, discovered_at)
    VALUES ('posts', 6, 'id', CURRENT_TIMESTAMP);

-- Provenance: SHA-256 hash-chained audit trail (ADR-0010)
CREATE TABLE IF NOT EXISTS verisimdb_provenance_log (
    hash          TEXT PRIMARY KEY,
    previous_hash TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    operation     TEXT NOT NULL CHECK (operation IN ('insert','update','delete','transform')),  -- V-L2-J1
    actor         TEXT NOT NULL,
    timestamp     TEXT NOT NULL,  -- ISO 8601
    before_snapshot TEXT,          -- JSON of entity state before operation
    transformation  TEXT,          -- description of transformation applied
    CHECK (operation IN ('insert','update','delete','transform'))
);
-- ADR-0010 #32 (superseded): NO UNIQUE(entity_id, previous_hash) —
-- a fork that cannot be written cannot be detected or audited. The
-- non-unique index below makes fork detection O(log n) instead.
CREATE INDEX IF NOT EXISTS idx_provenance_predecessor
    ON verisimdb_provenance_log(entity_id, previous_hash);
CREATE INDEX IF NOT EXISTS idx_provenance_entity ON verisimdb_provenance_log(entity_id);
CREATE INDEX IF NOT EXISTS idx_provenance_table  ON verisimdb_provenance_log(table_name);

-- ADR-0010 #31: chain-tip *set*. `append_provenance` keeps a
-- BEGIN IMMEDIATE write so racing duplicate appends on one node
-- still serialise; a linear append swaps its single tip, a
-- deliberate fork adds a tip without removing one.
CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_heads (
    entity_id TEXT NOT NULL,
    head_hash TEXT NOT NULL,
    PRIMARY KEY (entity_id, head_hash)
);
-- Legacy single-head table: kept one release for non-destructive
-- migration (see tier1::provenance::SIDECAR_DDL). No DROP ships here.
CREATE TABLE IF NOT EXISTS verisimdb_provenance_chain_head (
    entity_id  TEXT PRIMARY KEY,
    head_hash  TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Lineage: data derivation graph (DAG by intent; cycle prevention is
-- a runtime concern — see V-L1-G1 / V-L2-I2).
CREATE TABLE IF NOT EXISTS verisimdb_lineage_graph (
    edge_id         TEXT PRIMARY KEY,
    source_entity   TEXT NOT NULL,
    source_table    TEXT NOT NULL,
    target_entity   TEXT NOT NULL,
    target_table    TEXT NOT NULL,
    derivation_type TEXT NOT NULL
        CHECK (derivation_type IN ('copy','transform','aggregate','join','filter')),  -- V-L2-J1
    description     TEXT,
    created_at      TEXT NOT NULL,  -- ISO 8601
    -- V-L2-I1: self-edges are not derivations; rejected at DB level.
    CHECK (NOT (source_entity = target_entity AND source_table = target_table))
);
CREATE INDEX IF NOT EXISTS idx_lineage_source ON verisimdb_lineage_graph(source_entity);
CREATE INDEX IF NOT EXISTS idx_lineage_target ON verisimdb_lineage_graph(target_entity);

-- Temporal: version history with point-in-time support.
-- V-L2-H1: the partial UNIQUE INDEX enforces exactly one
-- current row per (entity, table) — "only one version is
-- valid right now" was an application-layer invariant before;
-- now it's structural.
-- V-L2-J1: operation is a closed set.
-- V-L2-H2: valid_to (if set) must not predate valid_from.
CREATE TABLE IF NOT EXISTS verisimdb_temporal_versions (
    entity_id  TEXT NOT NULL,
    table_name TEXT NOT NULL,
    version    INTEGER NOT NULL CHECK (version >= 1),
    valid_from TEXT NOT NULL,   -- ISO 8601
    valid_to   TEXT,            -- ISO 8601, NULL if current
    snapshot   TEXT NOT NULL,   -- JSON serialisation of entity state
    operation  TEXT NOT NULL CHECK (operation IN ('insert','update','rollback')),
    PRIMARY KEY (entity_id, table_name, version),
    CHECK (valid_to IS NULL OR valid_to >= valid_from)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_temporal_current
    ON verisimdb_temporal_versions(entity_id, table_name)
    WHERE valid_to IS NULL;

-- Access Control: row/column-level access policies.
-- V-L2-J1: access_level is a closed set.
CREATE TABLE IF NOT EXISTS verisimdb_access_policies (
    policy_id     TEXT PRIMARY KEY,
    target_table  TEXT NOT NULL,
    target_column TEXT,            -- NULL means whole-row policy
    principal     TEXT NOT NULL,   -- user, role, or group identifier
    access_level  TEXT NOT NULL
        CHECK (access_level IN ('read','write','admin','deny')),
    condition     TEXT,            -- SQL-like filter condition (V-L1-H1)
    created_at    TEXT NOT NULL,   -- ISO 8601
    active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1))
);
CREATE INDEX IF NOT EXISTS idx_access_table ON verisimdb_access_policies(target_table);
CREATE INDEX IF NOT EXISTS idx_access_principal ON verisimdb_access_policies(principal);

