<!-- SPDX-License-Identifier: PMPL-1.0-or-later -->
<!-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk> -->

# VeriSimiser Topology

## Component Map

```
verisimiser/
├── CLI Layer (Rust)
│   ├── src/main.rs           — clap subcommands: init, start, drift, provenance, history, status, octad
│   ├── src/manifest/         — TOML manifest parsing (verisimiser.toml)
│   ├── src/tier1/            — Tier 1 piggyback data types
│   │   ├── drift.rs          — DriftReport, DriftCategory (8 categories)
│   │   ├── provenance.rs     — re-exports abi::ProvenanceEntry; future write-path helpers (V-L1-C1)
│   │   └── temporal.rs       — TemporalVersion, point-in-time snapshots
│   ├── src/tier2/            — Tier 2 overlay stubs (graph, vector, tensor, semantic, document, spatial)
│   ├── src/intercept/        — Per-backend interception strategies
│   └── src/abi/              — Rust-side ABI module
│
├── Verified Interface (Idris2 ABI)
│   ├── src/interface/abi/Types.idr     — OctadDimension, DatabaseBackend, DriftCategory, AccessPolicy, Tier, Result
│   ├── src/interface/abi/Layout.idr    — OctadRecord (80B), ProvenanceEntry (88B), DriftMeasurement (88B), TemporalSnapshot (48B)
│   └── src/interface/abi/Foreign.idr   — FFI declarations: lifecycle, connect, overlay, provenance, temporal, drift, VCL-total
│
├── FFI Bridge (Zig)
│   ├── src/interface/ffi/build.zig             — Shared/static lib build
│   ├── src/interface/ffi/src/main.zig          — C-ABI implementation of all Foreign.idr declarations
│   └── src/interface/ffi/test/integration_test.zig — Integration tests against FFI
│
└── Generated Headers
    └── src/interface/generated/abi/            — Auto-generated C headers from Idris2 ABI
```

## Data Flow

```
Application → writes → Target Database (PostgreSQL / SQLite / MongoDB / Redis / MySQL)
                            │
                    VeriSimiser intercepts (per-backend strategy)
                            │
            ┌───────────────┼───────────────┐
            │               │               │
       Drift Index    Provenance       Temporal
       (Tier 1)       Sidecar          Sidecar
       read-path      (SHA-256         (versioned
       observer       hash chain)      snapshots)
            │               │               │
            └───────────────┼───────────────┘
                            │
                    VCL-total Query Interface
                            │
                    ┌───────┼───────┐
                    │       │       │
               Graph   Vector   Tensor   Semantic   Document   Spatial
               (Tier 2 overlays — additional storage alongside target DB)
```

## Key Invariants

1. **Tier 1 sidecar isolation**: Tier 1 operations NEVER write to the target database.
2. **Provenance append-only**: Hash chain records are immutable once written.
3. **Octad completeness**: All 8 dimensions accounted for (data, metadata, provenance, lineage, constraints, access-control, temporal, simulation).
4. **Drift completeness**: All 8 drift categories covered (structural, semantic, temporal, statistical, referential, provenance, spatial, embedding).
5. **C-ABI compatibility**: All FFI types match between Idris2 declarations and Zig implementations.

## Interception Strategies

| Backend    | Method                                   | Type        |
|------------|------------------------------------------|-------------|
| PostgreSQL | Logical replication / pg_notify / triggers | CDC         |
| SQLite     | sqlite3_update_hook / WAL monitoring     | Hook        |
| MongoDB    | Change streams                           | Stream      |
| Redis      | Keyspace notifications                   | PubSub      |
| MySQL      | Binlog CDC / triggers                    | CDC         |
| App-level  | Middleware / ORM hooks                   | Interceptor |

## Memory Layouts (from Layout.idr)

| Struct              | Size (bytes) | Alignment | Purpose                              |
|---------------------|-------------|-----------|--------------------------------------|
| OctadRecord         | 80          | 8         | Per-entity octad dimension pointers  |
| ProvenanceEntry     | 88          | 8         | SHA-256 hash chain link              |
| DriftMeasurement    | 88          | 8         | Per-entity drift scores (8 categories) |
| TemporalSnapshot    | 48          | 8         | Versioned entity state               |

## Dependencies

- **VeriSimDB** (nextgen-databases): Source of the octad model. VeriSimiser is a gateway to full VeriSimDB adoption.
- **TypedQLiser**: Compile-time query type checking. Works alongside VeriSimiser for formally verified queries.
- **SqueakWell**: Database recovery via cross-modal constraint propagation. Uses VeriSimiser's drift detection.
- **proven**: Shared Idris2 verified library for formal proofs.
- **TypeLL**: Type theory engine used by TypedQLiser.
