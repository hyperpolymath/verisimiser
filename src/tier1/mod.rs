// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Tier 1: True piggyback capabilities.
// These observe your database without modifying it.
// Storage is in external sidecars, never in your database's tables.

pub mod drift;
pub mod provenance;
pub mod temporal;
