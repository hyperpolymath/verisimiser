// VeriSimiser FFI Build Configuration
// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Shared library (.so, .dylib, .dll).
    // linkLibC: the implementation uses std.heap.c_allocator.
    const lib = b.addSharedLibrary(.{
        .name = "verisimiser",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib.linkLibC();

    // Static library (.a).
    const lib_static = b.addStaticLibrary(.{
        .name = "verisimiser",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib_static.linkLibC();

    // Install artifacts.
    b.installArtifact(lib);
    b.installArtifact(lib_static);

    // Unit tests (in-module tests in src/main.zig).
    const lib_tests = b.addTest(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib_tests.linkLibC();
    const run_lib_tests = b.addRunArtifact(lib_tests);

    // Integration tests (test/integration_test.zig) exercise the exported
    // C-ABI symbols through the actual compiled shared library.
    const integration_tests = b.addTest(.{
        .root_source_file = b.path("test/integration_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    integration_tests.linkLibrary(lib);
    integration_tests.linkLibC();
    const run_integration_tests = b.addRunArtifact(integration_tests);

    // `zig build test` runs BOTH the unit tests and the C-ABI integration tests.
    const test_step = b.step("test", "Run VeriSimiser FFI unit + integration tests");
    test_step.dependOn(&run_lib_tests.step);
    test_step.dependOn(&run_integration_tests.step);

    // `zig build test-unit` runs only the in-module unit tests.
    const unit_test_step = b.step("test-unit", "Run VeriSimiser FFI unit tests only");
    unit_test_step.dependOn(&run_lib_tests.step);

    // `zig build test-integration` runs only the C-ABI integration tests.
    const integration_test_step = b.step("test-integration", "Run VeriSimiser integration tests only");
    integration_test_step.dependOn(&run_integration_tests.step);

    // Documentation.
    const docs = b.addTest(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = .Debug,
    });
    const docs_step = b.step("docs", "Generate VeriSimiser FFI documentation");
    docs_step.dependOn(&b.addInstallDirectory(.{
        .source_dir = docs.getEmittedDocs(),
        .install_dir = .prefix,
        .install_subdir = "docs",
    }).step);
}
