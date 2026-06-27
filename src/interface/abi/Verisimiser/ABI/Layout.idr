-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Memory Layout Proofs for VeriSimiser
|||
||| Provides formal proofs about memory layout, alignment, and padding
||| for C-compatible structs used in the VeriSimiser octad augmentation layer.
|||
||| Key layouts:
||| - OctadRecord: the 8-dimension record overlaying a database entity
||| - ProvenanceEntry: a single link in the SHA-256 hash chain
||| - DriftMeasurement: per-entity drift score across modalities
||| - TemporalSnapshot: versioned entity state at a point in time
|||
||| Every concrete layout below carries machine-checked witnesses that
||| (1) its declared size is a multiple of its alignment and is large enough to
|||     hold all of its fields (the `aligned` / `sizeCorrect` invariants of
|||     `StructLayout`), and
||| (2) every field sits at an offset that is a multiple of that field's own
|||     alignment (`FieldsAligned` / `CABICompliant`).
||| These are full proofs: Idris checks them at compile time, no holes.
|||
||| @see https://en.wikipedia.org/wiki/Data_structure_alignment

module Verisimiser.ABI.Layout

import Verisimiser.ABI.Types
import Data.Vect
import Data.Fin
import Data.So

%default total

--------------------------------------------------------------------------------
-- Alignment Utilities
--------------------------------------------------------------------------------

||| Padding needed to bring `offset` up to the next multiple of `alignment`.
public export
paddingFor : (offset : Nat) -> (alignment : Nat) -> Nat
paddingFor offset alignment =
  if offset `mod` alignment == 0
    then 0
    else minus alignment (offset `mod` alignment)

||| Proof that `n` divides `m`: there is a `k` with `m = k * n`.
public export
data Divides : Nat -> Nat -> Type where
  DivideBy : (k : Nat) -> {n : Nat} -> {m : Nat} -> (m = k * n) -> Divides n m

||| Ceiling division: the smallest number of `d`-sized blocks covering `n`.
||| (For the degenerate `d = 0` this is `0`, matching `alignUp _ 0 = 0`.)
public export
ceilDiv : (n : Nat) -> (d : Nat) -> Nat
ceilDiv n d = (n + minus d 1) `div` d

||| Round `size` up to the next multiple of `alignment`.
|||
||| Defined as `ceilDiv size alignment * alignment` so that the result is
||| *manifestly* a multiple of `alignment`; that is exactly what
||| `alignUpCorrect` certifies, with no auxiliary arithmetic lemmas required.
public export
alignUp : (size : Nat) -> (alignment : Nat) -> Nat
alignUp size alignment = ceilDiv size alignment * alignment

||| Proof that `alignUp size align` is always a multiple of `align`.
||| Unconditional — it holds even for the degenerate `align = 0` (both sides 0).
public export
alignUpCorrect : (size : Nat) -> (align : Nat) -> Divides align (alignUp size align)
alignUpCorrect size align = DivideBy (ceilDiv size align) Refl

--------------------------------------------------------------------------------
-- Struct Field Layout
--------------------------------------------------------------------------------

||| A field in a struct with its offset, size and alignment (all in bytes).
public export
record Field where
  constructor MkField
  name : String
  offset : Nat
  size : Nat
  alignment : Nat

||| Offset of the next field: round past this field up to its alignment.
public export
nextFieldOffset : Field -> Nat
nextFieldOffset f = alignUp (f.offset + f.size) f.alignment

||| A struct layout: a vector of fields plus a declared total size and
||| alignment, carrying two erased invariants —
|||   * `sizeCorrect`: the total size is at least the sum of the field sizes;
|||   * `aligned`:     the total size is a multiple of the alignment.
public export
record StructLayout where
  constructor MkStructLayout
  fields : Vect n Field
  totalSize : Nat
  alignment : Nat
  {auto 0 sizeCorrect : So (totalSize >= sum (map (\f => f.size) fields))}
  {auto 0 aligned : Divides alignment totalSize}

||| Every field sits at an offset that is a multiple of the field's own
||| alignment — the core C-ABI field-placement rule.
public export
data FieldsAligned : {0 n : Nat} -> Vect n Field -> Type where
  NoFields : FieldsAligned []
  ConsField :
    {0 m : Nat} ->
    (f : Field) ->
    (rest : Vect m Field) ->
    Divides f.alignment f.offset ->
    FieldsAligned rest ->
    FieldsAligned (f :: rest)

--------------------------------------------------------------------------------
-- C ABI Compatibility
--------------------------------------------------------------------------------

||| A layout is C-ABI compliant when all of its fields are correctly aligned.
||| (The total-size and total-alignment invariants are already carried by
||| `StructLayout` itself, so a `CABICompliant` value certifies the full
||| picture: well-sized, well-aligned struct with every field in place.)
public export
data CABICompliant : StructLayout -> Type where
  CABIOk :
    (layout : StructLayout) ->
    FieldsAligned layout.fields ->
    CABICompliant layout

-- NOTE: there is deliberately no generic
--   checkCABI : (l : StructLayout) -> Either String (CABICompliant l)
-- Proving `FieldsAligned` for an *arbitrary* layout requires a per-field
-- divisibility decision procedure that threads the evidence back into the
-- proof term. The honest, fully machine-checked artefacts are the concrete
-- `*Valid` instances below — Idris checks each at compile time. A runtime
-- `Dec`-returning checker can be added later without weakening these proofs.

--------------------------------------------------------------------------------
-- OctadRecord Layout
--------------------------------------------------------------------------------

||| The OctadRecord layout represents the metadata overlay for a single
||| database entity. It contains pointers/handles to each of the 8 octad
||| dimensions' sidecar data.
|||
||| C struct equivalent:
|||   struct OctadRecord {
|||     uint64_t entity_id;       // offset 0,  size 8
|||     uint32_t backend;         // offset 8,  size 4
|||     uint32_t active_dims;     // offset 12, size 4 (bitmask of enabled dimensions)
|||     uint64_t provenance_ptr;  // offset 16, size 8 (pointer to provenance chain)
|||     uint64_t temporal_ptr;    // offset 24, size 8 (pointer to temporal sidecar)
|||     uint64_t drift_score_ptr; // offset 32, size 8 (pointer to drift measurements)
|||     uint64_t lineage_ptr;     // offset 40, size 8 (pointer to lineage graph)
|||     uint64_t constraints_ptr; // offset 48, size 8 (pointer to constraint set)
|||     uint64_t acl_ptr;         // offset 56, size 8 (pointer to access control list)
|||     uint64_t simulation_ptr;  // offset 64, size 8 (pointer to simulation sandbox)
|||     uint64_t metadata_ptr;    // offset 72, size 8 (pointer to metadata blob)
|||   };
public export
octadRecordLayout : StructLayout
octadRecordLayout =
  MkStructLayout
    [ MkField "entity_id"       0  8 8   -- Bits64: unique entity identifier
    , MkField "backend"         8  4 4   -- Bits32: DatabaseBackend enum
    , MkField "active_dims"     12 4 4   -- Bits32: bitmask of active OctadDimensions
    , MkField "provenance_ptr"  16 8 8   -- Bits64: pointer to provenance chain head
    , MkField "temporal_ptr"    24 8 8   -- Bits64: pointer to temporal version list
    , MkField "drift_score_ptr" 32 8 8   -- Bits64: pointer to drift measurement
    , MkField "lineage_ptr"     40 8 8   -- Bits64: pointer to lineage DAG node
    , MkField "constraints_ptr" 48 8 8   -- Bits64: pointer to constraint set
    , MkField "acl_ptr"         56 8 8   -- Bits64: pointer to access control list
    , MkField "simulation_ptr"  64 8 8   -- Bits64: pointer to simulation sandbox
    , MkField "metadata_ptr"    72 8 8   -- Bits64: pointer to metadata blob
    ]
    80  -- Total size: 80 bytes
    8   -- Alignment: 8 bytes
    {aligned = DivideBy 10 Refl, sizeCorrect = Oh}  -- 80 = 10*8, and 80 >= sum(sizes)=80

||| Proof that the OctadRecord layout is C-ABI compliant: all 11 fields sit at
||| offsets that are multiples of their respective 8- or 4-byte alignments.
export
octadRecordValid : CABICompliant Verisimiser.ABI.Layout.octadRecordLayout
octadRecordValid =
  CABIOk octadRecordLayout $
    ConsField _ _ (DivideBy 0 Refl)      -- entity_id       @0  / 8
    (ConsField _ _ (DivideBy 2 Refl)     -- backend         @8  / 4
    (ConsField _ _ (DivideBy 3 Refl)     -- active_dims     @12 / 4
    (ConsField _ _ (DivideBy 2 Refl)     -- provenance_ptr  @16 / 8
    (ConsField _ _ (DivideBy 3 Refl)     -- temporal_ptr    @24 / 8
    (ConsField _ _ (DivideBy 4 Refl)     -- drift_score_ptr @32 / 8
    (ConsField _ _ (DivideBy 5 Refl)     -- lineage_ptr     @40 / 8
    (ConsField _ _ (DivideBy 6 Refl)     -- constraints_ptr @48 / 8
    (ConsField _ _ (DivideBy 7 Refl)     -- acl_ptr         @56 / 8
    (ConsField _ _ (DivideBy 8 Refl)     -- simulation_ptr  @64 / 8
    (ConsField _ _ (DivideBy 9 Refl)     -- metadata_ptr    @72 / 8
    NoFields))))))))))

--------------------------------------------------------------------------------
-- ProvenanceEntry Layout
--------------------------------------------------------------------------------

||| Layout for a single provenance hash chain entry.
||| SHA-256 hashes are stored as 32-byte arrays.
|||
||| C struct equivalent:
|||   struct ProvenanceEntry {
|||     uint8_t  hash[32];          // offset 0,  size 32 (SHA-256)
|||     uint8_t  previous_hash[32]; // offset 32, size 32 (SHA-256)
|||     uint64_t entity_id;         // offset 64, size 8
|||     uint32_t operation;         // offset 72, size 4 (ProvenanceOperation enum)
|||     uint32_t _padding;          // offset 76, size 4
|||     int64_t  timestamp;         // offset 80, size 8 (Unix epoch microseconds)
|||   };
public export
provenanceEntryLayout : StructLayout
provenanceEntryLayout =
  MkStructLayout
    [ MkField "hash"          0  32 1  -- 32 bytes SHA-256 (byte-aligned)
    , MkField "previous_hash" 32 32 1  -- 32 bytes SHA-256
    , MkField "entity_id"     64 8  8  -- Bits64
    , MkField "operation"     72 4  4  -- Bits32 (ProvenanceOperation)
    , MkField "_padding"      76 4  4  -- padding for 8-byte alignment
    , MkField "timestamp"     80 8  8  -- Int64 (Unix epoch microseconds)
    ]
    88  -- Total size: 88 bytes
    8   -- Alignment: 8 bytes
    {aligned = DivideBy 11 Refl, sizeCorrect = Oh}  -- 88 = 11*8, and 88 >= sum(sizes)=88

||| Proof that the ProvenanceEntry layout is C-ABI compliant.
export
provenanceEntryValid : CABICompliant Verisimiser.ABI.Layout.provenanceEntryLayout
provenanceEntryValid =
  CABIOk provenanceEntryLayout $
    ConsField _ _ (DivideBy 0 Refl)      -- hash          @0  / 1
    (ConsField _ _ (DivideBy 32 Refl)    -- previous_hash @32 / 1
    (ConsField _ _ (DivideBy 8 Refl)     -- entity_id     @64 / 8
    (ConsField _ _ (DivideBy 18 Refl)    -- operation     @72 / 4
    (ConsField _ _ (DivideBy 19 Refl)    -- _padding      @76 / 4
    (ConsField _ _ (DivideBy 10 Refl)    -- timestamp     @80 / 8
    NoFields)))))

--------------------------------------------------------------------------------
-- DriftMeasurement Layout
--------------------------------------------------------------------------------

||| Layout for a per-entity drift measurement across all 8 categories.
|||
||| C struct equivalent:
|||   struct DriftMeasurement {
|||     uint64_t entity_id;     // offset 0,  size 8
|||     double   overall_score; // offset 8,  size 8 (0.0 to 1.0)
|||     double   scores[8];    // offset 16, size 64 (one per DriftCategory)
|||     int64_t  measured_at;   // offset 80, size 8 (Unix epoch microseconds)
|||   };
public export
driftMeasurementLayout : StructLayout
driftMeasurementLayout =
  MkStructLayout
    [ MkField "entity_id"     0  8  8  -- Bits64
    , MkField "overall_score" 8  8  8  -- Double (0.0 - 1.0)
    , MkField "scores"        16 64 8  -- 8 x Double (one per DriftCategory)
    , MkField "measured_at"   80 8  8  -- Int64 (Unix epoch microseconds)
    ]
    88  -- Total size: 88 bytes
    8   -- Alignment: 8 bytes
    {aligned = DivideBy 11 Refl, sizeCorrect = Oh}  -- 88 = 11*8, and 88 >= sum(sizes)=88

||| Proof that the DriftMeasurement layout is C-ABI compliant.
export
driftMeasurementValid : CABICompliant Verisimiser.ABI.Layout.driftMeasurementLayout
driftMeasurementValid =
  CABIOk driftMeasurementLayout $
    ConsField _ _ (DivideBy 0 Refl)      -- entity_id     @0  / 8
    (ConsField _ _ (DivideBy 1 Refl)     -- overall_score @8  / 8
    (ConsField _ _ (DivideBy 2 Refl)     -- scores        @16 / 8
    (ConsField _ _ (DivideBy 10 Refl)    -- measured_at   @80 / 8
    NoFields)))

--------------------------------------------------------------------------------
-- TemporalSnapshot Layout
--------------------------------------------------------------------------------

||| Layout for a versioned snapshot of an entity.
|||
||| C struct equivalent:
|||   struct TemporalSnapshot {
|||     uint64_t entity_id;   // offset 0,  size 8
|||     uint64_t version;     // offset 8,  size 8
|||     int64_t  valid_from;  // offset 16, size 8 (Unix epoch microseconds)
|||     int64_t  valid_to;    // offset 24, size 8 (0 if current)
|||     uint64_t snapshot_ptr; // offset 32, size 8 (pointer to serialised snapshot)
|||     uint32_t snapshot_len; // offset 40, size 4
|||     uint32_t operation;    // offset 44, size 4 (ProvenanceOperation enum)
|||   };
public export
temporalSnapshotLayout : StructLayout
temporalSnapshotLayout =
  MkStructLayout
    [ MkField "entity_id"    0  8 8   -- Bits64
    , MkField "version"      8  8 8   -- Bits64 (monotonically increasing)
    , MkField "valid_from"   16 8 8   -- Int64 (Unix epoch microseconds)
    , MkField "valid_to"     24 8 8   -- Int64 (0 if current version)
    , MkField "snapshot_ptr" 32 8 8   -- Bits64 (pointer to serialised data)
    , MkField "snapshot_len" 40 4 4   -- Bits32 (length in bytes)
    , MkField "operation"    44 4 4   -- Bits32 (ProvenanceOperation enum)
    ]
    48  -- Total size: 48 bytes
    8   -- Alignment: 8 bytes
    {aligned = DivideBy 6 Refl, sizeCorrect = Oh}  -- 48 = 6*8, and 48 >= sum(sizes)=48

||| Proof that the TemporalSnapshot layout is C-ABI compliant.
export
temporalSnapshotValid : CABICompliant Verisimiser.ABI.Layout.temporalSnapshotLayout
temporalSnapshotValid =
  CABIOk temporalSnapshotLayout $
    ConsField _ _ (DivideBy 0 Refl)      -- entity_id    @0  / 8
    (ConsField _ _ (DivideBy 1 Refl)     -- version      @8  / 8
    (ConsField _ _ (DivideBy 2 Refl)     -- valid_from   @16 / 8
    (ConsField _ _ (DivideBy 3 Refl)     -- valid_to     @24 / 8
    (ConsField _ _ (DivideBy 4 Refl)     -- snapshot_ptr @32 / 8
    (ConsField _ _ (DivideBy 10 Refl)    -- snapshot_len @40 / 4
    (ConsField _ _ (DivideBy 11 Refl)    -- operation    @44 / 4
    NoFields))))))

--------------------------------------------------------------------------------
-- Field Lookup
--------------------------------------------------------------------------------

||| Look up a field by name, returning its index and the field itself.
public export
fieldOffset : (layout : StructLayout) -> (fieldName : String) -> Maybe (Nat, Field)
fieldOffset layout name =
  case findIndex (\f => f.name == name) layout.fields of
    Just idx => Just (finToNat idx, index idx layout.fields)
    Nothing  => Nothing

-- NOTE: there is deliberately no generic
--   offsetInBounds : (l : StructLayout) -> (f : Field) -> So (f.offset + f.size <= l.totalSize)
-- That statement is *false* for an arbitrary `f` and `l` (nothing forces a
-- caller-supplied field to belong to the layout, let alone fit inside it).
-- The in-bounds fact holds for the concrete layouts above — it is implied,
-- field by field, by their offsets/sizes and the `sizeCorrect` invariant —
-- and would be stated as a per-layout lemma over fields drawn from
-- `layout.fields`, not as a universally-quantified claim over all fields.
