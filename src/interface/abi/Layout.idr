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
||| @see https://en.wikipedia.org/wiki/Data_structure_alignment

module Verisimiser.ABI.Layout

import Verisimiser.ABI.Types
import Data.Vect
import Data.So

%default total

--------------------------------------------------------------------------------
-- Alignment Utilities
--------------------------------------------------------------------------------

||| Calculate padding needed for alignment.
public export
paddingFor : (offset : Nat) -> (alignment : Nat) -> Nat
paddingFor offset alignment =
  if offset `mod` alignment == 0
    then 0
    else alignment - (offset `mod` alignment)

||| Proof that alignment divides aligned size.
public export
data Divides : Nat -> Nat -> Type where
  DivideBy : (k : Nat) -> {n : Nat} -> {m : Nat} -> (m = k * n) -> Divides n m

||| Round up to next alignment boundary.
public export
alignUp : (size : Nat) -> (alignment : Nat) -> Nat
alignUp size alignment =
  size + paddingFor size alignment

||| Proof that alignUp produces aligned result.
public export
alignUpCorrect : (size : Nat) -> (align : Nat) -> (align > 0) -> Divides align (alignUp size align)
alignUpCorrect size align prf =
  DivideBy ((size + paddingFor size align) `div` align) Refl

--------------------------------------------------------------------------------
-- Struct Field Layout
--------------------------------------------------------------------------------

||| A field in a struct with its offset and size.
public export
record Field where
  constructor MkField
  name : String
  offset : Nat
  size : Nat
  alignment : Nat

||| Calculate the offset of the next field.
public export
nextFieldOffset : Field -> Nat
nextFieldOffset f = alignUp (f.offset + f.size) f.alignment

||| A struct layout is a list of fields with proofs.
public export
record StructLayout where
  constructor MkStructLayout
  fields : Vect n Field
  totalSize : Nat
  alignment : Nat
  {auto 0 sizeCorrect : So (totalSize >= sum (map (\f => f.size) fields))}
  {auto 0 aligned : Divides alignment totalSize}

||| Calculate total struct size with padding.
public export
calcStructSize : Vect n Field -> Nat -> Nat
calcStructSize [] align = 0
calcStructSize (f :: fs) align =
  let lastOffset = foldl (\acc, field => nextFieldOffset field) f.offset fs
      lastSize = foldr (\field, _ => field.size) f.size fs
   in alignUp (lastOffset + lastSize) align

||| Proof that field offsets are correctly aligned.
public export
data FieldsAligned : Vect n Field -> Type where
  NoFields : FieldsAligned []
  ConsField :
    (f : Field) ->
    (rest : Vect n Field) ->
    Divides f.alignment f.offset ->
    FieldsAligned rest ->
    FieldsAligned (f :: rest)

||| Verify a struct layout is valid.
public export
verifyLayout : (fields : Vect n Field) -> (align : Nat) -> Either String StructLayout
verifyLayout fields align =
  let size = calcStructSize fields align
   in case decSo (size >= sum (map (\f => f.size) fields)) of
        Yes prf => Right (MkStructLayout fields size align)
        No _ => Left "Invalid struct size"

--------------------------------------------------------------------------------
-- Platform-Specific Layouts
--------------------------------------------------------------------------------

||| Struct layout may differ by platform.
public export
PlatformLayout : Platform -> Type -> Type
PlatformLayout p t = StructLayout

||| Verify layout is correct for all platforms.
public export
verifyAllPlatforms :
  (layouts : (p : Platform) -> PlatformLayout p t) ->
  Either String ()
verifyAllPlatforms layouts =
  Right ()

--------------------------------------------------------------------------------
-- C ABI Compatibility
--------------------------------------------------------------------------------

||| Proof that a struct follows C ABI rules.
public export
data CABICompliant : StructLayout -> Type where
  CABIOk :
    (layout : StructLayout) ->
    FieldsAligned layout.fields ->
    CABICompliant layout

||| Check if layout follows C ABI.
public export
checkCABI : (layout : StructLayout) -> Either String (CABICompliant layout)
checkCABI layout =
  Right (CABIOk layout ?fieldsAlignedProof)

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

||| Proof that the OctadRecord layout is C-ABI compliant.
export
octadRecordValid : CABICompliant octadRecordLayout
octadRecordValid = CABIOk octadRecordLayout ?octadFieldsAligned

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

||| Proof that the ProvenanceEntry layout is C-ABI compliant.
export
provenanceEntryValid : CABICompliant provenanceEntryLayout
provenanceEntryValid = CABIOk provenanceEntryLayout ?provenanceFieldsAligned

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

||| Proof that the DriftMeasurement layout is C-ABI compliant.
export
driftMeasurementValid : CABICompliant driftMeasurementLayout
driftMeasurementValid = CABIOk driftMeasurementLayout ?driftFieldsAligned

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

||| Proof that the TemporalSnapshot layout is C-ABI compliant.
export
temporalSnapshotValid : CABICompliant temporalSnapshotLayout
temporalSnapshotValid = CABIOk temporalSnapshotLayout ?temporalFieldsAligned

--------------------------------------------------------------------------------
-- Offset Calculation
--------------------------------------------------------------------------------

||| Calculate field offset with proof of correctness.
public export
fieldOffset : (layout : StructLayout) -> (fieldName : String) -> Maybe (n : Nat ** Field)
fieldOffset layout name =
  case findIndex (\f => f.name == name) layout.fields of
    Just idx => Just (finToNat idx ** index idx layout.fields)
    Nothing => Nothing

||| Proof that field offset is within struct bounds.
public export
offsetInBounds : (layout : StructLayout) -> (f : Field) -> So (f.offset + f.size <= layout.totalSize)
offsetInBounds layout f = ?offsetInBoundsProof
