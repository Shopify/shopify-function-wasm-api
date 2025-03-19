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
        if n.trunc() == n && n >= i32::MIN as f64 && n <= i32::MAX as f64 {
            let mut out = ValueSerializer::new();
            out.write_i32(n as i32)?;
            out.finalize()?;
        } else {
            let mut out = ValueSerializer::new();
            out.write_f64(n)?;
            out.finalize()?;
        }
    } else if let Some(s) = input.as_string() {
        let mut out = ValueSerializer::new();
        out.write_utf8_str(&s)?;
        out.finalize()?;
    } else {
        panic!("unexpected value");
    }

    Ok(())
}
