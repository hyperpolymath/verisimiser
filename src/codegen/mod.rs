// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Code generation module for VeriSimiser.
//
// This module takes a parsed database schema and the octad configuration from
// the manifest, then generates:
//   1. Sidecar overlay tables (provenance_log, lineage_graph, temporal_versions, access_policies)
//   2. Query interceptors that enrich native queries with octad dimension data
//
// Submodules:
//   - parser:  Parse database schema files (SQL DDL) into an intermediate representation
//   - overlay: Generate sidecar schema DDL for enabled octad dimensions
//   - query:   Generate query interceptor SQL for octad enrichment

pub mod ident;
pub mod overlay;
pub mod parser;
pub mod query;
