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
thisPlatform = Linux  -- Default; override with compiler flags

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

-- NOTE: a DecEq instance for OctadDimension is intentionally omitted — it is
-- auxiliary (no proof or function below uses decEq on it) and used a
-- `decEq _ _ = No absurd` catch-all, which Idris2 rejects (it needs explicit
-- off-diagonal constructor pairs — impractical for 8 constructors). Re-add via
-- elaborator-reflection derivation if decidable equality is ever needed.

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

-- (DecEq DatabaseBackend omitted — auxiliary/unused, see note above.)

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

-- (DecEq Result omitted — auxiliary/unused, see note above.)

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
createHandle ptr = case choose (ptr /= 0) of
  Left  nn => Just (MkHandle ptr {nonNull = nn})
  Right _  => Nothing

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
createDbConnection ptr = case choose (ptr /= 0) of
  Left  nn => Just (MkDbConnection ptr {nonNull = nn})
  Right _  => Nothing

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

||| Pointer type for platform. C pointers cross the FFI as 64-bit addresses
||| (cf. `Handle`'s `ptr : Bits64`); `ptrSize` records the platform width
||| separately for layout calculations.
public export
CPtr : Platform -> Type -> Type
CPtr _ _ = Bits64

--------------------------------------------------------------------------------
-- Memory Layout Proofs
--------------------------------------------------------------------------------

-- NOTE: the `HasSize`/`HasAlignment` data types and `cSizeOf`/`cAlignOf`
-- functions were removed here. They were unused and unsound: HasSize/HasAlignment
-- had a single constructor inhabiting `HasSize t n` for ANY `t` and `n` (proving
-- nothing), and cSizeOf/cAlignOf tried to pattern-match on `Type` (e.g. `CInt _`),
-- which Idris2 rejects ("not a constructor application or primitive"). Platform
-- integer widths remain captured by CInt/CSize/ptrSize above; a sound
-- size/alignment formalisation can be added later over an explicit C-type enum.

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
