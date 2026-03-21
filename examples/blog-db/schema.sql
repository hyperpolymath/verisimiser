-- SPDX-License-Identifier: PMPL-1.0-or-later
-- Example blog database schema for VeriSimiser.
--
-- This schema represents a typical blog application with users, posts,
-- comments, and tags. VeriSimiser augments this schema with octad dimensions
-- (provenance, lineage, temporal versioning, access control) without
-- modifying these tables.

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username VARCHAR(64) NOT NULL,
    email VARCHAR(320) NOT NULL,
    display_name TEXT,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    author_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    slug VARCHAR(255) NOT NULL,
    published BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP
);

CREATE TABLE comments (
    id INTEGER PRIMARY KEY,
    post_id INTEGER NOT NULL,
    author_id INTEGER NOT NULL,
    body TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name VARCHAR(64) NOT NULL
);

CREATE TABLE post_tags (
    post_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (post_id, tag_id)
);
