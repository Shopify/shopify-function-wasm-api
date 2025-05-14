const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Add the core module
    const core_mod = b.addModule("shopify_function_wasm_api_core", .{
        .root_source_file = b.path("core/src/lib.zig"),
    });

    // Add the API module
    const api_mod = b.addModule("shopify_function_wasm_api", .{
        .root_source_file = b.path("api/src/lib.zig"),
        .imports = &.{
            .{ .name = "shopify_function_wasm_api_core", .module = core_mod },
        },
    });

    // Example executables
    const cart_checkout_example = b.addExecutable(.{
        .name = "cart-checkout-validation",
        .root_source_file = b.path("api/examples/cart-checkout-validation.zig"),
        .target = target,
        .optimize = optimize,
    });
    
    // No additional size optimizations for now - we'll use the -Doptimize=ReleaseSmall flag
    
    // Add module dependencies to the executable
    cart_checkout_example.root_module.addImport("shopify_function_wasm_api", api_mod);
    cart_checkout_example.root_module.addImport("shopify_function_wasm_api_core", core_mod);

    b.installArtifact(cart_checkout_example);

    const cart_example_run_cmd = b.addRunArtifact(cart_checkout_example);
    cart_example_run_cmd.step.dependOn(b.getInstallStep());

    const cart_example_step = b.step("example-cart-checkout", "Run the cart checkout validation example");
    cart_example_step.dependOn(&cart_example_run_cmd.step);
}