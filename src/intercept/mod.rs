// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Database interception layer.
// Configurable per target database:
//   PostgreSQL: logical replication / pg_notify / triggers   (TODO)
//   MySQL:      binlog CDC / triggers                        (TODO)
//   SQLite:     sqlite3_update_hook / WAL monitoring         (V-L1-C1, this module)
//   MongoDB:    change streams                               (TODO)
//   App-level:  middleware / ORM hooks                       (TODO)

pub mod sqlite;
