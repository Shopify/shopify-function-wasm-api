const std = @import("std");

pub fn build(b: *std.Build) void {
    const optimize = b.standardOptimizeOption(.{});

    // Build the echo example
    const echo = b.addExecutable(.{
        .name = "echo",
        .root_source_file = b.path("examples/echo.zig"),
        .target = b.resolveTargetQuery(.{
            .cpu_arch = .wasm32,
            .os_tag = .freestanding,
        }),
        .optimize = optimize,
    });

    // Add the SDK source as a module
    echo.root_module.addImport("shopify_function", b.createModule(.{
        .root_source_file = b.path("src/shopify_function.zig"),
        .target = b.resolveTargetQuery(.{
            .cpu_arch = .wasm32,
            .os_tag = .freestanding,
        }),
        .optimize = optimize,
    }));

    echo.entry = .disabled;
    echo.rdynamic = true;
    echo.export_memory = true;
    echo.initial_memory = 65536 * 18; // 18 pages (~1.1MB)

    b.installArtifact(echo);
}
