use shopify_function_wasm_api::{input_get, intern_utf8_str, InternedStringId, Value};
use std::{error::Error, io::Write, sync::LazyLock};

fn main() -> Result<(), Box<dyn Error>> {
    let input = input_get();
    let mut out = std::io::stdout();
    let serialized = format!("got value {}\n", serialize_value(input));
    out.write_all(serialized.as_bytes())?;
    out.flush()?;

    Ok(())
}

static KEY: LazyLock<InternedStringId> = LazyLock::new(|| intern_utf8_str("key"));
static OTHER_KEY: LazyLock<InternedStringId> = LazyLock::new(|| intern_utf8_str("other_key"));

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
        let value_for_key = value.get_obj_prop(*KEY);
        let value_for_other_key = value.get_obj_prop(*OTHER_KEY);
        format!(
            "obj; key: {}, other_key: {}",
            serialize_value(value_for_key),
            serialize_value(value_for_other_key)
        )
    } else if let Some(array_len) = value.array_len() {
        let elements = (0..array_len)
            .map(|i| serialize_value(value.get_at_index(i as u32)))
            .collect::<Vec<String>>();

        format!("array; [{}]", elements.join(", "))
    } else {
        "unknown value".to_string()
    }
}
