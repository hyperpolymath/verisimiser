#![forbid(unsafe_code)]
#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::manual_strip,
    clippy::if_same_then_else,
    clippy::vec_init_then_push,
    clippy::upper_case_acronyms,
    clippy::format_in_format_args,
    clippy::enum_variant_names,
    clippy::module_inception,
    clippy::doc_lazy_continuation,
    clippy::manual_clamp,
    clippy::type_complexity
)]
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

pub use manifest::{Manifest, load_manifest};
