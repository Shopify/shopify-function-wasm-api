const std = @import("std");

pub fn build(b: *std.Build) void {
    const optimize = b.standardOptimizeOption(.{});

    const target = b.resolveTargetQuery(.{
        .cpu_arch = .wasm32,
        .os_tag = .freestanding,
    });

    const sf_module = b.createModule(.{
        .root_source_file = b.path("src/shopify_function.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Build the echo example
    const echo = addExample(b, "echo", "examples/echo.zig", target, optimize, sf_module);
    b.installArtifact(echo);

    // Build the cart-checkout-validation example
    const cart = addExample(b, "cart-checkout-validation", "examples/cart-checkout-validation.zig", target, optimize, sf_module);
    b.installArtifact(cart);

    // Build the interned-echo example
    const interned = addExample(b, "interned-echo", "examples/interned-echo.zig", target, optimize, sf_module);
    b.installArtifact(interned);
}

fn addExample(
    b: *std.Build,
    name: []const u8,
    source: []const u8,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    sf_module: *std.Build.Module,
) *std.Build.Step.Compile {
    const exe = b.addExecutable(.{
        .name = name,
        .root_source_file = b.path(source),
        .target = target,
        .optimize = optimize,
    });

    exe.root_module.addImport("shopify_function", sf_module);
    exe.entry = .disabled;
    exe.rdynamic = true;
    exe.export_memory = true;
    exe.initial_memory = 65536 * 18;

    return exe;
}
