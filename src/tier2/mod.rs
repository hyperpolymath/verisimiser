// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Tier 2: Augmentation overlays.
// These add additional storage alongside your database.
// Honest framing: these are (a subset of) VeriSimDB's modalities with your
// database as the primary store.
//
// The overlay modality set is fixed at six — graph, vector, tensor, semantic,
// document, spatial — and is NOT the Octad: VeriSimiser's Octad is the eight
// concerns in `crate::abi::OctadDimension`. Upstream's Temporal and Provenance
// modalities are Tier 1 concerns here, not overlays.
//
// `Tier2Config` flags select an enabled subset of those six (cardinality 0..=6).
// That selection is not a versioned "modality profile" — see ADR-0014.
//
// Each overlay is a separate module, enabled independently.
// TODO: graph, vector, tensor, semantic, document, spatial (stubs today; the
// `[tier2]` flags are parsed but no overlay codegen or query routing ships yet).
