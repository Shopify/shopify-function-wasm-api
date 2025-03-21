use shopify_function_wasm_api::{input_get, ValueSerializer};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = input_get();

    if let Some(b) = input.as_bool() {
        let mut out = ValueSerializer::new();
        out.write_bool(b)?;
        out.finalize()?;
    } else {
        panic!("expected bool");
    }

    Ok(())
}
