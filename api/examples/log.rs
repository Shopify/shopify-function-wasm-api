use std::error::Error;

use shopify_function_wasm_api::Context;

fn main() -> Result<(), Box<dyn Error>> {
    shopify_function_wasm_api::init_panic_handler();
    let mut context = Context::new();
    context.log("Hi!\n");
    context.log("Hello\n");
    context.log("Here's a third string\n");
    context.log("✌️\n");
    Ok(())
}
