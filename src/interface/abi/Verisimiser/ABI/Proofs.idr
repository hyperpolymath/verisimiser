-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Machine-checked proofs over the VeriSimiser ABI.
|||
||| These are not runtime tests — they are propositional statements the Idris2
||| type checker must discharge at compile time. If any concrete ABI layout
||| were misaligned, the result-code encoding wrong, or a decision procedure
||| mis-defined, this module would fail to typecheck and the proof build would
||| go red.
|||
||| The C-ABI compliance witnesses are built directly from per-field
||| divisibility proofs (`DivideBy k Refl`, where `offset = k * alignment`).
||| Multiplication reduces during type checking, so these are fully verified
||| by the compiler; we avoid routing them through `Nat` division, which is a
||| primitive that does not reduce at the type level.

module Verisimiser.ABI.Proofs

import Verisimiser.ABI.Types
import Verisimiser.ABI.Layout
import Data.So
import Data.Vect

%default total

--------------------------------------------------------------------------------
-- The concrete FFI struct layouts are provably C-ABI compliant.
--------------------------------------------------------------------------------

||| Every field offset in the OctadRecord layout divides its alignment:
||| 0|8, 8|4, 12|4, 16|8, 24|8, 32|8, 40|8, 48|8, 56|8, 64|8, 72|8.
export
octadRecordCompliant : CABICompliant Layout.octadRecordLayout
octadRecordCompliant =
  CABIOk octadRecordLayout
    (ConsField _ _ (DivideBy 0 Refl)
    (ConsField _ _ (DivideBy 2 Refl)
    (ConsField _ _ (DivideBy 3 Refl)
    (ConsField _ _ (DivideBy 2 Refl)
    (ConsField _ _ (DivideBy 3 Refl)
    (ConsField _ _ (DivideBy 4 Refl)
    (ConsField _ _ (DivideBy 5 Refl)
    (ConsField _ _ (DivideBy 6 Refl)
    (ConsField _ _ (DivideBy 7 Refl)
    (ConsField _ _ (DivideBy 8 Refl)
    (ConsField _ _ (DivideBy 9 Refl)
     NoFields)))))))))))

||| Every field offset in the ProvenanceEntry layout is aligned:
||| 0|1, 32|1, 64|8, 72|4, 76|4, 80|8.
export
provenanceEntryCompliant : CABICompliant Layout.provenanceEntryLayout
provenanceEntryCompliant =
  CABIOk provenanceEntryLayout
    (ConsField _ _ (DivideBy 0 Refl)
    (ConsField _ _ (DivideBy 32 Refl)
    (ConsField _ _ (DivideBy 8 Refl)
    (ConsField _ _ (DivideBy 18 Refl)
    (ConsField _ _ (DivideBy 19 Refl)
    (ConsField _ _ (DivideBy 10 Refl)
     NoFields))))))

||| Every field offset in the DriftMeasurement layout is aligned:
||| 0|8, 8|8, 16|8, 80|8.
export
driftMeasurementCompliant : CABICompliant Layout.driftMeasurementLayout
driftMeasurementCompliant =
  CABIOk driftMeasurementLayout
    (ConsField _ _ (DivideBy 0 Refl)
    (ConsField _ _ (DivideBy 1 Refl)
    (ConsField _ _ (DivideBy 2 Refl)
    (ConsField _ _ (DivideBy 10 Refl)
     NoFields))))

||| Every field offset in the TemporalSnapshot layout is aligned:
||| 0|8, 8|8, 16|8, 24|8, 32|8, 40|4, 44|4.
export
temporalSnapshotCompliant : CABICompliant Layout.temporalSnapshotLayout
temporalSnapshotCompliant =
  CABIOk temporalSnapshotLayout
    (ConsField _ _ (DivideBy 0 Refl)
    (ConsField _ _ (DivideBy 1 Refl)
    (ConsField _ _ (DivideBy 2 Refl)
    (ConsField _ _ (DivideBy 3 Refl)
    (ConsField _ _ (DivideBy 4 Refl)
    (ConsField _ _ (DivideBy 10 Refl)
    (ConsField _ _ (DivideBy 11 Refl)
     NoFields)))))))

--------------------------------------------------------------------------------
-- Result-code round-trip: the encoding the Zig FFI depends on.
--------------------------------------------------------------------------------

export
okIsZero : resultToInt Ok = 0
okIsZero = Refl

export
nullPointerIsFour : resultToInt NullPointer = 4
nullPointerIsFour = Refl

--------------------------------------------------------------------------------
-- Octad model invariants.
--------------------------------------------------------------------------------

||| The octad is exactly eight dimensions — the defining cardinality of the
||| VeriSimDB model.
export
octadCardinality : length Types.allDimensions = 8
octadCardinality = Refl

||| The octad tag encoding the Zig FFI depends on: Simulation is the eighth
||| (index 7) dimension.
export
simulationIsSeven : octadToInt Simulation = 7
simulationIsSeven = Refl

||| Tier 1 (piggyback) capabilities carry a static proof that they never write
||| to the target database — the core safety guarantee of VeriSimiser.
export
provenanceIsPiggyback : dimensionTier Provenance = Tier1
provenanceIsPiggyback = Refl
