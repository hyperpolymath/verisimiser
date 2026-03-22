#![forbid(unsafe_code)]
// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// VeriSimiser library crate.
//
// Re-exports the core modules for use as a library (e.g., by integration tests
// or downstream crates). The binary crate (main.rs) uses these modules directly.

pub mod abi;
pub mod codegen;
pub mod intercept;
pub mod manifest;
pub mod tier1;
pub mod tier2;

pub use manifest::{load_manifest, Manifest};
