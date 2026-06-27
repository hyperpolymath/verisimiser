-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Foreign Function Interface Declarations for VeriSimiser
|||
||| Declares all C-compatible functions implemented in the Zig FFI layer.
||| VeriSimiser wraps existing databases to add VeriSimDB octad capabilities.
|||
||| Function groups:
||| - Library lifecycle (init/free)
||| - Database connection (connect to target DB)
||| - Octad overlay (attach dimensions to entities)
||| - Tier 1: drift detection, provenance, temporal versioning
||| - VCL-total queries (type-safe octad queries)
||| - Error handling and version info
|||
||| All functions are declared here with type signatures and safety proofs.
||| Implementations live in src/interface/ffi/

module Verisimiser.ABI.Foreign

import Verisimiser.ABI.Types
import Verisimiser.ABI.Layout

%default total

--------------------------------------------------------------------------------
-- Library Lifecycle
--------------------------------------------------------------------------------

||| Initialise the VeriSimiser library.
||| Returns a handle to the augmentation instance, or Nothing on failure.
export
%foreign "C:verisimiser_init, libverisimiser"
prim__init : PrimIO Bits64

||| Safe wrapper for library initialisation.
export
init : IO (Maybe Handle)
init = do
  ptr <- primIO prim__init
  pure (createHandle ptr)

||| Clean up all VeriSimiser resources (sidecars, connections, overlays).
export
%foreign "C:verisimiser_free, libverisimiser"
prim__free : Bits64 -> PrimIO ()

||| Safe wrapper for cleanup.
export
free : Handle -> IO ()
free h = primIO (prim__free (handlePtr h))

--------------------------------------------------------------------------------
-- Database Connection
--------------------------------------------------------------------------------

||| Connect to a target database backend.
||| The connection string format depends on the backend type.
export
%foreign "C:verisimiser_connect, libverisimiser"
prim__connect : Bits64 -> Bits32 -> Bits64 -> PrimIO Bits64

||| Safe wrapper for database connection.
||| Connects the VeriSimiser instance to the target database.
export
connect : Handle -> DatabaseBackend -> (connString : Bits64) -> IO (Maybe DbConnection)
connect h backend connStr = do
  ptr <- primIO (prim__connect (handlePtr h) (backendToInt backend) connStr)
  pure (createDbConnection ptr)

||| Disconnect from the target database.
export
%foreign "C:verisimiser_disconnect, libverisimiser"
prim__disconnect : Bits64 -> Bits64 -> PrimIO ()

||| Safe wrapper for disconnection.
export
disconnect : Handle -> DbConnection -> IO ()
disconnect h db = primIO (prim__disconnect (handlePtr h) (dbConnectionPtr db))

--------------------------------------------------------------------------------
-- Octad Overlay Operations
--------------------------------------------------------------------------------

||| Enable an octad dimension for a database entity.
||| Only enables the dimension -- does not create initial data.
export
%foreign "C:verisimiser_enable_dimension, libverisimiser"
prim__enableDimension : Bits64 -> Bits64 -> Bits32 -> PrimIO Bits32

||| Safe wrapper: enable an octad dimension for an entity.
export
enableDimension : Handle -> (entityId : Bits64) -> OctadDimension -> IO (Either Result ())
enableDimension h entityId dim = do
  result <- primIO (prim__enableDimension (handlePtr h) entityId (octadToInt dim))
  pure $ if result == 0 then Right () else Left Error

||| Get the active dimension bitmask for an entity.
export
%foreign "C:verisimiser_get_active_dimensions, libverisimiser"
prim__getActiveDimensions : Bits64 -> Bits64 -> PrimIO Bits32

||| Safe wrapper: query which octad dimensions are active for an entity.
export
getActiveDimensions : Handle -> (entityId : Bits64) -> IO Bits32
getActiveDimensions h entityId =
  primIO (prim__getActiveDimensions (handlePtr h) entityId)

--------------------------------------------------------------------------------
-- Tier 1: Provenance Tracking
--------------------------------------------------------------------------------

||| Record a provenance event for an entity.
||| Appends to the SHA-256 hash chain in the provenance sidecar.
export
%foreign "C:verisimiser_record_provenance, libverisimiser"
prim__recordProvenance : Bits64 -> Bits64 -> Bits32 -> Bits64 -> PrimIO Bits32

||| Safe wrapper: record a provenance event.
||| The actor pointer should reference a null-terminated C string.
export
recordProvenance : Handle -> (entityId : Bits64) -> ProvenanceOperation -> (actor : Bits64) -> IO (Either Result ())
recordProvenance h entityId op actor = do
  result <- primIO (prim__recordProvenance (handlePtr h) entityId (provenanceOpToInt op) actor)
  pure $ if result == 0 then Right () else Left ChainCorrupted

||| Verify the integrity of an entity's provenance chain.
||| Returns Ok if the hash chain is intact, ChainCorrupted otherwise.
export
%foreign "C:verisimiser_verify_provenance, libverisimiser"
prim__verifyProvenance : Bits64 -> Bits64 -> PrimIO Bits32

||| Safe wrapper: verify provenance chain integrity.
export
verifyProvenance : Handle -> (entityId : Bits64) -> IO (Either Result ())
verifyProvenance h entityId = do
  result <- primIO (prim__verifyProvenance (handlePtr h) entityId)
  pure $ case result of
    0 => Right ()
    6 => Left ChainCorrupted
    _ => Left Error

||| Get the length of an entity's provenance chain.
export
%foreign "C:verisimiser_provenance_length, libverisimiser"
prim__provenanceLength : Bits64 -> Bits64 -> PrimIO Bits64

||| Safe wrapper: get provenance chain length.
export
provenanceLength : Handle -> (entityId : Bits64) -> IO Bits64
provenanceLength h entityId =
  primIO (prim__provenanceLength (handlePtr h) entityId)

--------------------------------------------------------------------------------
-- Tier 1: Temporal Versioning
--------------------------------------------------------------------------------

||| Record a temporal snapshot for an entity.
||| Stores the current state in the temporal sidecar.
export
%foreign "C:verisimiser_record_version, libverisimiser"
prim__recordVersion : Bits64 -> Bits64 -> Bits64 -> Bits32 -> PrimIO Bits32

||| Safe wrapper: record a temporal version.
||| snapshotPtr should point to serialised entity state.
export
recordVersion : Handle -> (entityId : Bits64) -> (snapshotPtr : Bits64) -> (snapshotLen : Bits32) -> IO (Either Result ())
recordVersion h entityId snap len = do
  result <- primIO (prim__recordVersion (handlePtr h) entityId snap len)
  pure $ if result == 0 then Right () else Left SidecarUnavailable

||| Query entity state at a specific point in time.
||| Returns a pointer to the serialised snapshot, or null if not found.
export
%foreign "C:verisimiser_query_at_time, libverisimiser"
prim__queryAtTime : Bits64 -> Bits64 -> Bits64 -> PrimIO Bits64

||| Safe wrapper: point-in-time query.
||| timestamp is Unix epoch microseconds.
export
queryAtTime : Handle -> (entityId : Bits64) -> (timestamp : Bits64) -> IO (Maybe Bits64)
queryAtTime h entityId ts = do
  ptr <- primIO (prim__queryAtTime (handlePtr h) entityId ts)
  pure $ if ptr == 0 then Nothing else Just ptr

||| Get the current version number for an entity.
export
%foreign "C:verisimiser_current_version, libverisimiser"
prim__currentVersion : Bits64 -> Bits64 -> PrimIO Bits64

||| Safe wrapper: get current version number.
export
currentVersion : Handle -> (entityId : Bits64) -> IO Bits64
currentVersion h entityId =
  primIO (prim__currentVersion (handlePtr h) entityId)

--------------------------------------------------------------------------------
-- Tier 1: Drift Detection
--------------------------------------------------------------------------------

||| Measure cross-modal drift for an entity.
||| Computes drift scores across all 8 categories.
export
%foreign "C:verisimiser_measure_drift, libverisimiser"
prim__measureDrift : Bits64 -> Bits64 -> PrimIO Bits64

||| Safe wrapper: measure drift.
||| Returns a pointer to a DriftMeasurement struct, or Nothing if entity not found.
export
measureDrift : Handle -> (entityId : Bits64) -> IO (Maybe Bits64)
measureDrift h entityId = do
  ptr <- primIO (prim__measureDrift (handlePtr h) entityId)
  pure $ if ptr == 0 then Nothing else Just ptr

||| Get the overall drift score for an entity (0.0 = consistent, 1.0 = diverged).
export
%foreign "C:verisimiser_drift_score, libverisimiser"
prim__driftScore : Bits64 -> Bits64 -> PrimIO Double

||| Safe wrapper: get overall drift score.
export
driftScore : Handle -> (entityId : Bits64) -> IO Double
driftScore h entityId =
  primIO (prim__driftScore (handlePtr h) entityId)

||| Get drift score for a specific category.
export
%foreign "C:verisimiser_drift_category_score, libverisimiser"
prim__driftCategoryScore : Bits64 -> Bits64 -> Bits32 -> PrimIO Double

||| Safe wrapper: get drift score for one category.
export
driftCategoryScore : Handle -> (entityId : Bits64) -> DriftCategory -> IO Double
driftCategoryScore h entityId cat =
  primIO (prim__driftCategoryScore (handlePtr h) entityId (driftToInt cat))

--------------------------------------------------------------------------------
-- VCL-total Query Interface
--------------------------------------------------------------------------------

||| Execute a VCL-total query against the augmented database.
||| The query string is type-checked before execution.
export
%foreign "C:verisimiser_vql_query, libverisimiser"
prim__vqlQuery : Bits64 -> Bits64 -> PrimIO Bits64

||| Safe wrapper: execute a VCL-total query.
||| queryPtr should point to a null-terminated VCL-total query string.
||| Returns a pointer to the result set, or Nothing on failure.
export
vqlQuery : Handle -> (queryPtr : Bits64) -> IO (Maybe Bits64)
vqlQuery h qPtr = do
  ptr <- primIO (prim__vqlQuery (handlePtr h) qPtr)
  pure $ if ptr == 0 then Nothing else Just ptr

||| Free a VCL-total query result set.
export
%foreign "C:verisimiser_vql_free_result, libverisimiser"
prim__vqlFreeResult : Bits64 -> PrimIO ()

||| Safe wrapper: free a VCL-total result set.
export
vqlFreeResult : (resultPtr : Bits64) -> IO ()
vqlFreeResult ptr = primIO (prim__vqlFreeResult ptr)

--------------------------------------------------------------------------------
-- String Operations
--------------------------------------------------------------------------------

||| Convert C string to Idris String.
export
%foreign "support:idris2_getString, libidris2_support"
prim__getString : Bits64 -> String

||| Free C string allocated by VeriSimiser.
export
%foreign "C:verisimiser_free_string, libverisimiser"
prim__freeString : Bits64 -> PrimIO ()

||| Get string result from VeriSimiser.
export
%foreign "C:verisimiser_get_string, libverisimiser"
prim__getResult : Bits64 -> PrimIO Bits64

||| Safe string getter.
export
getString : Handle -> IO (Maybe String)
getString h = do
  ptr <- primIO (prim__getResult (handlePtr h))
  if ptr == 0
    then pure Nothing
    else do
      let str = prim__getString ptr
      primIO (prim__freeString ptr)
      pure (Just str)

--------------------------------------------------------------------------------
-- Error Handling
--------------------------------------------------------------------------------

||| Get last error message.
export
%foreign "C:verisimiser_last_error, libverisimiser"
prim__lastError : PrimIO Bits64

||| Retrieve last error as string.
export
lastError : IO (Maybe String)
lastError = do
  ptr <- primIO prim__lastError
  if ptr == 0
    then pure Nothing
    else pure (Just (prim__getString ptr))

||| Get error description for result code.
export
errorDescription : Result -> String
errorDescription Ok                 = "Success"
errorDescription Error              = "Generic error"
errorDescription InvalidParam       = "Invalid parameter"
errorDescription OutOfMemory        = "Out of memory"
errorDescription NullPointer        = "Null pointer"
errorDescription ConnectionFailed   = "Database connection failed"
errorDescription ChainCorrupted     = "Provenance chain integrity violation"
errorDescription SidecarUnavailable = "Sidecar storage unavailable"

--------------------------------------------------------------------------------
-- Version Information
--------------------------------------------------------------------------------

||| Get VeriSimiser library version.
export
%foreign "C:verisimiser_version, libverisimiser"
prim__version : PrimIO Bits64

||| Get version as string.
export
version : IO String
version = do
  ptr <- primIO prim__version
  pure (prim__getString ptr)

||| Get library build info.
export
%foreign "C:verisimiser_build_info, libverisimiser"
prim__buildInfo : PrimIO Bits64

||| Get build information.
export
buildInfo : IO String
buildInfo = do
  ptr <- primIO prim__buildInfo
  pure (prim__getString ptr)

--------------------------------------------------------------------------------
-- Utility Functions
--------------------------------------------------------------------------------

||| Check if VeriSimiser instance is initialised.
export
%foreign "C:verisimiser_is_initialized, libverisimiser"
prim__isInitialized : Bits64 -> PrimIO Bits32

||| Check initialisation status.
export
isInitialized : Handle -> IO Bool
isInitialized h = do
  result <- primIO (prim__isInitialized (handlePtr h))
  pure (result /= 0)

||| Check if a specific database backend is supported.
export
%foreign "C:verisimiser_backend_supported, libverisimiser"
prim__backendSupported : Bits32 -> PrimIO Bits32

||| Safe wrapper: check backend support.
export
backendSupported : DatabaseBackend -> IO Bool
backendSupported backend = do
  result <- primIO (prim__backendSupported (backendToInt backend))
  pure (result /= 0)
