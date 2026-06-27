-- SPDX-License-Identifier: MPL-2.0
-- VeriSimiser query interceptors (auto-generated)

-- ==========================================================
-- Table: users
-- ==========================================================

-- Provenance-enriched view for 'users'
-- Joins each row with its latest provenance entry from the sidecar.
CREATE VIEW IF NOT EXISTS verisimdb_users_with_provenance AS
SELECT
    users.id,
    users.username,
    users.email,
    users.created_at,
    prov.operation   AS _verisimdb_last_operation,
    prov.actor       AS _verisimdb_last_actor,
    prov.timestamp   AS _verisimdb_last_modified,
    prov.hash        AS _verisimdb_provenance_hash
FROM users
LEFT JOIN (
    SELECT entity_id, operation, actor, timestamp, hash
    FROM (
        SELECT entity_id, operation, actor, timestamp, hash,
               ROW_NUMBER() OVER (PARTITION BY entity_id ORDER BY timestamp DESC) AS _rn
        FROM verisimdb_provenance_log
        WHERE table_name = 'users'
    ) ranked
    WHERE _rn = 1
) prov ON prov.entity_id = (CAST(users.id AS TEXT));

-- Temporal-enriched view for 'users'
-- Joins each row with its current version metadata.
CREATE VIEW IF NOT EXISTS verisimdb_users_with_temporal AS
SELECT
    users.id,
    users.username,
    users.email,
    users.created_at,
    tv.version    AS _verisimdb_version,
    tv.valid_from AS _verisimdb_valid_from,
    tv.operation  AS _verisimdb_version_operation
FROM users
LEFT JOIN verisimdb_temporal_versions tv
    ON tv.entity_id = (CAST(users.id AS TEXT))
    AND tv.table_name = 'users'
    AND tv.valid_to IS NULL;

-- Lineage queries for 'users'

-- Upstream: what data was this entity derived from?
-- SELECT * FROM verisimdb_lineage_graph
-- WHERE target_entity = :entity_id AND target_table = 'users';

-- Downstream: what entities depend on this entity?
-- SELECT * FROM verisimdb_lineage_graph
-- WHERE source_entity = :entity_id AND source_table = 'users';

-- Access control filter for 'users'
-- Apply this as a WHERE clause addition to enforce row-level security.
--
-- Example usage (parameterised):
-- SELECT * FROM users
-- WHERE ... AND EXISTS (
--     SELECT 1 FROM verisimdb_access_policies
--     WHERE target_table = 'users'
--     AND principal = :current_principal
--     AND access_level IN ('read', 'admin')
--     AND active = 1
--     AND (condition IS NULL OR :row_matches_condition)
-- );

-- ==========================================================
-- Table: posts
-- ==========================================================

-- Provenance-enriched view for 'posts'
-- Joins each row with its latest provenance entry from the sidecar.
CREATE VIEW IF NOT EXISTS verisimdb_posts_with_provenance AS
SELECT
    posts.id,
    posts.author_id,
    posts.title,
    posts.body,
    posts.published,
    posts.created_at,
    prov.operation   AS _verisimdb_last_operation,
    prov.actor       AS _verisimdb_last_actor,
    prov.timestamp   AS _verisimdb_last_modified,
    prov.hash        AS _verisimdb_provenance_hash
FROM posts
LEFT JOIN (
    SELECT entity_id, operation, actor, timestamp, hash
    FROM (
        SELECT entity_id, operation, actor, timestamp, hash,
               ROW_NUMBER() OVER (PARTITION BY entity_id ORDER BY timestamp DESC) AS _rn
        FROM verisimdb_provenance_log
        WHERE table_name = 'posts'
    ) ranked
    WHERE _rn = 1
) prov ON prov.entity_id = (CAST(posts.id AS TEXT));

-- Temporal-enriched view for 'posts'
-- Joins each row with its current version metadata.
CREATE VIEW IF NOT EXISTS verisimdb_posts_with_temporal AS
SELECT
    posts.id,
    posts.author_id,
    posts.title,
    posts.body,
    posts.published,
    posts.created_at,
    tv.version    AS _verisimdb_version,
    tv.valid_from AS _verisimdb_valid_from,
    tv.operation  AS _verisimdb_version_operation
FROM posts
LEFT JOIN verisimdb_temporal_versions tv
    ON tv.entity_id = (CAST(posts.id AS TEXT))
    AND tv.table_name = 'posts'
    AND tv.valid_to IS NULL;

-- Lineage queries for 'posts'

-- Upstream: what data was this entity derived from?
-- SELECT * FROM verisimdb_lineage_graph
-- WHERE target_entity = :entity_id AND target_table = 'posts';

-- Downstream: what entities depend on this entity?
-- SELECT * FROM verisimdb_lineage_graph
-- WHERE source_entity = :entity_id AND source_table = 'posts';

-- Access control filter for 'posts'
-- Apply this as a WHERE clause addition to enforce row-level security.
--
-- Example usage (parameterised):
-- SELECT * FROM posts
-- WHERE ... AND EXISTS (
--     SELECT 1 FROM verisimdb_access_policies
--     WHERE target_table = 'posts'
--     AND principal = :current_principal
--     AND access_level IN ('read', 'admin')
--     AND active = 1
--     AND (condition IS NULL OR :row_matches_condition)
-- );

