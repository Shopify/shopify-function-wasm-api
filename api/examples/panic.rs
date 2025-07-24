use std::error::Error;

use shopify_function_wasm_api::init_panic_handler;

fn main() -> Result<(), Box<dyn Error>> {
    init_panic_handler();
    panic!("at the disco");
}
