# Shopify Function Wasm API

A high-performance API for building Shopify Functions using WebAssembly (Wasm).

## Architecture

The Wasm API consists of these main components:

1. **Provider (`provider/`)**
    - Implements low-level Wasm operations for:
        - Reading function input from FBF (Functions Binary Format)
        - Serializing function output as FBF

2. **Core (`core/`)**
    - Defines common types used by the `providers` and `api`

3. **API (`api/`)**
    - Provides a high-level interface for interacting with the provider
    - Abstracts away low-level Wasm details
    - Includes examples and documentation

4. **Trampoline (`trampoline/`)**
    - CLI tool that augments Wasm modules to interface with the provider
    - Handles memory sharing between guest and provider modules
    - Creates the necessary Wasm imports/exports

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)

### Basic Usage

Here's a simple example of how to use the API:

```rust
fn main(context: &mut Context) -> Result<()> {
    shopify_function_wasm_api::init_panic_handler();
    let input = context.input_get()?;

    // Function logic

    Ok(())
}
```

Function inputs and outputs use [FBF (Functions Binary Format)](https://github.com/Shopify/functions-binary-format), including its string and shape tables and length-framed containers.

When writing many objects with the same keys, define the key order once with `Context::define_shape` and reuse the returned shape with `Context::write_shaped_object`. This avoids writing the keys for every object. See the [shapes example](./api/examples/shapes.rs) for a complete function.

To build a function example, create a new example and build it targeting `wasm32-unknown-unknown`:

```shell
cargo build --release --target wasm32-unknown-unknown -p shopify_function_wasm_api --example echo
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
