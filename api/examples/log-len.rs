use std::error::Error;

use shopify_function_wasm_api::Context;

fn main() -> Result<(), Box<dyn Error>> {
    shopify_function_wasm_api::init_panic_handler();
    let context = Context::new();
    let input = context.input_get()?;
    let len = input.as_number().unwrap() as usize;
    for _ in 0..len / 100 {
        eprintln!("{}", &"a".repeat(100));
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn run() {
    main().unwrap()
}
