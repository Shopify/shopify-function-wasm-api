# Shopify Function WASM API - C SDK

C SDK for building Shopify Functions that compile to WebAssembly.

## Prerequisites

- `clang` with WASM target support (LLVM/Clang 14+)
- `wasm-ld` (LLVM linker, usually bundled with clang)

## Building

```bash
make echo
```

This produces `build/echo.wasm`.

## Project Structure

- `include/shopify_function.h` — WASM import declarations for the Shopify Function API
- `include/shopify_function_value.h` — NaN-box decoding helpers, value type inspection
- `src/shopify_function_value.c` — Bump allocator storage
- `examples/echo.c` — Echo example (reads input, writes it back unchanged)

## Usage

Include the headers and implement a `_start` function:

```c
#include "shopify_function_value.h"

void _start(void) {
    Val input = shopify_function_input_get();
    // Process input and write output using the API
}
```

Build with:

```bash
clang --target=wasm32-unknown-unknown -nostdlib -O2 -I include \
  -Wl,--no-entry -Wl,--export=_start -Wl,--export=memory -Wl,--allow-undefined \
  -o my_function.wasm my_function.c src/shopify_function_value.c
```
