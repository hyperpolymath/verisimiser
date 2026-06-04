<!--
SPDX-License-Identifier: MPL-2.0
Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
-->
# verisimiser — Project Instructions

## Overview

Augment any database with VeriSimDB octad capabilities

**Status:** pre-alpha
**Priority in -iser family:** 3
**Part of:** https://github.com/hyperpolymath/iseriser (-iser ecosystem)

## Architecture

All -iser projects follow the same architecture:
- **Manifest** (`verisimiser.toml`) — user describes WHAT they want
- **Idris2 ABI** (`src/abi/` or `src/interface/abi/`) — formal proofs of interface correctness
- **Zig FFI** (`ffi/zig/` or `src/interface/ffi/`) — C-ABI bridge to target language
- **Codegen** (`src/codegen/`) — generates target language wrapper code
- **Rust CLI** (`src/main.rs`) — orchestrates everything

## Build & Test

```bash
cargo build --release
cargo test
```

## Key Design Decisions

- Follows hyperpolymath ABI-FFI standard (Idris2 ABI, Zig FFI)
- MPL-2.0 license
- RSR (Rhodium Standard Repository) template
- Author: Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

## Integration Points

- **iseriser**: Meta-framework that can generate new -iser scaffolding
- **typedqliser**: #1 priority — formal type safety for query languages
- **chapeliser**: #2 priority — distributed computing acceleration
- **verisimiser**: #3 priority — database octad augmentation
- **squeakwell**: Database recovery via cross-modal constraint propagation
