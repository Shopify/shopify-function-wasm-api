# Shopify Function WASM API - Zig SDK

Zig SDK for building Shopify Functions that compile to WebAssembly.

## Prerequisites

- Zig 0.13+

## Building

```bash
zig build -Doptimize=ReleaseSmall
```

This produces `zig-out/bin/echo.wasm`.

## Project Structure

- `src/shopify_function.zig` — API bindings, Value type, NaN-box decoding, output helpers
- `examples/echo.zig` — Echo example (reads input, writes it back unchanged)
- `build.zig` — Build configuration

## Usage

Import the module and implement a `_start` function:

```zig
const sf = @import("shopify_function");

export fn _start() void {
    const input = sf.inputGet();
    // Process input and write output using the API
}
```
