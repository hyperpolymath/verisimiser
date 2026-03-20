// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Database interception layer.
// Configurable per target database:
//   PostgreSQL: logical replication / pg_notify / triggers
//   MySQL:      binlog CDC / triggers
//   SQLite:     sqlite3_update_hook / WAL monitoring
//   MongoDB:    change streams
//   App-level:  middleware / ORM hooks

// TODO: implement per-database interception strategies
