<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->
<!-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk> -->
# TOPOLOGY.md — verisimiser

## Purpose

verisimiser augments any existing database with the full VeriSimDB octad of capabilities (the eight cross-modal dimensions). It reads the target database schema and a `verisimiser.toml` manifest, then generates a sidecar overlay, query interceptors, drift detection, provenance chains, and temporal version history — without requiring schema migrations. verisimiser is priority #3 in the -iser family and is the primary path for adding VeriSimDB capabilities to legacy PostgreSQL, SQLite, or MongoDB deployments.

## Module Map

```
verisimiser/
├── src/
│   ├── main.rs                    # CLI entry point (clap): init, generate, start, drift, provenance, history, status, octad
│   ├── lib.rs                     # Library API
│   ├── manifest/mod.rs            # verisimiser.toml parser
│   ├── codegen/mod.rs             # Sidecar overlay and query interceptor generation
│   ├── intercept/                 # Query interception layer
│   ├── tier1/                     # Tier-1 octad dimension modules
│   ├── tier2/                     # Tier-2 octad dimension modules
│   └── abi/                       # Idris2 ABI bridge stubs
├── examples/                      # Worked examples
├── verification/                  # Proof harnesses
├── container/                     # Stapeln container ecosystem
└── .machine_readable/             # A2ML metadata
```

## Data Flow

```
verisimiser.toml manifest
        │
   ┌────▼────┐
   │ Manifest │  parse + validate database backend and octad dimension selections
   │  Parser  │
   └────┬────┘
        │  validated augmentation config
   ┌────▼────┐
   │ Analyser │  introspect target database schema, plan sidecar overlay
   └────┬────┘
        │  schema IR + octad plan
   ┌────▼────┐
   │ Codegen  │  emit generated/verisimiser/ (sidecar overlay, query interceptors,
   │          │  provenance chains, temporal history, drift detectors)
   └────┬────┘
        │  VeriSimDB octad augmentation artifacts
   ┌────▼────┐
   │  Daemon  │  start augmentation daemon alongside target database
   └─────────┘
```
