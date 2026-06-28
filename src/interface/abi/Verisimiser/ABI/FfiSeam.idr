-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Layer-4 proof: SEALING the ABI<->FFI seam for VeriSimiser.
|||
||| The structural gate (scripts/abi-ffi-gate.py) checks that the Idris and Zig
||| result-code enums agree by name+value. This module supplies the PROOF-SIDE
||| guarantee that the encoding `resultToInt : Result -> Bits32` is SOUND:
|||
|||   (a) injective  -- distinct ABI outcomes never collide on the wire;
|||   (b) faithfully decodable -- a decoder `intToResult` round-trips every
|||       `Result` back from its C integer (lossless encoding);
|||   (c) the same injectivity for every other FFI enum encoder in Types.
|||
||| Injectivity for `Result` is DERIVED from the round-trip via `justInj`
||| + `cong`, the cleanest route: if `intToResult (resultToInt r)` reduces to
||| `Just r` definitionally, then equal ints force equal decodes force equal
||| Results. The decoder is built with boolean `==` on concrete `Bits32`
||| literals (which reduces) so the round-trip `Refl`s check.
|||
||| Positive controls: concrete decodes by `Refl`.
||| Negative / non-vacuity control: two distinct codes have distinct ints,
||| machine-checked.

module Verisimiser.ABI.FfiSeam

import Verisimiser.ABI.Types

%default total

--------------------------------------------------------------------------------
-- Faithful decoder for Result (boolean == on literals so it reduces)
--------------------------------------------------------------------------------

||| Decode a C integer back into a Result. Total: unknown codes -> Nothing.
||| Built with `if x == k` on concrete `Bits32` literals; the boolean `==`
||| reduces on concrete constants, so `intToResult (resultToInt r)` reduces to
||| `Just r` definitionally and the round-trip `Refl`s below type-check.
public export
intToResult : Bits32 -> Maybe Result
intToResult x =
  if      x == 0 then Just Ok
  else if x == 1 then Just Error
  else if x == 2 then Just InvalidParam
  else if x == 3 then Just OutOfMemory
  else if x == 4 then Just NullPointer
  else if x == 5 then Just ConnectionFailed
  else if x == 6 then Just ChainCorrupted
  else if x == 7 then Just SidecarUnavailable
  else Nothing

||| (b) Faithful / lossless: every Result round-trips through its C integer.
public export
resultRoundTrip : (r : Result) -> intToResult (resultToInt r) = Just r
resultRoundTrip Ok                 = Refl
resultRoundTrip Error              = Refl
resultRoundTrip InvalidParam       = Refl
resultRoundTrip OutOfMemory        = Refl
resultRoundTrip NullPointer        = Refl
resultRoundTrip ConnectionFailed   = Refl
resultRoundTrip ChainCorrupted     = Refl
resultRoundTrip SidecarUnavailable = Refl

--------------------------------------------------------------------------------
-- (a) Injectivity of resultToInt, DERIVED from the round-trip
--------------------------------------------------------------------------------

||| Local injectivity of `Just`: matching `Refl` unifies the two payloads.
||| (The Prelude/base only provide the `Injective Just` interface; this small
||| pure-pattern helper avoids any interface-resolution noise.)
justInj : {0 x, y : a} -> Just x = Just y -> x = y
justInj Refl = Refl

||| (a) The encoding is unambiguous: equal wire integers come from equal
||| Results. Derived from `resultRoundTrip` via `cong` + `justInj`:
||| from `resultToInt a = resultToInt b` we get
||| `intToResult (resultToInt a) = intToResult (resultToInt b)` (cong), i.e.
||| `Just a = Just b` (round-trip both sides), then `a = b` (justInj).
public export
resultToIntInjective : (a, b : Result)
                    -> resultToInt a = resultToInt b
                    -> a = b
resultToIntInjective a b prf =
  justInj $
    trans (sym (resultRoundTrip a)) $
    trans (cong intToResult prf) (resultRoundTrip b)

--------------------------------------------------------------------------------
-- Positive controls (concrete decodes, by Refl)
--------------------------------------------------------------------------------

||| Decoding 0 yields Ok.
public export
decodeOk : intToResult 0 = Just Ok
decodeOk = Refl

||| Decoding 7 yields SidecarUnavailable (the top of the range).
public export
decodeSidecar : intToResult 7 = Just SidecarUnavailable
decodeSidecar = Refl

||| Decoding an out-of-range code yields Nothing.
public export
decodeUnknown : intToResult 99 = Nothing
decodeUnknown = Refl

--------------------------------------------------------------------------------
-- Negative / non-vacuity control (distinct codes -> distinct ints)
--------------------------------------------------------------------------------

||| Distinct primitive Bits32 literals are provably unequal; the coverage
||| checker discharges `Refl impossible` for distinct primitive constants.
||| This proves the seam is NON-VACUOUS: Ok and Error really do encode
||| differently on the wire, so injectivity has content.
public export
okNotError : Not (resultToInt Ok = resultToInt Error)
okNotError Refl impossible

||| A second non-vacuity witness across a wider gap.
public export
okNotSidecar : Not (resultToInt Ok = resultToInt SidecarUnavailable)
okNotSidecar Refl impossible

--------------------------------------------------------------------------------
-- (c) Same injectivity for the other FFI enum encoders in Types
--------------------------------------------------------------------------------
-- VeriSimiser's Types has no ProofStatus/statusToInt, but it defines five
-- further FFI enum encoders that cross the same C-ABI seam. Each is proven
-- injective DIRECTLY: nested case on both arguments, diagonal = Refl,
-- off-diagonal refuted because the two int literals differ
-- (\case Refl impossible on the literal-equality hypothesis).

||| OctadDimension -> Bits32 is injective (8 tags, 0..7).
public export
octadToIntInjective : (a, b : OctadDimension)
                   -> octadToInt a = octadToInt b
                   -> a = b
octadToIntInjective Data          Data          _   = Refl
octadToIntInjective Metadata      Metadata      _   = Refl
octadToIntInjective Provenance    Provenance    _   = Refl
octadToIntInjective Lineage       Lineage       _   = Refl
octadToIntInjective Constraints   Constraints   _   = Refl
octadToIntInjective AccessControl AccessControl _   = Refl
octadToIntInjective Temporal      Temporal      _   = Refl
octadToIntInjective Simulation    Simulation    _   = Refl
octadToIntInjective Data          Metadata      prf = case prf of Refl impossible
octadToIntInjective Data          Provenance    prf = case prf of Refl impossible
octadToIntInjective Data          Lineage       prf = case prf of Refl impossible
octadToIntInjective Data          Constraints   prf = case prf of Refl impossible
octadToIntInjective Data          AccessControl prf = case prf of Refl impossible
octadToIntInjective Data          Temporal      prf = case prf of Refl impossible
octadToIntInjective Data          Simulation    prf = case prf of Refl impossible
octadToIntInjective Metadata      Data          prf = case prf of Refl impossible
octadToIntInjective Metadata      Provenance    prf = case prf of Refl impossible
octadToIntInjective Metadata      Lineage       prf = case prf of Refl impossible
octadToIntInjective Metadata      Constraints   prf = case prf of Refl impossible
octadToIntInjective Metadata      AccessControl prf = case prf of Refl impossible
octadToIntInjective Metadata      Temporal      prf = case prf of Refl impossible
octadToIntInjective Metadata      Simulation    prf = case prf of Refl impossible
octadToIntInjective Provenance    Data          prf = case prf of Refl impossible
octadToIntInjective Provenance    Metadata      prf = case prf of Refl impossible
octadToIntInjective Provenance    Lineage       prf = case prf of Refl impossible
octadToIntInjective Provenance    Constraints   prf = case prf of Refl impossible
octadToIntInjective Provenance    AccessControl prf = case prf of Refl impossible
octadToIntInjective Provenance    Temporal      prf = case prf of Refl impossible
octadToIntInjective Provenance    Simulation    prf = case prf of Refl impossible
octadToIntInjective Lineage       Data          prf = case prf of Refl impossible
octadToIntInjective Lineage       Metadata      prf = case prf of Refl impossible
octadToIntInjective Lineage       Provenance    prf = case prf of Refl impossible
octadToIntInjective Lineage       Constraints   prf = case prf of Refl impossible
octadToIntInjective Lineage       AccessControl prf = case prf of Refl impossible
octadToIntInjective Lineage       Temporal      prf = case prf of Refl impossible
octadToIntInjective Lineage       Simulation    prf = case prf of Refl impossible
octadToIntInjective Constraints   Data          prf = case prf of Refl impossible
octadToIntInjective Constraints   Metadata      prf = case prf of Refl impossible
octadToIntInjective Constraints   Provenance    prf = case prf of Refl impossible
octadToIntInjective Constraints   Lineage       prf = case prf of Refl impossible
octadToIntInjective Constraints   AccessControl prf = case prf of Refl impossible
octadToIntInjective Constraints   Temporal      prf = case prf of Refl impossible
octadToIntInjective Constraints   Simulation    prf = case prf of Refl impossible
octadToIntInjective AccessControl Data          prf = case prf of Refl impossible
octadToIntInjective AccessControl Metadata      prf = case prf of Refl impossible
octadToIntInjective AccessControl Provenance    prf = case prf of Refl impossible
octadToIntInjective AccessControl Lineage       prf = case prf of Refl impossible
octadToIntInjective AccessControl Constraints   prf = case prf of Refl impossible
octadToIntInjective AccessControl Temporal      prf = case prf of Refl impossible
octadToIntInjective AccessControl Simulation    prf = case prf of Refl impossible
octadToIntInjective Temporal      Data          prf = case prf of Refl impossible
octadToIntInjective Temporal      Metadata      prf = case prf of Refl impossible
octadToIntInjective Temporal      Provenance    prf = case prf of Refl impossible
octadToIntInjective Temporal      Lineage       prf = case prf of Refl impossible
octadToIntInjective Temporal      Constraints   prf = case prf of Refl impossible
octadToIntInjective Temporal      AccessControl prf = case prf of Refl impossible
octadToIntInjective Temporal      Simulation    prf = case prf of Refl impossible
octadToIntInjective Simulation    Data          prf = case prf of Refl impossible
octadToIntInjective Simulation    Metadata      prf = case prf of Refl impossible
octadToIntInjective Simulation    Provenance    prf = case prf of Refl impossible
octadToIntInjective Simulation    Lineage       prf = case prf of Refl impossible
octadToIntInjective Simulation    Constraints   prf = case prf of Refl impossible
octadToIntInjective Simulation    AccessControl prf = case prf of Refl impossible
octadToIntInjective Simulation    Temporal      prf = case prf of Refl impossible

||| DatabaseBackend -> Bits32 is injective (5 tags, 0..4).
public export
backendToIntInjective : (a, b : DatabaseBackend)
                     -> backendToInt a = backendToInt b
                     -> a = b
backendToIntInjective PostgreSQL PostgreSQL _   = Refl
backendToIntInjective SQLite     SQLite     _   = Refl
backendToIntInjective MongoDB    MongoDB    _   = Refl
backendToIntInjective Redis      Redis      _   = Refl
backendToIntInjective MySQL      MySQL      _   = Refl
backendToIntInjective PostgreSQL SQLite     prf = case prf of Refl impossible
backendToIntInjective PostgreSQL MongoDB    prf = case prf of Refl impossible
backendToIntInjective PostgreSQL Redis      prf = case prf of Refl impossible
backendToIntInjective PostgreSQL MySQL      prf = case prf of Refl impossible
backendToIntInjective SQLite     PostgreSQL prf = case prf of Refl impossible
backendToIntInjective SQLite     MongoDB    prf = case prf of Refl impossible
backendToIntInjective SQLite     Redis      prf = case prf of Refl impossible
backendToIntInjective SQLite     MySQL      prf = case prf of Refl impossible
backendToIntInjective MongoDB    PostgreSQL prf = case prf of Refl impossible
backendToIntInjective MongoDB    SQLite     prf = case prf of Refl impossible
backendToIntInjective MongoDB    Redis      prf = case prf of Refl impossible
backendToIntInjective MongoDB    MySQL      prf = case prf of Refl impossible
backendToIntInjective Redis      PostgreSQL prf = case prf of Refl impossible
backendToIntInjective Redis      SQLite     prf = case prf of Refl impossible
backendToIntInjective Redis      MongoDB    prf = case prf of Refl impossible
backendToIntInjective Redis      MySQL      prf = case prf of Refl impossible
backendToIntInjective MySQL      PostgreSQL prf = case prf of Refl impossible
backendToIntInjective MySQL      SQLite     prf = case prf of Refl impossible
backendToIntInjective MySQL      MongoDB    prf = case prf of Refl impossible
backendToIntInjective MySQL      Redis      prf = case prf of Refl impossible

||| ProvenanceOperation -> Bits32 is injective (4 tags, 0..3).
public export
provenanceOpToIntInjective : (a, b : ProvenanceOperation)
                          -> provenanceOpToInt a = provenanceOpToInt b
                          -> a = b
provenanceOpToIntInjective Create    Create    _   = Refl
provenanceOpToIntInjective Update    Update    _   = Refl
provenanceOpToIntInjective Delete    Delete    _   = Refl
provenanceOpToIntInjective Transform Transform _   = Refl
provenanceOpToIntInjective Create    Update    prf = case prf of Refl impossible
provenanceOpToIntInjective Create    Delete    prf = case prf of Refl impossible
provenanceOpToIntInjective Create    Transform prf = case prf of Refl impossible
provenanceOpToIntInjective Update    Create    prf = case prf of Refl impossible
provenanceOpToIntInjective Update    Delete    prf = case prf of Refl impossible
provenanceOpToIntInjective Update    Transform prf = case prf of Refl impossible
provenanceOpToIntInjective Delete    Create    prf = case prf of Refl impossible
provenanceOpToIntInjective Delete    Update    prf = case prf of Refl impossible
provenanceOpToIntInjective Delete    Transform prf = case prf of Refl impossible
provenanceOpToIntInjective Transform Create    prf = case prf of Refl impossible
provenanceOpToIntInjective Transform Update    prf = case prf of Refl impossible
provenanceOpToIntInjective Transform Delete    prf = case prf of Refl impossible

||| DriftCategory -> Bits32 is injective (8 tags, 0..7).
public export
driftToIntInjective : (a, b : DriftCategory)
                   -> driftToInt a = driftToInt b
                   -> a = b
driftToIntInjective Structural      Structural      _   = Refl
driftToIntInjective SemanticDrift   SemanticDrift   _   = Refl
driftToIntInjective TemporalDrift   TemporalDrift   _   = Refl
driftToIntInjective Statistical     Statistical     _   = Refl
driftToIntInjective Referential     Referential     _   = Refl
driftToIntInjective ProvenanceDrift ProvenanceDrift _   = Refl
driftToIntInjective SpatialDrift    SpatialDrift    _   = Refl
driftToIntInjective EmbeddingDrift  EmbeddingDrift  _   = Refl
driftToIntInjective Structural      SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective Structural      TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective Structural      Statistical     prf = case prf of Refl impossible
driftToIntInjective Structural      Referential     prf = case prf of Refl impossible
driftToIntInjective Structural      ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective Structural      SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective Structural      EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   Structural      prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   Statistical     prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   Referential     prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective SemanticDrift   EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   Structural      prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   Statistical     prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   Referential     prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective TemporalDrift   EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective Statistical     Structural      prf = case prf of Refl impossible
driftToIntInjective Statistical     SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective Statistical     TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective Statistical     Referential     prf = case prf of Refl impossible
driftToIntInjective Statistical     ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective Statistical     SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective Statistical     EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective Referential     Structural      prf = case prf of Refl impossible
driftToIntInjective Referential     SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective Referential     TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective Referential     Statistical     prf = case prf of Refl impossible
driftToIntInjective Referential     ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective Referential     SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective Referential     EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift Structural      prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift Statistical     prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift Referential     prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift SpatialDrift    prf = case prf of Refl impossible
driftToIntInjective ProvenanceDrift EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    Structural      prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    Statistical     prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    Referential     prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective SpatialDrift    EmbeddingDrift  prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  Structural      prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  SemanticDrift   prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  TemporalDrift   prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  Statistical     prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  Referential     prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  ProvenanceDrift prf = case prf of Refl impossible
driftToIntInjective EmbeddingDrift  SpatialDrift    prf = case prf of Refl impossible

||| AccessPolicy -> Bits32 is injective (5 tags, 0..4).
public export
accessPolicyToIntInjective : (a, b : AccessPolicy)
                          -> accessPolicyToInt a = accessPolicyToInt b
                          -> a = b
accessPolicyToIntInjective Open          Open          _   = Refl
accessPolicyToIntInjective ReadOnly      ReadOnly      _   = Refl
accessPolicyToIntInjective Authenticated Authenticated _   = Refl
accessPolicyToIntInjective RBAC          RBAC          _   = Refl
accessPolicyToIntInjective Audited       Audited       _   = Refl
accessPolicyToIntInjective Open          ReadOnly      prf = case prf of Refl impossible
accessPolicyToIntInjective Open          Authenticated prf = case prf of Refl impossible
accessPolicyToIntInjective Open          RBAC          prf = case prf of Refl impossible
accessPolicyToIntInjective Open          Audited       prf = case prf of Refl impossible
accessPolicyToIntInjective ReadOnly      Open          prf = case prf of Refl impossible
accessPolicyToIntInjective ReadOnly      Authenticated prf = case prf of Refl impossible
accessPolicyToIntInjective ReadOnly      RBAC          prf = case prf of Refl impossible
accessPolicyToIntInjective ReadOnly      Audited       prf = case prf of Refl impossible
accessPolicyToIntInjective Authenticated Open          prf = case prf of Refl impossible
accessPolicyToIntInjective Authenticated ReadOnly      prf = case prf of Refl impossible
accessPolicyToIntInjective Authenticated RBAC          prf = case prf of Refl impossible
accessPolicyToIntInjective Authenticated Audited       prf = case prf of Refl impossible
accessPolicyToIntInjective RBAC          Open          prf = case prf of Refl impossible
accessPolicyToIntInjective RBAC          ReadOnly      prf = case prf of Refl impossible
accessPolicyToIntInjective RBAC          Authenticated prf = case prf of Refl impossible
accessPolicyToIntInjective RBAC          Audited       prf = case prf of Refl impossible
accessPolicyToIntInjective Audited       Open          prf = case prf of Refl impossible
accessPolicyToIntInjective Audited       ReadOnly      prf = case prf of Refl impossible
accessPolicyToIntInjective Audited       Authenticated prf = case prf of Refl impossible
accessPolicyToIntInjective Audited       RBAC          prf = case prf of Refl impossible
