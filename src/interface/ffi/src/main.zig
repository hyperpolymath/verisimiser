// VeriSimiser FFI Implementation
//
// Implements the C-compatible FFI declared in src/interface/abi/Foreign.idr.
// VeriSimiser augments existing databases with VeriSimDB octad capabilities:
// drift detection, provenance tracking, temporal versioning, and modality overlays.
//
// All types and layouts must match the Idris2 ABI definitions in Types.idr and Layout.idr.
//
// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

const std = @import("std");

// Version information (keep in sync with Cargo.toml)
const VERSION = "0.1.0";
const BUILD_INFO = "VeriSimiser built with Zig " ++ @import("builtin").zig_version_string;

/// Thread-local error storage
threadlocal var last_error: ?[]const u8 = null;

/// Set the last error message
fn setError(msg: []const u8) void {
    last_error = msg;
}

/// Clear the last error
fn clearError() void {
    last_error = null;
}

//==============================================================================
// Core Types (must match src/interface/abi/Types.idr)
//==============================================================================

/// Result codes (must match Idris2 Result type)
pub const Result = enum(c_int) {
    ok = 0,
    @"error" = 1,
    invalid_param = 2,
    out_of_memory = 3,
    null_pointer = 4,
    connection_failed = 5,
    chain_corrupted = 6,
    sidecar_unavailable = 7,
};

/// Octad dimension tags (must match Idris2 OctadDimension type)
pub const OctadDimension = enum(u32) {
    data = 0,
    metadata = 1,
    provenance = 2,
    lineage = 3,
    constraints = 4,
    access_control = 5,
    temporal = 6,
    simulation = 7,
};

/// Database backend identifiers (must match Idris2 DatabaseBackend type)
pub const DatabaseBackend = enum(u32) {
    postgresql = 0,
    sqlite = 1,
    mongodb = 2,
    redis = 3,
    mysql = 4,
};

/// Provenance operations (must match Idris2 ProvenanceOperation type)
pub const ProvenanceOperation = enum(u32) {
    create = 0,
    update = 1,
    delete = 2,
    transform = 3,
};

/// Drift categories (must match Idris2 DriftCategory type)
pub const DriftCategory = enum(u32) {
    structural = 0,
    semantic = 1,
    temporal = 2,
    statistical = 3,
    referential = 4,
    provenance = 5,
    spatial = 6,
    embedding = 7,
};

/// Bitmask of active octad dimensions for an entity.
pub const DimensionMask = u32;

/// VeriSimiser library handle (opaque to C callers).
/// Holds the augmentation state, sidecar connections, and configuration.
///
/// This is a normal Zig struct, never an `extern struct`: it holds a
/// `std.mem.Allocator` (which is not extern-compatible) and is only ever
/// crossed the C boundary as an opaque `?*Handle` pointer, so its layout is
/// private to this module.
pub const Handle = struct {
    allocator: std.mem.Allocator,
    initialized: bool,
    backend: DatabaseBackend,
    db_connected: bool,
    // Sidecar handles (null if not enabled)
    provenance_enabled: bool,
    temporal_enabled: bool,
    drift_enabled: bool,
};

/// Database connection handle (opaque to C callers).
/// Normal struct for the same reason as `Handle`: it holds an allocator and
/// only crosses the C boundary as an opaque pointer.
pub const DbConnection = struct {
    allocator: std.mem.Allocator,
    backend: DatabaseBackend,
    connected: bool,
};

//==============================================================================
// Library Lifecycle
//==============================================================================

/// Initialise the VeriSimiser library.
/// Returns a handle, or null on failure.
export fn verisimiser_init() ?*Handle {
    const allocator = std.heap.c_allocator;

    const handle = allocator.create(Handle) catch {
        setError("Failed to allocate VeriSimiser handle");
        return null;
    };

    handle.* = .{
        .allocator = allocator,
        .initialized = true,
        .backend = .postgresql, // default backend
        .db_connected = false,
        .provenance_enabled = false,
        .temporal_enabled = false,
        .drift_enabled = false,
    };

    clearError();
    return handle;
}

/// Free the VeriSimiser handle and all associated resources.
export fn verisimiser_free(handle: ?*Handle) void {
    const h = handle orelse return;
    const allocator = h.allocator;

    h.initialized = false;
    h.db_connected = false;

    allocator.destroy(h);
    clearError();
}

//==============================================================================
// Database Connection
//==============================================================================

/// Connect to a target database backend.
/// backend_id: DatabaseBackend enum value.
/// conn_str_ptr: pointer to null-terminated connection string.
export fn verisimiser_connect(
    handle: ?*Handle,
    backend_id: u32,
    conn_str_ptr: u64,
) u64 {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return 0;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return 0;
    }

    _ = conn_str_ptr; // TODO: parse connection string

    const allocator = h.allocator;
    const db = allocator.create(DbConnection) catch {
        setError("Failed to allocate database connection");
        return 0;
    };

    const backend = std.meta.intToEnum(DatabaseBackend, backend_id) catch {
        setError("Invalid database backend");
        allocator.destroy(db);
        return 0;
    };

    db.* = .{
        .allocator = allocator,
        .backend = backend,
        .connected = true,
    };

    h.backend = backend;
    h.db_connected = true;

    clearError();
    return @intFromPtr(db);
}

/// Disconnect from the target database.
export fn verisimiser_disconnect(handle: ?*Handle, db_ptr: u64) void {
    const h = handle orelse return;
    if (!h.initialized) return;

    if (db_ptr != 0) {
        const db: *DbConnection = @ptrFromInt(db_ptr);
        db.connected = false;
        db.allocator.destroy(db);
        h.db_connected = false;
    }
    clearError();
}

//==============================================================================
// Octad Overlay Operations
//==============================================================================

/// Enable an octad dimension for an entity.
export fn verisimiser_enable_dimension(
    handle: ?*Handle,
    entity_id: u64,
    dimension: u32,
) Result {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return .null_pointer;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return .@"error";
    }

    _ = entity_id;

    // Validate dimension enum
    _ = std.meta.intToEnum(OctadDimension, dimension) catch {
        setError("Invalid octad dimension");
        return .invalid_param;
    };

    // The overlay index that persists per-entity dimension state is not yet
    // wired into the FFI. Fail loudly rather than report a dimension as enabled
    // when it is not (soundness: no silent success).
    setError("octad dimension overlay not yet wired into the FFI");
    return .sidecar_unavailable;
}

/// Get the active dimension bitmask for an entity.
export fn verisimiser_get_active_dimensions(
    handle: ?*Handle,
    entity_id: u64,
) u32 {
    const h = handle orelse return 0;
    if (!h.initialized) return 0;
    _ = entity_id;

    // TODO: look up the entity's active dimensions from the overlay index
    // Return bitmask: bit 0 = Data, bit 1 = Metadata, etc.
    return 0;
}

//==============================================================================
// Tier 1: Provenance Tracking
//==============================================================================

/// Record a provenance event (appends to SHA-256 hash chain).
export fn verisimiser_record_provenance(
    handle: ?*Handle,
    entity_id: u64,
    operation: u32,
    actor_ptr: u64,
) Result {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return .null_pointer;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return .@"error";
    }

    _ = entity_id;
    _ = actor_ptr;

    // Validate operation enum
    _ = std.meta.intToEnum(ProvenanceOperation, operation) catch {
        setError("Invalid provenance operation");
        return .invalid_param;
    };

    // The SHA-256 hash-chain sidecar append is not yet wired into the FFI.
    // Fail loudly rather than report a provenance event as recorded when no
    // entry was written (soundness: no phantom audit trail).
    setError("provenance sidecar not yet wired into the FFI");
    return .sidecar_unavailable;
}

/// Verify the integrity of an entity's provenance hash chain.
export fn verisimiser_verify_provenance(
    handle: ?*Handle,
    entity_id: u64,
) Result {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return .null_pointer;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return .@"error";
    }

    _ = entity_id;

    // The hash-chain store is not yet wired into the FFI, so integrity cannot
    // be confirmed. Return an error rather than .ok — reporting "verified" for
    // an unchecked (possibly tampered) chain would be the worst kind of
    // soundness hole.
    setError("provenance sidecar not yet wired into the FFI; cannot verify integrity");
    return .sidecar_unavailable;
}

/// Get the length of an entity's provenance chain.
export fn verisimiser_provenance_length(
    handle: ?*Handle,
    entity_id: u64,
) u64 {
    const h = handle orelse return 0;
    if (!h.initialized) return 0;
    _ = entity_id;

    // TODO: count provenance entries for entity
    return 0;
}

//==============================================================================
// Tier 1: Temporal Versioning
//==============================================================================

/// Record a temporal snapshot for an entity.
export fn verisimiser_record_version(
    handle: ?*Handle,
    entity_id: u64,
    snapshot_ptr: u64,
    snapshot_len: u32,
) Result {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return .null_pointer;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return .@"error";
    }

    _ = entity_id;
    _ = snapshot_ptr;
    _ = snapshot_len;

    // The temporal sidecar that stores version snapshots is not yet wired into
    // the FFI. Fail loudly rather than report a version as recorded when no
    // snapshot was stored (soundness: no phantom history).
    setError("temporal sidecar not yet wired into the FFI");
    return .sidecar_unavailable;
}

/// Query entity state at a specific point in time.
/// Returns pointer to serialised snapshot, or 0 if not found.
export fn verisimiser_query_at_time(
    handle: ?*Handle,
    entity_id: u64,
    timestamp: u64,
) u64 {
    const h = handle orelse return 0;
    if (!h.initialized) return 0;
    _ = entity_id;
    _ = timestamp;

    // TODO: binary search temporal sidecar for version valid at timestamp
    return 0;
}

/// Get the current version number for an entity.
export fn verisimiser_current_version(
    handle: ?*Handle,
    entity_id: u64,
) u64 {
    const h = handle orelse return 0;
    if (!h.initialized) return 0;
    _ = entity_id;

    // TODO: look up latest version number
    return 0;
}

//==============================================================================
// Tier 1: Drift Detection
//==============================================================================

/// Measure cross-modal drift for an entity.
/// Returns pointer to DriftMeasurement struct, or 0 if entity not found.
export fn verisimiser_measure_drift(
    handle: ?*Handle,
    entity_id: u64,
) u64 {
    const h = handle orelse return 0;
    if (!h.initialized) return 0;
    _ = entity_id;

    // TODO: compute drift scores across all 8 categories
    return 0;
}

/// Get the overall drift score for an entity (0.0 = consistent, 1.0 = diverged).
export fn verisimiser_drift_score(
    handle: ?*Handle,
    entity_id: u64,
) f64 {
    const h = handle orelse return 0.0;
    if (!h.initialized) return 0.0;
    _ = entity_id;

    // TODO: compute aggregate drift score
    return 0.0;
}

/// Get drift score for a specific category.
export fn verisimiser_drift_category_score(
    handle: ?*Handle,
    entity_id: u64,
    category: u32,
) f64 {
    const h = handle orelse return 0.0;
    if (!h.initialized) return 0.0;
    _ = entity_id;
    _ = category;

    // TODO: look up per-category drift score
    return 0.0;
}

//==============================================================================
// VCL-total Query Interface
//==============================================================================

/// Execute a VCL-total query against the augmented database.
/// query_ptr: pointer to null-terminated VCL-total query string.
/// Returns pointer to result set, or 0 on failure.
export fn verisimiser_vql_query(
    handle: ?*Handle,
    query_ptr: u64,
) u64 {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return 0;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return 0;
    }

    _ = query_ptr;

    // TODO: parse VCL-total query, plan execution, return result set
    return 0;
}

/// Free a VCL-total query result set.
export fn verisimiser_vql_free_result(result_ptr: u64) void {
    if (result_ptr == 0) return;
    // TODO: free result set memory
}

//==============================================================================
// String Operations
//==============================================================================

/// Get a string result from VeriSimiser.
/// Caller must free the returned string with verisimiser_free_string.
export fn verisimiser_get_string(handle: ?*Handle) ?[*:0]const u8 {
    const h = handle orelse {
        setError("Null VeriSimiser handle");
        return null;
    };

    if (!h.initialized) {
        setError("VeriSimiser handle not initialized");
        return null;
    }

    const result = h.allocator.dupeZ(u8, "VeriSimiser octad augmentation active") catch {
        setError("Failed to allocate string");
        return null;
    };

    clearError();
    return result.ptr;
}

/// Free a string allocated by VeriSimiser.
export fn verisimiser_free_string(str: ?[*:0]const u8) void {
    const s = str orelse return;
    const allocator = std.heap.c_allocator;

    const slice = std.mem.span(s);
    allocator.free(slice);
}

//==============================================================================
// Error Handling
//==============================================================================

/// Get the last error message.
/// Returns null if no error.
export fn verisimiser_last_error() ?[*:0]const u8 {
    const err = last_error orelse return null;

    const allocator = std.heap.c_allocator;
    const c_str = allocator.dupeZ(u8, err) catch return null;
    return c_str.ptr;
}

//==============================================================================
// Version Information
//==============================================================================

/// Get the VeriSimiser library version.
export fn verisimiser_version() [*:0]const u8 {
    return VERSION.ptr;
}

/// Get build information.
export fn verisimiser_build_info() [*:0]const u8 {
    return BUILD_INFO.ptr;
}

//==============================================================================
// Utility Functions
//==============================================================================

/// Check if VeriSimiser handle is initialised.
export fn verisimiser_is_initialized(handle: ?*Handle) u32 {
    const h = handle orelse return 0;
    return if (h.initialized) 1 else 0;
}

/// Check if a database backend is supported.
export fn verisimiser_backend_supported(backend_id: u32) u32 {
    // All defined backends are supported
    _ = std.meta.intToEnum(DatabaseBackend, backend_id) catch return 0;
    return 1;
}

//==============================================================================
// Tests
//==============================================================================

test "lifecycle" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    try std.testing.expect(verisimiser_is_initialized(handle) == 1);
}

test "error handling" {
    const result = verisimiser_record_provenance(null, 0, 0, 0);
    try std.testing.expectEqual(Result.null_pointer, result);

    const err = verisimiser_last_error();
    try std.testing.expect(err != null);
}

test "version" {
    const ver = verisimiser_version();
    const ver_str = std.mem.span(ver);
    try std.testing.expectEqualStrings(VERSION, ver_str);
}

test "backend supported" {
    // PostgreSQL
    try std.testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(0));
    // SQLite
    try std.testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(1));
    // Invalid
    try std.testing.expectEqual(@as(u32, 0), verisimiser_backend_supported(99));
}

test "provenance with null handle" {
    const result = verisimiser_record_provenance(null, 42, 0, 0);
    try std.testing.expectEqual(Result.null_pointer, result);
}

test "verify provenance with null handle" {
    const result = verisimiser_verify_provenance(null, 42);
    try std.testing.expectEqual(Result.null_pointer, result);
}

test "drift score with null handle" {
    const score = verisimiser_drift_score(null, 42);
    try std.testing.expectEqual(@as(f64, 0.0), score);
}

test "enable dimension with invalid dimension" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const result = verisimiser_enable_dimension(handle, 42, 99);
    try std.testing.expectEqual(Result.invalid_param, result);
}

// Soundness: the persistence-backed octad operations are not yet wired into
// the FFI, so they must fail loudly rather than report a false success.

test "verify provenance does not falsely report verified" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // A valid handle + entity must NOT return .ok while verification is unwired:
    // claiming a chain is verified without checking it would be unsound.
    const result = verisimiser_verify_provenance(handle, 42);
    try std.testing.expect(result != Result.ok);
    try std.testing.expectEqual(Result.sidecar_unavailable, result);
}

test "record provenance does not falsely report recorded" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const result = verisimiser_record_provenance(handle, 42, 0, 0);
    try std.testing.expect(result != Result.ok);
    try std.testing.expectEqual(Result.sidecar_unavailable, result);
}

test "record version does not falsely report stored" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const result = verisimiser_record_version(handle, 42, 0, 0);
    try std.testing.expect(result != Result.ok);
    try std.testing.expectEqual(Result.sidecar_unavailable, result);
}

test "enable dimension does not falsely report enabled" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // Dimension 2 (provenance) is a valid enum value; the call must still fail
    // loudly because the overlay index is not wired in.
    const result = verisimiser_enable_dimension(handle, 42, 2);
    try std.testing.expect(result != Result.ok);
    try std.testing.expectEqual(Result.sidecar_unavailable, result);
}
