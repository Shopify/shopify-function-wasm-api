const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const core_dep = b.dependency("shopify_function_wasm_api_core", .{
        .target = target,
        .optimize = optimize,
    });

    const core_module = core_dep.module("shopify_function_wasm_api_core");

    const lib = b.addStaticLibrary(.{
        .name = "shopify_function_wasm_api",
        .root_source_file = .{ .path = "src/lib.zig" },
        .target = target,
        .optimize = optimize,
    });

    lib.addModule("shopify_function_wasm_api_core", core_module);
    b.installArtifact(lib);

    const module = b.addModule("shopify_function_wasm_api", .{
        .source_file = .{ .path = "src/lib.zig" },
        .dependencies = &.{
            .{ .name = "shopify_function_wasm_api_core", .module = core_module },
        },
    });

    _ = module;

    // Tests
    const main_tests = b.addTest(.{
        .root_source_file = .{ .path = "src/lib.zig" },
        .target = target,
        .optimize = optimize,
    });
    main_tests.addModule("shopify_function_wasm_api_core", core_module);

    const run_main_tests = b.addRunArtifact(main_tests);
    const test_step = b.step("test", "Run library tests");
    test_step.dependOn(&run_main_tests.step);

    // Examples
    const cart_checkout_example = b.addExecutable(.{
        .name = "cart-checkout-validation",
        .root_source_file = .{ .path = "examples/cart-checkout-validation.zig" },
        .target = target,
        .optimize = optimize,
    });
    cart_checkout_example.addModule("shopify_function_wasm_api", module);
    cart_checkout_example.addModule("shopify_function_wasm_api_core", core_module);

    const cart_checkout_example_step = b.step("example-cart-checkout", "Build the cart checkout validation example");
    cart_checkout_example_step.dependOn(&b.addInstallArtifact(cart_checkout_example, {}).step);
}