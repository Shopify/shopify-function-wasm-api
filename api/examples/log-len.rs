use std::error::Error;

use shopify_function_wasm_api::Context;

#[cfg_attr(target_family = "wasm", export_name = "_start")]
fn main() {
    run().unwrap()
}

fn run() -> Result<(), Box<dyn Error>> {
    shopify_function_wasm_api::init_panic_handler();
    let mut context = Context::new();
    let input = context.input_get()?;
    let len = input.as_number().unwrap() as usize;
    for _ in 0..len / 100 {
        context.log(&"a".repeat(100));
    }
    Ok(())
}
