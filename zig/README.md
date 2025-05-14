# Shopify Function WASM API for Zig

A Zig implementation of the Shopify Function WebAssembly API, equivalent to the Rust implementation.

## Architecture

The WASM API consists of these main components:

1. **Core (`core/`)**
   - Defines common types used by the API
   - Implements NanBox for value encoding
   - Defines error codes and write results

2. **API (`api/`)**
   - Provides a high-level interface for interacting with the Shopify Function host
   - Abstracts away low-level WASM details
   - Includes examples and documentation

## Getting Started

### Prerequisites

- [Zig](https://ziglang.org/download/) (latest version)

### Basic Usage

Here's a simple example of how to use the API:

```zig
const std = @import("std");
const sf = @import("shopify_function_wasm_api");

pub fn main() !void {
    var context = sf.Context.init();
    
    const input = try context.inputGet();
    
    // Function logic
    
    try context.finalizeOutput();
}
```

To build a function example, use the Zig build system:

```shell
zig build example-cart-checkout -Dtarget=wasm32-wasi
```

## Documentation

For more detailed documentation, refer to:

- [Examples](./api/examples)

## Comparison with Rust API

This Zig implementation is a direct port of the Rust API with equivalent functionality:

- NanBox implementation for value encoding
- Deserialization and serialization capabilities
- Context and value handling
- Object and array manipulation
- Error handling

The main differences are:

1. Zig uses comptime generics instead of Rust traits
2. Error handling is done with Zig's error union type instead of Result
3. Memory management uses Zig's allocators explicitly
4. Object/Array writing uses anonymous structs with closures

## Examples

The included cart checkout validation example demonstrates how to:

1. Read input from the context
2. Process a cart to validate quantities
3. Generate a response with any validation errors
4. Finalize the output

The API makes it simple to work with complex nested data structures in a type-safe manner.