#[cfg(target_family = "wasm")]
use shopify_function_wasm_api::{Context, InternedStringId, Value};
#[cfg(target_family = "wasm")]
use std::io::Write;

#[cfg(not(target_family = "wasm"))]
use shopify_function_wasm_api::{InternedStringId, Value};

use std::error::Error;

// Uses a mix of interned and non-interned strings
#[cfg(target_family = "wasm")]
fn main() -> Result<(), Box<dyn Error>> {
    let context = Context::new();
    let input = context.input_get()?;
    let interned_key = context.intern_utf8_str("key");
    let mut out = std::io::stdout();

    let serialized = format!("got value {}\n", serialize_value(input, interned_key));
    out.write_all(serialized.as_bytes())?;
    out.flush()?;

    Ok(())
}

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), Box<dyn Error>> {
    panic!("This example is only supported in a WASM target");
}

#[allow(dead_code)]
fn serialize_value(value: Value, interned_key: InternedStringId) -> String {
    if let Some(boolean) = value.as_bool() {
        format!("{}", boolean)
    } else if value.is_null() {
        "null".to_string()
    } else if let Some(number) = value.as_number() {
        format!("{}", number)
    } else if let Some(string) = value.as_string() {
        string
    } else if value.is_obj() {
        let value_for_key = value.get_interned_obj_prop(interned_key);
        let value_for_other_key = value.get_obj_prop("other_key");
        format!(
            "obj; key: {}, other_key: {}",
            serialize_value(value_for_key, interned_key),
            serialize_value(value_for_other_key, interned_key)
        )
    } else if let Some(array_len) = value.array_len() {
        let elements = (0..array_len)
            .map(|i| serialize_value(value.get_at_index(i), interned_key))
            .collect::<Vec<String>>();

        format!("array; [{}]", elements.join(", "))
    } else {
        "unknown value".to_string()
    }
}

#[cfg(not(target_family = "wasm"))]
#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_serialize_value_with_obj_input() {
        let input = json!({"other_key": "other_value", "key": "Hello, world!"});
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "obj; key: Hello, world!, other_key: other_value");
    }

    #[test]
    fn test_serialize_value_with_array_input() {
        let input = json!([1, 2, 3]);
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "array; [1, 2, 3]");
    }

    #[test]
    fn test_serialize_value_with_string_input() {
        let input = json!("Hello, world!");
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_serialize_value_with_number_input() {
        let input = json!(123);
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "123");
    }

    #[test]
    fn test_serialize_value_with_boolean_input() {
        let input = json!(true);
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "true");
    }

    #[test]
    fn test_serialize_value_with_null_input() {
        let input = json!(null);
        let mut value = Value::new(input);
        let interned_key = value.intern_utf8_str("key");
        let result = serialize_value(value, interned_key);
        assert_eq!(result, "null");
    }
}
