-- SPDX-License-Identifier: MPL-2.0
-- Frozen golden schema for the codegen-drift check in provable.yml.
--
-- Two tables with a typical author relationship — small enough that the
-- generated sidecar overlay + interceptors stay reviewable in a diff, but
-- enough to exercise multiple column types and more than one table.

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username VARCHAR(64) NOT NULL,
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
