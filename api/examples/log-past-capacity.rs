use std::error::Error;

use shopify_function_wasm_api::Context;

fn main() -> Result<(), Box<dyn Error>> {
    shopify_function_wasm_api::init_panic_handler();
    let context = Context::new();
    context.log(&"a".repeat(1020));
    context.log(&"b".repeat(10));
    Ok(())
}
