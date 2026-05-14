// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// SQLite write-path interception.
//
// Wires `sqlite3_update_hook` (via rusqlite's `Connection::update_hook`)
// on the target connection so every INSERT, UPDATE, and DELETE produces
// a provenance entry in the sidecar. The hook fires synchronously
// inside the target's SQL operation; it MUST NOT touch the target
// connection (the docs are explicit about this — re-entrancy hazard),
// only the sidecar.
//
// V-L1-C1 (#46): sqlite3_update_hook + sidecar provenance writer.

use crate::tier1::provenance::append_provenance;
use rusqlite::hooks::Action;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Type alias for a per-call entity-id resolver. Given `(table, rowid)`
/// returns the logical entity-id to record. Default behaviour stringifies
/// the rowid — appropriate for tables that use SQLite's implicit rowid
/// as their primary key. Tables with a logical PK column should supply
/// a custom resolver that runs a `SELECT` against the row.
pub type EntityIdResolver = Arc<dyn Fn(&str, i64) -> String + Send + Sync + 'static>;

/// Builder + lifecycle wrapper for the SQLite provenance interceptor.
///
/// The interceptor owns the sidecar connection (shared behind an
/// `Arc<Mutex>`) and an `actor` label that's stamped onto every
/// provenance entry it writes. `install` is the moment the
/// `sqlite3_update_hook` is registered on the target connection; the
/// hook stays installed for the lifetime of that connection (rusqlite
/// dropping the target removes the hook).
pub struct SqliteInterceptor {
    sidecar: Arc<Mutex<Connection>>,
    actor: String,
    resolver: EntityIdResolver,
}

impl SqliteInterceptor {
    /// Create a new interceptor backed by `sidecar`. The caller must
    /// have already called `tier1::provenance::init_sidecar_schema`
    /// against this connection.
    pub fn new(sidecar: Connection, actor: impl Into<String>) -> Self {
        Self {
            sidecar: Arc::new(Mutex::new(sidecar)),
            actor: actor.into(),
            resolver: Arc::new(|_table, rowid| rowid.to_string()),
        }
    }

    /// Replace the default rowid-stringifying resolver with a custom
    /// callable. Useful when the target tables carry an explicit
    /// logical entity-id column that should appear in the provenance
    /// log instead of the rowid.
    pub fn with_resolver(mut self, resolver: EntityIdResolver) -> Self {
        self.resolver = resolver;
        self
    }

    /// Install the update hook on `target`. After this returns, every
    /// INSERT, UPDATE, and DELETE on `target` produces a provenance
    /// entry in the sidecar (with the current `actor` label and the
    /// configured entity-id resolver).
    ///
    /// The target connection itself is NOT written to — the sidecar
    /// path holds the only mutation. This is the V-L1-C1 invariant
    /// the integration test enforces (see `tests/intercept_*.rs`).
    pub fn install(&self, target: &Connection) {
        let sidecar = Arc::clone(&self.sidecar);
        let actor = self.actor.clone();
        let resolver = Arc::clone(&self.resolver);
        target.update_hook(Some(move |action: Action, _db: &str, table: &str, rowid: i64| {
            let op = match action {
                Action::SQLITE_INSERT => "insert",
                Action::SQLITE_UPDATE => "update",
                Action::SQLITE_DELETE => "delete",
                _ => return, // unknown action — skip
            };
            let entity_id = resolver(table, rowid);

            // Lock the sidecar and append. We swallow errors here
            // because the hook is invoked from inside SQLite's
            // transaction machinery — a panic could destabilise the
            // target connection. Errors are observable later via
            // `verify_chain` returning Ok(false) or by inspecting
            // the sidecar log.
            if let Ok(mut conn) = sidecar.lock() {
                let _ = append_provenance(
                    &mut conn,
                    &entity_id,
                    table,
                    op,
                    &actor,
                    None,
                    None,
                );
            }
        }));
    }

    /// Borrow the sidecar connection for read-only queries (e.g.
    /// `verify_chain`). Intended for tests and tooling; production
    /// callers should access the sidecar through the canonical
    /// `tier1::provenance` API instead.
    pub fn sidecar(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.sidecar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier1::provenance::{init_sidecar_schema, verify_chain};
    use rusqlite::params;

    fn fresh_target() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);",
        )
        .unwrap();
        conn
    }

    fn fresh_sidecar() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_sidecar_schema(&conn).unwrap();
        conn
    }

    /// Inserting a row in the target produces an `insert` provenance
    /// entry in the sidecar with the right table_name and operation.
    #[test]
    fn target_insert_produces_sidecar_provenance_entry() {
        let target = fresh_target();
        let interceptor = SqliteInterceptor::new(fresh_sidecar(), "test-actor");
        interceptor.install(&target);

        target
            .execute(
                "INSERT INTO users (id, name, email) VALUES (?1, ?2, ?3)",
                params![1i64, "Alice", "alice@example.org"],
            )
            .unwrap();

        let sidecar = interceptor.sidecar();
        let conn = sidecar.lock().unwrap();
        let (table_name, operation, actor): (String, String, String) = conn
            .query_row(
                "SELECT table_name, operation, actor FROM verisimdb_provenance_log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(table_name, "users");
        assert_eq!(operation, "insert");
        assert_eq!(actor, "test-actor");
    }

    /// Update + delete on the same row each produce their own
    /// provenance entry, and the chain for that rowid verifies.
    #[test]
    fn update_and_delete_produce_chained_entries() {
        let target = fresh_target();
        let interceptor = SqliteInterceptor::new(fresh_sidecar(), "test-actor");
        interceptor.install(&target);

        target
            .execute(
                "INSERT INTO users (id, name) VALUES (?1, ?2)",
                params![1i64, "Alice"],
            )
            .unwrap();
        target
            .execute("UPDATE users SET name = ?1 WHERE id = ?2", params!["Alicia", 1i64])
            .unwrap();
        target
            .execute("DELETE FROM users WHERE id = ?1", params![1i64])
            .unwrap();

        let sidecar = interceptor.sidecar();
        let conn = sidecar.lock().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM verisimdb_provenance_log WHERE entity_id = '1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3, "expected one entry each for insert/update/delete");

        assert!(
            verify_chain(&conn, "1").unwrap(),
            "three-entry chain must verify"
        );
    }

    /// The target database is NEVER written to by the interceptor —
    /// only its own application writes appear. The sidecar carries
    /// all the provenance state. This is the V-L1-C1 isolation
    /// invariant.
    #[test]
    fn target_database_is_not_modified_by_the_hook() {
        let target = fresh_target();
        let interceptor = SqliteInterceptor::new(fresh_sidecar(), "test-actor");
        interceptor.install(&target);

        target
            .execute(
                "INSERT INTO users (id, name) VALUES (?1, ?2)",
                params![1i64, "Alice"],
            )
            .unwrap();
        target
            .execute(
                "INSERT INTO users (id, name) VALUES (?1, ?2)",
                params![2i64, "Bob"],
            )
            .unwrap();

        // The target has exactly its own 2 inserts and no
        // verisimdb_* tables: the interceptor never wrote back.
        let user_count: i64 = target
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_count, 2);

        let leaked: i64 = target
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE name LIKE 'verisimdb_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            leaked, 0,
            "verisimdb_* tables must NOT appear on the target — sidecar isolation broken"
        );
    }

    /// A custom `entity_id` resolver replaces the rowid default. Here
    /// we route to a fixed string so the test asserts the resolver is
    /// consulted; production resolvers would do a `SELECT` against the
    /// target row to fetch a logical PK column.
    #[test]
    fn custom_resolver_overrides_rowid_default() {
        let target = fresh_target();
        let resolver: EntityIdResolver =
            Arc::new(|table, rowid| format!("{table}#{rowid}"));
        let interceptor = SqliteInterceptor::new(fresh_sidecar(), "test-actor")
            .with_resolver(resolver);
        interceptor.install(&target);

        target
            .execute("INSERT INTO users (id, name) VALUES (?1, ?2)", params![1i64, "Alice"])
            .unwrap();

        let sidecar = interceptor.sidecar();
        let conn = sidecar.lock().unwrap();
        let entity_id: String = conn
            .query_row(
                "SELECT entity_id FROM verisimdb_provenance_log",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(entity_id, "users#1");
    }
}
