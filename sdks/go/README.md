# Shopify Function WASM API - Go SDK (TinyGo)

Go SDK for building Shopify Functions that compile to WebAssembly using TinyGo.

## Prerequisites

- [TinyGo](https://tinygo.org/) 0.30+
- Go 1.22+

## Building

```bash
make echo
```

This produces `build/echo.wasm`.

## Project Structure

- `shopify_function/imports.go` — WASM import declarations (`//go:wasmimport`)
- `shopify_function/value.go` — Value type, NaN-box decoding, input reading
- `shopify_function/output.go` — Output builder helpers, string interning, logging
- `examples/echo/main.go` — Echo example (reads input, writes it back unchanged)

## Usage

Import the package and implement your logic in `main()`:

```go
package main

import sf "github.com/Shopify/shopify-function-wasm-api/sdks/go/shopify_function"

func main() {
    input := sf.InputGet()
    // Process input and write output using the API
}
```

TinyGo's `wasm` target automatically generates a `_start` export that calls `main()`.

Build with:

```bash
tinygo build -o my_function.wasm -target wasm -no-debug -scheduler=none ./my_function/
```
