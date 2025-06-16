use std::error::Error;

use shopify_function_wasm_api::log::log_utf8_str;

fn main() -> Result<(), Box<dyn Error>> {
    log_utf8_str("Hi!\n");
    log_utf8_str("Hello\n");
    log_utf8_str("Here's a third string\n");
    log_utf8_str("✌️\n");
    Ok(())
}
