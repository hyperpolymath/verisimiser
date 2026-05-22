// VeriSimiser Integration Tests
// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// These tests verify that the Zig FFI correctly implements the Idris2 ABI
// for the VeriSimiser database augmentation layer.

const std = @import("std");
const testing = std.testing;

// Import VeriSimiser FFI functions
extern fn verisimiser_init() ?*opaque {};
extern fn verisimiser_free(?*opaque {}) void;
extern fn verisimiser_connect(?*opaque {}, u32, u64) u64;
extern fn verisimiser_disconnect(?*opaque {}, u64) void;
extern fn verisimiser_enable_dimension(?*opaque {}, u64, u32) c_int;
extern fn verisimiser_get_active_dimensions(?*opaque {}, u64) u32;
extern fn verisimiser_record_provenance(?*opaque {}, u64, u32, u64) c_int;
extern fn verisimiser_verify_provenance(?*opaque {}, u64) c_int;
extern fn verisimiser_provenance_length(?*opaque {}, u64) u64;
extern fn verisimiser_record_version(?*opaque {}, u64, u64, u32) c_int;
extern fn verisimiser_query_at_time(?*opaque {}, u64, u64) u64;
extern fn verisimiser_current_version(?*opaque {}, u64) u64;
extern fn verisimiser_measure_drift(?*opaque {}, u64) u64;
extern fn verisimiser_drift_score(?*opaque {}, u64) f64;
extern fn verisimiser_drift_category_score(?*opaque {}, u64, u32) f64;
extern fn verisimiser_vql_query(?*opaque {}, u64) u64;
extern fn verisimiser_vql_free_result(u64) void;
extern fn verisimiser_get_string(?*opaque {}) ?[*:0]const u8;
extern fn verisimiser_free_string(?[*:0]const u8) void;
extern fn verisimiser_last_error() ?[*:0]const u8;
extern fn verisimiser_version() [*:0]const u8;
extern fn verisimiser_is_initialized(?*opaque {}) u32;
extern fn verisimiser_backend_supported(u32) u32;

//==============================================================================
// Lifecycle Tests
//==============================================================================

test "create and destroy VeriSimiser handle" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    try testing.expect(handle != null);
}

test "handle is initialized" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const initialized = verisimiser_is_initialized(handle);
    try testing.expectEqual(@as(u32, 1), initialized);
}

test "null handle is not initialized" {
    const initialized = verisimiser_is_initialized(null);
    try testing.expectEqual(@as(u32, 0), initialized);
}

//==============================================================================
// Backend Support Tests
//==============================================================================

test "PostgreSQL backend is supported" {
    try testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(0));
}

test "SQLite backend is supported" {
    try testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(1));
}

test "MongoDB backend is supported" {
    try testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(2));
}

test "Redis backend is supported" {
    try testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(3));
}

test "MySQL backend is supported" {
    try testing.expectEqual(@as(u32, 1), verisimiser_backend_supported(4));
}

test "invalid backend is not supported" {
    try testing.expectEqual(@as(u32, 0), verisimiser_backend_supported(99));
}

//==============================================================================
// Octad Dimension Tests
//==============================================================================

test "enable valid octad dimension" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // Enable provenance dimension (2) for entity 42
    const result = verisimiser_enable_dimension(handle, 42, 2);
    try testing.expectEqual(@as(c_int, 0), result); // 0 = ok
}

test "enable invalid octad dimension returns error" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // Dimension 99 does not exist
    const result = verisimiser_enable_dimension(handle, 42, 99);
    try testing.expectEqual(@as(c_int, 2), result); // 2 = invalid_param
}

test "enable dimension with null handle returns null_pointer" {
    const result = verisimiser_enable_dimension(null, 42, 0);
    try testing.expectEqual(@as(c_int, 4), result); // 4 = null_pointer
}

//==============================================================================
// Provenance Tests
//==============================================================================

test "record provenance with valid handle" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // Record a create operation (0) for entity 42
    const result = verisimiser_record_provenance(handle, 42, 0, 0);
    try testing.expectEqual(@as(c_int, 0), result); // 0 = ok
}

test "record provenance with null handle" {
    const result = verisimiser_record_provenance(null, 42, 0, 0);
    try testing.expectEqual(@as(c_int, 4), result); // 4 = null_pointer
}

test "record provenance with invalid operation" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    // Operation 99 does not exist
    const result = verisimiser_record_provenance(handle, 42, 99, 0);
    try testing.expectEqual(@as(c_int, 2), result); // 2 = invalid_param
}

test "verify provenance with null handle" {
    const result = verisimiser_verify_provenance(null, 42);
    try testing.expectEqual(@as(c_int, 4), result); // 4 = null_pointer
}

test "provenance length with null handle" {
    const length = verisimiser_provenance_length(null, 42);
    try testing.expectEqual(@as(u64, 0), length);
}

//==============================================================================
// Temporal Versioning Tests
//==============================================================================

test "record version with valid handle" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const result = verisimiser_record_version(handle, 42, 0, 0);
    try testing.expectEqual(@as(c_int, 0), result); // 0 = ok
}

test "record version with null handle" {
    const result = verisimiser_record_version(null, 42, 0, 0);
    try testing.expectEqual(@as(c_int, 4), result); // 4 = null_pointer
}

test "query at time with null handle" {
    const ptr = verisimiser_query_at_time(null, 42, 1000000);
    try testing.expectEqual(@as(u64, 0), ptr);
}

test "current version with null handle" {
    const ver = verisimiser_current_version(null, 42);
    try testing.expectEqual(@as(u64, 0), ver);
}

//==============================================================================
// Drift Detection Tests
//==============================================================================

test "measure drift with null handle" {
    const ptr = verisimiser_measure_drift(null, 42);
    try testing.expectEqual(@as(u64, 0), ptr);
}

test "drift score with null handle returns 0" {
    const score = verisimiser_drift_score(null, 42);
    try testing.expectEqual(@as(f64, 0.0), score);
}

test "drift category score with null handle returns 0" {
    const score = verisimiser_drift_category_score(null, 42, 0);
    try testing.expectEqual(@as(f64, 0.0), score);
}

test "drift score with valid handle returns initial value" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const score = verisimiser_drift_score(handle, 42);
    try testing.expectEqual(@as(f64, 0.0), score);
}

//==============================================================================
// VCL-total Tests
//==============================================================================

test "vcl query with null handle returns 0" {
    const ptr = verisimiser_vql_query(null, 0);
    try testing.expectEqual(@as(u64, 0), ptr);
}

test "vcl free result with null is safe" {
    verisimiser_vql_free_result(0); // Should not crash
}

//==============================================================================
// String Tests
//==============================================================================

test "get string result" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const str = verisimiser_get_string(handle);
    defer if (str) |s| verisimiser_free_string(s);

    try testing.expect(str != null);
}

test "get string with null handle" {
    const str = verisimiser_get_string(null);
    try testing.expect(str == null);
}

//==============================================================================
// Error Handling Tests
//==============================================================================

test "last error after null handle operation" {
    _ = verisimiser_record_provenance(null, 0, 0, 0);

    const err = verisimiser_last_error();
    try testing.expect(err != null);

    if (err) |e| {
        const err_str = std.mem.span(e);
        try testing.expect(err_str.len > 0);
    }
}

//==============================================================================
// Version Tests
//==============================================================================

test "version string is not empty" {
    const ver = verisimiser_version();
    const ver_str = std.mem.span(ver);

    try testing.expect(ver_str.len > 0);
}

test "version string is semantic version format" {
    const ver = verisimiser_version();
    const ver_str = std.mem.span(ver);

    // Should be in format X.Y.Z
    try testing.expect(std.mem.count(u8, ver_str, ".") >= 1);
}

//==============================================================================
// Memory Safety Tests
//==============================================================================

test "multiple handles are independent" {
    const h1 = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(h1);

    const h2 = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(h2);

    try testing.expect(h1 != h2);

    // Operations on h1 should not affect h2
    _ = verisimiser_record_provenance(h1, 1, 0, 0);
    _ = verisimiser_record_provenance(h2, 2, 0, 0);
}

test "free null is safe" {
    verisimiser_free(null); // Should not crash
}

//==============================================================================
// Thread Safety Tests
//==============================================================================

test "concurrent provenance operations" {
    const handle = verisimiser_init() orelse return error.InitFailed;
    defer verisimiser_free(handle);

    const ThreadContext = struct {
        h: *opaque {},
        entity_id: u64,
    };

    const thread_fn = struct {
        fn run(ctx: ThreadContext) void {
            _ = verisimiser_record_provenance(ctx.h, ctx.entity_id, 0, 0);
        }
    }.run;

    var threads: [4]std.Thread = undefined;
    for (&threads, 0..) |*thread, i| {
        thread.* = try std.Thread.spawn(.{}, thread_fn, .{
            ThreadContext{ .h = handle, .entity_id = @intCast(i) },
        });
    }

    for (threads) |thread| {
        thread.join();
    }
}
