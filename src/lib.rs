// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
#![forbid(unsafe_code)]
//
// VeriSimiser library crate.
//
// Re-exports the core modules for use as a library (e.g., by integration tests
// or downstream crates). The binary crate (main.rs) uses these modules directly.

pub mod abi;
pub mod codegen;
pub mod doctor;
pub mod gc;
pub mod intercept;
pub mod manifest;
pub mod tier1;
pub mod tier2;

pub use manifest::{Manifest, load_manifest};
