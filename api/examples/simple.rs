use shopify_function_wasm_api::input_get;
use std::{error::Error, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let input = input_get();
    let mut out = std::io::stdout();
    let serialized = if let Some(boolean) = input.as_bool() {
        format!("got value {}\n", boolean)
    } else if input.as_null().is_some() {
        "got value null\n".to_string()
    } else if let Some(number) = input.as_number() {
        format!("got value {}\n", number)
    } else if let Some(string) = input.as_string() {
        format!("got value {}\n", string)
    } else {
        "got unknown value\n".to_string()
    };
    out.write_all(serialized.as_bytes())?;
    out.flush()?;

    Ok(())
}
