# Shopify Function WASM API

A high-performance API for building Shopify Functions using WebAssembly (WASM).

## Architecture

The WASM API consists of these main components:

1. **Provider (`provider/`)**
    - Implements low-level WASM operations for:
        - Reading function input
        - Serializing the function output

2. **Core (`core/`)**
    - Defines common types used by the `providers` and `api`

3. **API (`api/`)**
    - Provides a high-level interface for interacting with the provider
    - Abstracts away low-level WASM details
    - Includes examples and documentation

4. **Trampoline (`trampoline/`)**
    - CLI tool that augments WASM modules to interface with the provider
    - Handles memory sharing between guest and provider modules
    - Creates the necessary WASM imports/exports

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)

### Basic Usage

Here's a simple example of how to use the API:

```rust
fn main(context: &mut Context) -> Result<()> {
    let input = context.input_get()?;

    // Function logic

    context.finalize_output()?;
    
    Ok(())
}
```

To build a function example, create a new example and build it targeting `wasm32-wasip1`:

```shell
cargo build --release --target wasm32-wasip1 -p shopify_function_wasm_api --example echo
```


The trampoline tool bridges communication between your Wasm module and the provider module. To trampoline your Wasm module:

```shell
# Short flags
cargo run -p shopify_function_trampoline -- -i input.wasm -o output.wasm
```

For examples, check out the [examples directory](./api/examples/).

## Documentation

For more detailed documentation, refer to:

- [Examples](./api/examples)
- [Integration Tests](./integration_tests/tests/integration_test.rs)

## Contributing

Contributions are welcome! Please read our [Contributing Guide](./CONTRIBUTING.md) and [Code of Conduct](./CODE_OF_CONDUCT.md) before submitting a pull request.

## License

This project is licensed under the [MIT License](./LICENSE.md).
