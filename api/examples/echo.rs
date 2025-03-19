use shopify_function_wasm_api::{input_get, ValueSerializer};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = input_get();

    if let Some(b) = input.as_bool() {
        let mut out = ValueSerializer::new();
        out.write_bool(b)?;
        out.finalize()?;
    } else if let Some(()) = input.as_null() {
        let mut out = ValueSerializer::new();
        out.write_null()?;
        out.finalize()?;
    } else if let Some(n) = input.as_number() {
        if n.trunc() == n {
            let mut out = ValueSerializer::new();
            out.write_int(n as i32)?;
            out.finalize()?;
        } else {
            panic!("unexpected value")
        }
    } else {
        panic!("unexpected value");
    }

    Ok(())
}
