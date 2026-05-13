// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Tier 1 provenance write-path helpers.
//
// Type definitions live in `crate::abi` — this module exists for the
// *write-path* code (V-L1-C1 onwards: hooking the target database,
// appending tamper-evident records to the sidecar). The duplicate
// `ProvenanceRecord` struct that previously lived here was removed
// in V-L2-N1 (it shadowed `abi::ProvenanceEntry` and risked drifting
// from the canonical hash function).
//
// Re-export the canonical type so existing `use crate::tier1::provenance::…`
// call sites continue to work.

pub use crate::abi::ProvenanceEntry;

// Write-path helpers (V-L2-L1) will land here.
