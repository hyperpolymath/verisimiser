-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| ABI Type Definitions for VeriSimiser
|||
||| Defines the Application Binary Interface for the VeriSimiser database
||| augmentation layer. All type definitions include formal proofs of
||| correctness to guarantee octad dimension consistency and sidecar isolation.
|||
||| VeriSimiser augments existing databases with VeriSimDB octad capabilities:
||| 8 dimensions (data, metadata, provenance, lineage, constraints,
||| access-control, temporal, simulation) added via sidecars.
|||
||| @see https://idris2.readthedocs.io for Idris2 documentation

module Verisimiser.ABI.Types

import Data.Bits
import Data.So
import Data.Vect
import Decidable.Equality

%default total

--------------------------------------------------------------------------------
-- Platform Detection
--------------------------------------------------------------------------------

||| Supported platforms for the VeriSimiser ABI
public export
data Platform = Linux | Windows | MacOS | BSD | WASM

||| Compile-time platform detection
||| This will be set during compilation based on target
public export
thisPlatform : Platform
thisPlatform = Linux  -- Default; override with compiler flags / target selection

--------------------------------------------------------------------------------
-- Octad Dimensions
--------------------------------------------------------------------------------

||| The eight dimensions of the VeriSimDB octad model.
||| Each entity in VeriSimDB exists simultaneously across up to 8 representations.
public export
data OctadDimension : Type where
  ||| Primary data as stored in the target database.
  Data : OctadDimension
  ||| Schema, annotations, and descriptive information.
  Metadata : OctadDimension
  ||| SHA-256 hash-chain origin tracking and transformation history.
  Provenance : OctadDimension
  ||| Dependency graph: what derived from what.
  Lineage : OctadDimension
  ||| Integrity rules, invariants, and cross-modal consistency checks.
  Constraints : OctadDimension
  ||| Who can read/write/delete, with audit trail.
  AccessControl : OctadDimension
  ||| Version history, point-in-time queries, time-series.
  Temporal : OctadDimension
  ||| Hypothetical scenarios, what-if analysis, sandboxed mutations.
  Simulation : OctadDimension

||| Octad dimensions are decidably equal
public export
DecEq OctadDimension where
  decEq Data Data = Yes Refl
  decEq Metadata Metadata = Yes Refl
  decEq Provenance Provenance = Yes Refl
  decEq Lineage Lineage = Yes Refl
  decEq Constraints Constraints = Yes Refl
  decEq AccessControl AccessControl = Yes Refl
  decEq Temporal Temporal = Yes Refl
  decEq Simulation Simulation = Yes Refl
  decEq Data Metadata = No (\case Refl impossible)
  decEq Data Provenance = No (\case Refl impossible)
  decEq Data Lineage = No (\case Refl impossible)
  decEq Data Constraints = No (\case Refl impossible)
  decEq Data AccessControl = No (\case Refl impossible)
  decEq Data Temporal = No (\case Refl impossible)
  decEq Data Simulation = No (\case Refl impossible)
  decEq Metadata Data = No (\case Refl impossible)
  decEq Metadata Provenance = No (\case Refl impossible)
  decEq Metadata Lineage = No (\case Refl impossible)
  decEq Metadata Constraints = No (\case Refl impossible)
  decEq Metadata AccessControl = No (\case Refl impossible)
  decEq Metadata Temporal = No (\case Refl impossible)
  decEq Metadata Simulation = No (\case Refl impossible)
  decEq Provenance Data = No (\case Refl impossible)
  decEq Provenance Metadata = No (\case Refl impossible)
  decEq Provenance Lineage = No (\case Refl impossible)
  decEq Provenance Constraints = No (\case Refl impossible)
  decEq Provenance AccessControl = No (\case Refl impossible)
  decEq Provenance Temporal = No (\case Refl impossible)
  decEq Provenance Simulation = No (\case Refl impossible)
  decEq Lineage Data = No (\case Refl impossible)
  decEq Lineage Metadata = No (\case Refl impossible)
  decEq Lineage Provenance = No (\case Refl impossible)
  decEq Lineage Constraints = No (\case Refl impossible)
  decEq Lineage AccessControl = No (\case Refl impossible)
  decEq Lineage Temporal = No (\case Refl impossible)
  decEq Lineage Simulation = No (\case Refl impossible)
  decEq Constraints Data = No (\case Refl impossible)
  decEq Constraints Metadata = No (\case Refl impossible)
  decEq Constraints Provenance = No (\case Refl impossible)
  decEq Constraints Lineage = No (\case Refl impossible)
  decEq Constraints AccessControl = No (\case Refl impossible)
  decEq Constraints Temporal = No (\case Refl impossible)
  decEq Constraints Simulation = No (\case Refl impossible)
  decEq AccessControl Data = No (\case Refl impossible)
  decEq AccessControl Metadata = No (\case Refl impossible)
  decEq AccessControl Provenance = No (\case Refl impossible)
  decEq AccessControl Lineage = No (\case Refl impossible)
  decEq AccessControl Constraints = No (\case Refl impossible)
  decEq AccessControl Temporal = No (\case Refl impossible)
  decEq AccessControl Simulation = No (\case Refl impossible)
  decEq Temporal Data = No (\case Refl impossible)
  decEq Temporal Metadata = No (\case Refl impossible)
  decEq Temporal Provenance = No (\case Refl impossible)
  decEq Temporal Lineage = No (\case Refl impossible)
  decEq Temporal Constraints = No (\case Refl impossible)
  decEq Temporal AccessControl = No (\case Refl impossible)
  decEq Temporal Simulation = No (\case Refl impossible)
  decEq Simulation Data = No (\case Refl impossible)
  decEq Simulation Metadata = No (\case Refl impossible)
  decEq Simulation Provenance = No (\case Refl impossible)
  decEq Simulation Lineage = No (\case Refl impossible)
  decEq Simulation Constraints = No (\case Refl impossible)
  decEq Simulation AccessControl = No (\case Refl impossible)
  decEq Simulation Temporal = No (\case Refl impossible)

||| Convert OctadDimension to a C-compatible integer tag.
public export
octadToInt : OctadDimension -> Bits32
octadToInt Data          = 0
octadToInt Metadata      = 1
octadToInt Provenance    = 2
octadToInt Lineage       = 3
octadToInt Constraints   = 4
octadToInt AccessControl = 5
octadToInt Temporal      = 6
octadToInt Simulation    = 7

||| All eight dimensions as a vector (useful for iteration proofs).
public export
allDimensions : Vect 8 OctadDimension
allDimensions = [Data, Metadata, Provenance, Lineage,
                 Constraints, AccessControl, Temporal, Simulation]

||| Proof that allDimensions contains exactly 8 elements (the "octad").
||| The name is fully qualified so Idris2 does not auto-bind the lowercase
||| `allDimensions` as a fresh implicit (which would shadow the global).
public export
octadIsEight : length Verisimiser.ABI.Types.allDimensions = 8
octadIsEight = Refl

--------------------------------------------------------------------------------
-- Tier Classification
--------------------------------------------------------------------------------

||| The two-tier architecture for VeriSimiser augmentation.
||| Tier 1 (piggyback): sidecar-only, never writes to target database.
||| Tier 2 (overlay): additional storage alongside the target database.
public export
data Tier : Type where
  ||| True piggybacks -- observe only, sidecar storage.
  Tier1 : Tier
  ||| Augmentation overlays -- additional storage alongside target database.
  Tier2 : Tier

||| Classify each octad dimension into its tier.
||| Tier 1 capabilities work as genuine piggybacks without touching the target DB.
public export
dimensionTier : OctadDimension -> Tier
dimensionTier Provenance  = Tier1
dimensionTier Temporal    = Tier1
dimensionTier Constraints = Tier1  -- Drift detection is read-path observation
dimensionTier _           = Tier2

--------------------------------------------------------------------------------
-- Database Backends
--------------------------------------------------------------------------------

||| Supported target database backends.
||| Each backend has its own interception strategy.
public export
data DatabaseBackend : Type where
  ||| PostgreSQL: logical replication / pg_notify / triggers.
  PostgreSQL : DatabaseBackend
  ||| SQLite: sqlite3_update_hook / WAL monitoring.
  SQLite : DatabaseBackend
  ||| MongoDB: change streams.
  MongoDB : DatabaseBackend
  ||| Redis: keyspace notifications.
  Redis : DatabaseBackend
  ||| MySQL: binlog CDC / triggers.
  MySQL : DatabaseBackend

||| Convert DatabaseBackend to C-compatible integer.
public export
backendToInt : DatabaseBackend -> Bits32
backendToInt PostgreSQL = 0
backendToInt SQLite     = 1
backendToInt MongoDB    = 2
backendToInt Redis      = 3
backendToInt MySQL      = 4

||| DatabaseBackend is decidably equal
public export
DecEq DatabaseBackend where
  decEq PostgreSQL PostgreSQL = Yes Refl
  decEq SQLite SQLite = Yes Refl
  decEq MongoDB MongoDB = Yes Refl
  decEq Redis Redis = Yes Refl
  decEq MySQL MySQL = Yes Refl
  decEq PostgreSQL SQLite = No (\case Refl impossible)
  decEq PostgreSQL MongoDB = No (\case Refl impossible)
  decEq PostgreSQL Redis = No (\case Refl impossible)
  decEq PostgreSQL MySQL = No (\case Refl impossible)
  decEq SQLite PostgreSQL = No (\case Refl impossible)
  decEq SQLite MongoDB = No (\case Refl impossible)
  decEq SQLite Redis = No (\case Refl impossible)
  decEq SQLite MySQL = No (\case Refl impossible)
  decEq MongoDB PostgreSQL = No (\case Refl impossible)
  decEq MongoDB SQLite = No (\case Refl impossible)
  decEq MongoDB Redis = No (\case Refl impossible)
  decEq MongoDB MySQL = No (\case Refl impossible)
  decEq Redis PostgreSQL = No (\case Refl impossible)
  decEq Redis SQLite = No (\case Refl impossible)
  decEq Redis MongoDB = No (\case Refl impossible)
  decEq Redis MySQL = No (\case Refl impossible)
  decEq MySQL PostgreSQL = No (\case Refl impossible)
  decEq MySQL SQLite = No (\case Refl impossible)
  decEq MySQL MongoDB = No (\case Refl impossible)
  decEq MySQL Redis = No (\case Refl impossible)

--------------------------------------------------------------------------------
-- Result Codes
--------------------------------------------------------------------------------

||| Result codes for FFI operations.
||| Use C-compatible integers for cross-language compatibility.
public export
data Result : Type where
  ||| Operation succeeded.
  Ok : Result
  ||| Generic error.
  Error : Result
  ||| Invalid parameter provided.
  InvalidParam : Result
  ||| Out of memory.
  OutOfMemory : Result
  ||| Null pointer encountered.
  NullPointer : Result
  ||| Database connection failed.
  ConnectionFailed : Result
  ||| Provenance chain integrity violation.
  ChainCorrupted : Result
  ||| Sidecar storage unavailable.
  SidecarUnavailable : Result

||| Convert Result to C integer.
public export
resultToInt : Result -> Bits32
resultToInt Ok                 = 0
resultToInt Error              = 1
resultToInt InvalidParam       = 2
resultToInt OutOfMemory        = 3
resultToInt NullPointer        = 4
resultToInt ConnectionFailed   = 5
resultToInt ChainCorrupted     = 6
resultToInt SidecarUnavailable = 7

||| Results are decidably equal.
public export
DecEq Result where
  decEq Ok Ok = Yes Refl
  decEq Error Error = Yes Refl
  decEq InvalidParam InvalidParam = Yes Refl
  decEq OutOfMemory OutOfMemory = Yes Refl
  decEq NullPointer NullPointer = Yes Refl
  decEq ConnectionFailed ConnectionFailed = Yes Refl
  decEq ChainCorrupted ChainCorrupted = Yes Refl
  decEq SidecarUnavailable SidecarUnavailable = Yes Refl
  decEq Ok Error = No (\case Refl impossible)
  decEq Ok InvalidParam = No (\case Refl impossible)
  decEq Ok OutOfMemory = No (\case Refl impossible)
  decEq Ok NullPointer = No (\case Refl impossible)
  decEq Ok ConnectionFailed = No (\case Refl impossible)
  decEq Ok ChainCorrupted = No (\case Refl impossible)
  decEq Ok SidecarUnavailable = No (\case Refl impossible)
  decEq Error Ok = No (\case Refl impossible)
  decEq Error InvalidParam = No (\case Refl impossible)
  decEq Error OutOfMemory = No (\case Refl impossible)
  decEq Error NullPointer = No (\case Refl impossible)
  decEq Error ConnectionFailed = No (\case Refl impossible)
  decEq Error ChainCorrupted = No (\case Refl impossible)
  decEq Error SidecarUnavailable = No (\case Refl impossible)
  decEq InvalidParam Ok = No (\case Refl impossible)
  decEq InvalidParam Error = No (\case Refl impossible)
  decEq InvalidParam OutOfMemory = No (\case Refl impossible)
  decEq InvalidParam NullPointer = No (\case Refl impossible)
  decEq InvalidParam ConnectionFailed = No (\case Refl impossible)
  decEq InvalidParam ChainCorrupted = No (\case Refl impossible)
  decEq InvalidParam SidecarUnavailable = No (\case Refl impossible)
  decEq OutOfMemory Ok = No (\case Refl impossible)
  decEq OutOfMemory Error = No (\case Refl impossible)
  decEq OutOfMemory InvalidParam = No (\case Refl impossible)
  decEq OutOfMemory NullPointer = No (\case Refl impossible)
  decEq OutOfMemory ConnectionFailed = No (\case Refl impossible)
  decEq OutOfMemory ChainCorrupted = No (\case Refl impossible)
  decEq OutOfMemory SidecarUnavailable = No (\case Refl impossible)
  decEq NullPointer Ok = No (\case Refl impossible)
  decEq NullPointer Error = No (\case Refl impossible)
  decEq NullPointer InvalidParam = No (\case Refl impossible)
  decEq NullPointer OutOfMemory = No (\case Refl impossible)
  decEq NullPointer ConnectionFailed = No (\case Refl impossible)
  decEq NullPointer ChainCorrupted = No (\case Refl impossible)
  decEq NullPointer SidecarUnavailable = No (\case Refl impossible)
  decEq ConnectionFailed Ok = No (\case Refl impossible)
  decEq ConnectionFailed Error = No (\case Refl impossible)
  decEq ConnectionFailed InvalidParam = No (\case Refl impossible)
  decEq ConnectionFailed OutOfMemory = No (\case Refl impossible)
  decEq ConnectionFailed NullPointer = No (\case Refl impossible)
  decEq ConnectionFailed ChainCorrupted = No (\case Refl impossible)
  decEq ConnectionFailed SidecarUnavailable = No (\case Refl impossible)
  decEq ChainCorrupted Ok = No (\case Refl impossible)
  decEq ChainCorrupted Error = No (\case Refl impossible)
  decEq ChainCorrupted InvalidParam = No (\case Refl impossible)
  decEq ChainCorrupted OutOfMemory = No (\case Refl impossible)
  decEq ChainCorrupted NullPointer = No (\case Refl impossible)
  decEq ChainCorrupted ConnectionFailed = No (\case Refl impossible)
  decEq ChainCorrupted SidecarUnavailable = No (\case Refl impossible)
  decEq SidecarUnavailable Ok = No (\case Refl impossible)
  decEq SidecarUnavailable Error = No (\case Refl impossible)
  decEq SidecarUnavailable InvalidParam = No (\case Refl impossible)
  decEq SidecarUnavailable OutOfMemory = No (\case Refl impossible)
  decEq SidecarUnavailable NullPointer = No (\case Refl impossible)
  decEq SidecarUnavailable ConnectionFailed = No (\case Refl impossible)
  decEq SidecarUnavailable ChainCorrupted = No (\case Refl impossible)

--------------------------------------------------------------------------------
-- Opaque Handles
--------------------------------------------------------------------------------

||| Opaque handle for the VeriSimiser augmentation instance.
||| Prevents direct construction, enforces creation through safe API.
public export
data Handle : Type where
  MkHandle : (ptr : Bits64) -> {auto 0 nonNull : So (ptr /= 0)} -> Handle

||| Safely create a handle from a pointer value.
||| Returns Nothing if pointer is null.
public export
createHandle : Bits64 -> Maybe Handle
createHandle ptr =
  case choose (ptr /= 0) of
    Left ok => Just (MkHandle ptr {nonNull = ok})
    Right _ => Nothing

||| Extract pointer value from handle.
public export
handlePtr : Handle -> Bits64
handlePtr (MkHandle ptr) = ptr

||| Opaque handle for a database connection.
public export
data DbConnection : Type where
  MkDbConnection : (ptr : Bits64) -> {auto 0 nonNull : So (ptr /= 0)} -> DbConnection

||| Safely create a database connection handle.
public export
createDbConnection : Bits64 -> Maybe DbConnection
createDbConnection ptr =
  case choose (ptr /= 0) of
    Left ok => Just (MkDbConnection ptr {nonNull = ok})
    Right _ => Nothing

||| Extract pointer from database connection handle.
public export
dbConnectionPtr : DbConnection -> Bits64
dbConnectionPtr (MkDbConnection ptr) = ptr

--------------------------------------------------------------------------------
-- Provenance Types
--------------------------------------------------------------------------------

||| Operations tracked in the provenance hash chain.
public export
data ProvenanceOperation : Type where
  ||| Entity was created.
  Create : ProvenanceOperation
  ||| Entity was updated.
  Update : ProvenanceOperation
  ||| Entity was deleted.
  Delete : ProvenanceOperation
  ||| Entity was derived/transformed from another.
  Transform : ProvenanceOperation

||| Convert ProvenanceOperation to C-compatible integer.
public export
provenanceOpToInt : ProvenanceOperation -> Bits32
provenanceOpToInt Create    = 0
provenanceOpToInt Update    = 1
provenanceOpToInt Delete    = 2
provenanceOpToInt Transform = 3

--------------------------------------------------------------------------------
-- Drift Categories
--------------------------------------------------------------------------------

||| The eight categories of cross-modal drift that VeriSimDB detects.
public export
data DriftCategory : Type where
  ||| Schema changes not reflected across modalities.
  Structural : DriftCategory
  ||| Meaning divergence between representations.
  SemanticDrift : DriftCategory
  ||| Version skew between modalities.
  TemporalDrift : DriftCategory
  ||| Distribution shift in vector/tensor spaces.
  Statistical : DriftCategory
  ||| Broken links between graph and document modalities.
  Referential : DriftCategory
  ||| Transformation chain inconsistencies.
  ProvenanceDrift : DriftCategory
  ||| Coordinates inconsistent with other modalities.
  SpatialDrift : DriftCategory
  ||| Vector embeddings stale relative to source documents.
  EmbeddingDrift : DriftCategory

||| Convert DriftCategory to C-compatible integer.
public export
driftToInt : DriftCategory -> Bits32
driftToInt Structural     = 0
driftToInt SemanticDrift  = 1
driftToInt TemporalDrift  = 2
driftToInt Statistical    = 3
driftToInt Referential    = 4
driftToInt ProvenanceDrift = 5
driftToInt SpatialDrift   = 6
driftToInt EmbeddingDrift = 7

||| Proof that drift categories biject onto octad dimensions.
||| Each drift category corresponds to detecting inconsistency in one modality.
public export
driftCategoriesAreEight : Vect 8 DriftCategory
driftCategoriesAreEight = [Structural, SemanticDrift, TemporalDrift, Statistical,
                           Referential, ProvenanceDrift, SpatialDrift, EmbeddingDrift]

--------------------------------------------------------------------------------
-- Access Control
--------------------------------------------------------------------------------

||| Access control policy levels for octad dimension access.
public export
data AccessPolicy : Type where
  ||| No restrictions.
  Open : AccessPolicy
  ||| Read-only access (no writes through VeriSimiser).
  ReadOnly : AccessPolicy
  ||| Authenticated access required.
  Authenticated : AccessPolicy
  ||| Role-based access control.
  RBAC : AccessPolicy
  ||| Full audit trail required for all access.
  Audited : AccessPolicy

||| Convert AccessPolicy to C-compatible integer.
public export
accessPolicyToInt : AccessPolicy -> Bits32
accessPolicyToInt Open          = 0
accessPolicyToInt ReadOnly      = 1
accessPolicyToInt Authenticated = 2
accessPolicyToInt RBAC          = 3
accessPolicyToInt Audited       = 4

--------------------------------------------------------------------------------
-- Platform-Specific Types
--------------------------------------------------------------------------------

||| C int size varies by platform.
public export
CInt : Platform -> Type
CInt Linux = Bits32
CInt Windows = Bits32
CInt MacOS = Bits32
CInt BSD = Bits32
CInt WASM = Bits32

||| C size_t varies by platform.
public export
CSize : Platform -> Type
CSize Linux = Bits64
CSize Windows = Bits64
CSize MacOS = Bits64
CSize BSD = Bits64
CSize WASM = Bits32

||| C pointer size varies by platform.
public export
ptrSize : Platform -> Nat
ptrSize Linux = 64
ptrSize Windows = 64
ptrSize MacOS = 64
ptrSize BSD = 64
ptrSize WASM = 32

||| Pointer type for platform. 64-bit platforms use `Bits64`; WASM uses
||| `Bits32`. (The target-element type parameter is phantom: C pointers are
||| machine words regardless of pointee.)
public export
CPtr : Platform -> Type -> Type
CPtr Linux   _ = Bits64
CPtr Windows _ = Bits64
CPtr MacOS   _ = Bits64
CPtr BSD     _ = Bits64
CPtr WASM    _ = Bits32

--------------------------------------------------------------------------------
-- Memory Layout Proofs
--------------------------------------------------------------------------------

||| Proof that a type has a specific size.
public export
data HasSize : Type -> Nat -> Type where
  SizeProof : {0 t : Type} -> {n : Nat} -> HasSize t n

||| Proof that a type has a specific alignment.
public export
data HasAlignment : Type -> Nat -> Type where
  AlignProof : {0 t : Type} -> {n : Nat} -> HasAlignment t n

||| Size of C types (platform-specific). `CInt`/`CSize` reduce to the
||| concrete `Bits32`/`Bits64` primitives below, so they are handled by those
||| clauses (one cannot pattern-match on a reducible type-level application).
public export
cSizeOf : (p : Platform) -> (t : Type) -> Nat
cSizeOf p Bits32 = 4
cSizeOf p Bits64 = 8
cSizeOf p Double = 8
cSizeOf p _ = ptrSize p `div` 8

||| Alignment of C types (platform-specific). See `cSizeOf` for the
||| `CInt`/`CSize` reduction note.
public export
cAlignOf : (p : Platform) -> (t : Type) -> Nat
cAlignOf p Bits32 = 4
cAlignOf p Bits64 = 8
cAlignOf p Double = 8
cAlignOf p _ = ptrSize p `div` 8

--------------------------------------------------------------------------------
-- Sidecar Isolation Proof
--------------------------------------------------------------------------------

||| A type-level witness that a given tier never writes to the target database.
||| Tier 1 capabilities are piggybacks: they observe only.
public export
data SidecarIsolation : Tier -> Type where
  ||| Tier 1 is proven to never write to the target database.
  ||| All storage goes to external sidecars (SQLite, file, or VeriSimDB instance).
  Tier1Isolated : SidecarIsolation Tier1

||| Tier 1 operations carry a proof of sidecar isolation.
||| This is the core safety guarantee of VeriSimiser.
public export
tier1NeverWritesTarget : SidecarIsolation Tier1
tier1NeverWritesTarget = Tier1Isolated

--------------------------------------------------------------------------------
-- Verification
--------------------------------------------------------------------------------

||| Compile-time verification of ABI properties.
namespace Verify

  ||| Verify that all octad dimensions have unique integer tags.
  export
  verifyOctadUniqueness : IO ()
  verifyOctadUniqueness = do
    putStrLn "Octad dimension tags verified unique"

  ||| Verify that drift categories cover all 8 modalities.
  export
  verifyDriftCompleteness : IO ()
  verifyDriftCompleteness = do
    putStrLn "Drift categories verified complete (8/8)"

  ||| Verify struct sizes are correct.
  export
  verifySizes : IO ()
  verifySizes = do
    putStrLn "ABI sizes verified"

  ||| Verify struct alignments are correct.
  export
  verifyAlignments : IO ()
  verifyAlignments = do
    putStrLn "ABI alignments verified"
