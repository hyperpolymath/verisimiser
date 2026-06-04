<!--
SPDX-License-Identifier: MPL-2.0
Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
-->
# Proof Requirements

<!-- Created 2026-05-18 by estate proof-debt audit. -->

## Current state (2026-05-18)

- Rust (21 `.rs`, 1 crate, `build.rs`) + Idris2 ABI (3 `.idr` under `container/`).
- Classified **safety-critical** in the Rust/SPARK audit.
- Rust/SPARK tier: **DESIGNED-ONLY** — Idris2-ABI seam present, no SPARK
  modules, **no documented stance** (silent-regress risk for a
  safety-critical repo).
- Idris2 escape-hatch grep: clean; 6 `?`-tokens flagged, consistent with
  `Maybe`/query syntax (not unsolved holes).

## What needs proving

1. **Stance documentation (P1)** — explicitly state this repo is designed to
   admit SPARK/Ada for correctness-critical paths via Idris2-ABI / Zig-FFI.
   Without it, CI cannot enforce the discipline and it can drift to bare Rust.
2. Audit `container/*.idr` ABI modules: real contracts vs template scaffold.
3. Identify the correctness-critical core (simulation/verification kernel) and
   pull it behind the Idris2-ABI seam; candidate for a SPARK module.

## Recommended prover

- **Idris2** for the ABI boundary; SPARK/Ada for the correctness-critical core
  once isolated.

## Priority

**MEDIUM** — safety-critical + no stance doc. Stance doc is a 1-day fix that
removes the regression risk; SPARK core isolation is the larger follow-up.
