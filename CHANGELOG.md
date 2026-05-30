<!--
SPDX-License-Identifier: MPL-2.0
SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell (hyperpolymath)
-->

# Changelog

All notable changes to `verisimiser` will be documented in this file.

This file is generated from conventional commits by the
[`changelog-reusable.yml`](https://github.com/hyperpolymath/standards/blob/main/.github/workflows/changelog-reusable.yml)
workflow (`hyperpolymath/standards#206`). Adopt the workflow in this repo's CI to keep this file in sync automatically — see
[`templates/cliff.toml`](https://github.com/hyperpolymath/standards/blob/main/templates/cliff.toml)
for the canonical config.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- feat(codegen): split sidecar DDL by dialect; reject json sidecar (#45) (#133)
- feat(codegen): split sidecar DDL by dialect; reject json sidecar (#45) (#131)
- feat(logging): tracing diagnostics with --log-format/--log-level (#51) (#124)
- feat(codegen): split sidecar DDL by dialect; reject json sidecar (#45) (#129)
- feat(codegen): parse DDL with sqlparser, drop hand-rolled scanner (#38) (#123)
- feat(provenance): fork-first-class chain model — ADR-0010 (#31; supersedes #32) (#122)
- feat(provenance): fork-first-class chain model — ADR-0010 (#31; supersedes #32) (#121)
- feat(provenance): fork-first-class chain model — ADR-0010 (#31; supersedes #32) (#120)
- feat(codegen): split sidecar DDL by dialect; reject json sidecar (#45) (#113)
- feat(provenance): fork-first-class chain model — ADR-0010 (#31; supersedes #32) (#109)

### Removed

- feat(manifest): drop the never-implemented `json` sidecar store; close `[sidecar].storage` to sqlite + postgres and reject unknown values at validate/doctor/generate (V-L2-F2, ADR-0011, closes #112) (#144, #147)

### Fixed

- fix(rhodibot): automated RSR compliance fixes (#135)
- fix: restore sqlparser dependency and security policy compliance
- fix(ci): bump a2ml/k9-validate-action pins to canonical (standards#85) (#114)
- fix(ci): sync hypatia-scan.yml to canonical (kill cd-scanner build drift) (#108)
- fix(ci): remove duplicate top-level concurrency block in rust-ci
- fix(ci): CodeQL language-aware detect (replace hardcoded rust) (#105)
- fix(#26): unify ProvenanceRecord/ProvenanceEntry (#103)
- fix(provenance): domain-separated, length-prefixed hash over all fields (#88)
- fix(manifest): TOML parse errors now report filename:line:col (#84)
- fix(codegen): validate every identifier before interpolation into DDL (#83)

### Changed

- refactor(rename): VQL → VCL + verisimdb → verisim

### Documentation

- docs(readme): rewrite around concerns octad (V-L1-A2) (#99)
- docs(adr): 0009-build-path — cargo + Containerfile canonical; rest experimental (#97)
- docs(adr): 0008-contractile-ci — informational only until a real consumer exists (#96)
- docs(adr): 0007-access-control-model — prefix-typed principals, deny-wins, typed predicate (#95)
- docs(adr): 0006-simulation-semantics — isolated snapshots + manual merge (#94)
- docs(adr): 0005-lineage-acyclicity — enforce DAG at write time (#93)
- docs(adr): 0004-octad-ontology — concerns octad is canonical (#92)
- docs(adr): 0003-drift-categories — input, distance, threshold per category (#91)
- docs(theory): provenance threat model (foundational doc for Step 2) (#90)
- docs: add CONTRIBUTING-DEV.adoc (#74)

### CI

- ci(spark): adopt estate SPARK Theatre Gate (standards#135) (#117)
- ci: redistribute concurrency-cancel guard to read-only check workflows (#116)
- ci(rust): rewrite rust-ci.yml so the workflow actually registers (#89)
- ci: bump actions/upload-artifact SHA to current v4 (#23)
- ci: bump actions/upload-artifact SHA to current v4 (#18)

## Pre-history

Prior commits to this file's introduction are recorded in git history but not formally classified into Keep-a-Changelog sections. To backfill, run `git cliff -o CHANGELOG.md` locally using the canonical [`cliff.toml`](https://github.com/hyperpolymath/standards/blob/main/templates/cliff.toml) — this is one-shot mechanical work.

---

<!-- This file was seeded by the 2026-05-26 estate tech-debt audit follow-up (Row-2 Phase 3); see [`hyperpolymath/standards/docs/audits/2026-05-26-estate-documentation-debt.md`](https://github.com/hyperpolymath/standards/blob/main/docs/audits/2026-05-26-estate-documentation-debt.md). -->
