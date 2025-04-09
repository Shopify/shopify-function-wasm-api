use shopify_function_wasm_api::{Context, Value};
use std::{error::Error, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let context = Context::new();
    let input = context.input_get()?;
    let mut out = std::io::stdout();
    let serialized = format!("got value {}\n", serialize_value(input));
    out.write_all(serialized.as_bytes())?;
    out.flush()?;

    Ok(())
}

fn serialize_value(value: Value) -> String {
    if let Some(boolean) = value.as_bool() {
        format!("{}", boolean)
    } else if value.as_null().is_some() {
        "null".to_string()
    } else if let Some(number) = value.as_number() {
        format!("{}", number)
    } else if let Some(string) = value.as_string() {
        string
    } else if value.is_obj() {
        let value_for_key = value.get_obj_prop("key");
        let value_for_other_key = value.get_obj_prop("other_key");
        format!(
            "obj; key: {}, other_key: {}",
            serialize_value(value_for_key),
            serialize_value(value_for_other_key)
        )
    } else if let Some(array_len) = value.array_len() {
        let elements = (0..array_len)
            .map(|i| serialize_value(value.get_at_index(i)))
            .collect::<Vec<String>>();

        format!("array; [{}]", elements.join(", "))
    } else {
        "unknown value".to_string()
    }
}
