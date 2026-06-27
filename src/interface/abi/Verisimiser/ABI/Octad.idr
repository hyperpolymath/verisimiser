-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Semantic octad-model invariants for VeriSimiser.
|||
||| `Proofs.idr` checks the octad only shallowly (`length allDimensions = 8`,
||| a couple of tag values). This module proves the *defining* invariants of the
||| VeriSimDB octad model as genuine theorems:
|||
|||   1. Octad ≅ Fin 8 — the eight dimensions are exactly eight, all distinct and
|||      with no gaps: a full bijection (both round-trips), not just a count.
|||   2. DriftCategory ≅ OctadDimension — the asserted "drift categories biject
|||      onto octad dimensions" proven as a real bijection.
|||   3. Sidecar isolation — the per-dimension write model agrees with the tier
|||      classification: every Tier-1 (piggyback) dimension provably never writes
|||      to the target database, and every Tier-2 (overlay) dimension does.

module Verisimiser.ABI.Octad

import Verisimiser.ABI.Types
import Data.Fin
import Data.Vect

%default total

--------------------------------------------------------------------------------
-- 1. Octad ≅ Fin 8  (exactly eight distinct dimensions, no gaps)
--------------------------------------------------------------------------------

||| Ordinal position of each octad dimension (matches `octadToInt`).
public export
octadToFin : OctadDimension -> Fin 8
octadToFin Data          = 0
octadToFin Metadata      = 1
octadToFin Provenance    = 2
octadToFin Lineage       = 3
octadToFin Constraints   = 4
octadToFin AccessControl = 5
octadToFin Temporal      = 6
octadToFin Simulation    = 7

||| Inverse: recover the dimension from its ordinal.
public export
octadFromFin : Fin 8 -> OctadDimension
octadFromFin FZ                                    = Data
octadFromFin (FS FZ)                               = Metadata
octadFromFin (FS (FS FZ))                          = Provenance
octadFromFin (FS (FS (FS FZ)))                     = Lineage
octadFromFin (FS (FS (FS (FS FZ))))                = Constraints
octadFromFin (FS (FS (FS (FS (FS FZ)))))           = AccessControl
octadFromFin (FS (FS (FS (FS (FS (FS FZ))))))      = Temporal
octadFromFin (FS (FS (FS (FS (FS (FS (FS FZ))))))) = Simulation

||| Round-trip 1: every dimension survives ordinal encode/decode (injective).
export
octadFinInverseL : (d : OctadDimension) -> octadFromFin (octadToFin d) = d
octadFinInverseL Data          = Refl
octadFinInverseL Metadata      = Refl
octadFinInverseL Provenance    = Refl
octadFinInverseL Lineage       = Refl
octadFinInverseL Constraints   = Refl
octadFinInverseL AccessControl = Refl
octadFinInverseL Temporal      = Refl
octadFinInverseL Simulation    = Refl

||| Round-trip 2: every ordinal in [0,8) names a dimension (surjective, no gaps).
export
octadFinInverseR : (i : Fin 8) -> octadToFin (octadFromFin i) = i
octadFinInverseR FZ                                    = Refl
octadFinInverseR (FS FZ)                               = Refl
octadFinInverseR (FS (FS FZ))                          = Refl
octadFinInverseR (FS (FS (FS FZ)))                     = Refl
octadFinInverseR (FS (FS (FS (FS FZ))))                = Refl
octadFinInverseR (FS (FS (FS (FS (FS FZ)))))           = Refl
octadFinInverseR (FS (FS (FS (FS (FS (FS FZ))))))      = Refl
octadFinInverseR (FS (FS (FS (FS (FS (FS (FS FZ))))))) = Refl

--------------------------------------------------------------------------------
-- 2. DriftCategory ≅ OctadDimension  (the asserted bijection, made real)
--------------------------------------------------------------------------------

||| Each drift category detects inconsistency in exactly one octad dimension,
||| paired by ordinal.
public export
driftToDim : DriftCategory -> OctadDimension
driftToDim Structural      = Data
driftToDim SemanticDrift   = Metadata
driftToDim TemporalDrift   = Provenance
driftToDim Statistical     = Lineage
driftToDim Referential     = Constraints
driftToDim ProvenanceDrift = AccessControl
driftToDim SpatialDrift    = Temporal
driftToDim EmbeddingDrift  = Simulation

||| Inverse pairing.
public export
dimToDrift : OctadDimension -> DriftCategory
dimToDrift Data          = Structural
dimToDrift Metadata      = SemanticDrift
dimToDrift Provenance    = TemporalDrift
dimToDrift Lineage       = Statistical
dimToDrift Constraints   = Referential
dimToDrift AccessControl = ProvenanceDrift
dimToDrift Temporal      = SpatialDrift
dimToDrift Simulation    = EmbeddingDrift

||| The drift↔octad correspondence is a genuine bijection (round-trip 1).
export
driftDimInverseL : (c : DriftCategory) -> dimToDrift (driftToDim c) = c
driftDimInverseL Structural      = Refl
driftDimInverseL SemanticDrift   = Refl
driftDimInverseL TemporalDrift   = Refl
driftDimInverseL Statistical     = Refl
driftDimInverseL Referential     = Refl
driftDimInverseL ProvenanceDrift = Refl
driftDimInverseL SpatialDrift    = Refl
driftDimInverseL EmbeddingDrift  = Refl

||| …and round-trip 2.
export
driftDimInverseR : (d : OctadDimension) -> driftToDim (dimToDrift d) = d
driftDimInverseR Data          = Refl
driftDimInverseR Metadata      = Refl
driftDimInverseR Provenance    = Refl
driftDimInverseR Lineage       = Refl
driftDimInverseR Constraints   = Refl
driftDimInverseR AccessControl = Refl
driftDimInverseR Temporal      = Refl
driftDimInverseR Simulation    = Refl

--------------------------------------------------------------------------------
-- 3. Sidecar isolation: the write model agrees with the tier classification
--------------------------------------------------------------------------------

||| Whether augmenting a given octad dimension writes to the *target* database.
||| Defined per-dimension (independently of `dimensionTier`): the three
||| read-path/sidecar dimensions never touch the target; the overlay dimensions
||| add storage alongside it.
public export
writesTarget : OctadDimension -> Bool
writesTarget Provenance  = False  -- piggyback: append-only provenance sidecar
writesTarget Temporal    = False  -- piggyback: read-path temporal snapshots
writesTarget Constraints = False  -- piggyback: read-path drift observation
writesTarget Data          = True
writesTarget Metadata      = True
writesTarget Lineage       = True
writesTarget AccessControl = True
writesTarget Simulation    = True

||| SIDECAR ISOLATION (the core safety guarantee): every Tier-1 (piggyback)
||| dimension provably never writes to the target database. This proves the
||| independently-defined write model is *consistent* with the tier model — a
||| real cross-check, not a tautology.
export
tier1NeverWritesTarget : (d : OctadDimension) ->
                         dimensionTier d = Tier1 -> writesTarget d = False
tier1NeverWritesTarget Provenance    _ = Refl
tier1NeverWritesTarget Temporal      _ = Refl
tier1NeverWritesTarget Constraints   _ = Refl
tier1NeverWritesTarget Data          Refl impossible
tier1NeverWritesTarget Metadata      Refl impossible
tier1NeverWritesTarget Lineage       Refl impossible
tier1NeverWritesTarget AccessControl Refl impossible
tier1NeverWritesTarget Simulation    Refl impossible

||| Dual: every Tier-2 (overlay) dimension does write to the target — so the
||| isolation above is not vacuous (the two tiers genuinely partition by write
||| behaviour).
export
tier2WritesTarget : (d : OctadDimension) ->
                    dimensionTier d = Tier2 -> writesTarget d = True
tier2WritesTarget Data          _ = Refl
tier2WritesTarget Metadata      _ = Refl
tier2WritesTarget Lineage       _ = Refl
tier2WritesTarget AccessControl _ = Refl
tier2WritesTarget Simulation    _ = Refl
tier2WritesTarget Provenance    Refl impossible
tier2WritesTarget Temporal      Refl impossible
tier2WritesTarget Constraints   Refl impossible

--------------------------------------------------------------------------------
-- Negative controls (the invariants are non-vacuous)
--------------------------------------------------------------------------------

||| Distinct dimensions are genuinely distinct — the ordinal tagging cannot
||| collide two dimensions onto one slot.
export
dataNotMetadata : Not (Data = Metadata)
dataNotMetadata Refl impossible

||| Sidecar isolation has real content: at least one dimension *does* write to
||| the target, so `writesTarget` is not constantly `False`.
export
dataDoesWrite : writesTarget Data = True
dataDoesWrite = Refl
