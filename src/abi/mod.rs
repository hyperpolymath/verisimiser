// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// ABI module for VeriSimiser.
// Idris2 proofs for:
//   - Drift detection correctness (no false negatives)
//   - Provenance chain integrity (hash chain is append-only and tamper-evident)
//   - Temporal version ordering (versions are totally ordered per entity)
//   - Sidecar isolation (Tier 1 never writes to the target database)
