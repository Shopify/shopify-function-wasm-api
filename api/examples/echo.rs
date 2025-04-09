use shopify_function_wasm_api::{write::Error as WriteError, Context, Value};
use std::error::Error;

const KNOWN_KEYS: [&str; 2] = ["foo", "bar"];

fn main() -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();
    let input = context.input_get()?;

    serialize_value(input, &mut context)?;
    context.finalize_output()?;

    Ok(())
}

fn serialize_value(value: Value, out: &mut Context) -> Result<(), WriteError> {
    if let Some(b) = value.as_bool() {
        out.write_bool(b)
    } else if let Some(()) = value.as_null() {
        out.write_null()
    } else if let Some(n) = value.as_number() {
        if n.trunc() == n && n >= i32::MIN as f64 && n <= i32::MAX as f64 {
            out.write_i32(n as i32)
        } else {
            out.write_f64(n)
        }
    } else if let Some(s) = value.as_string() {
        out.write_utf8_str(&s)
    } else if value.is_obj() {
        out.write_object(
            |out| {
                for key in KNOWN_KEYS {
                    let value = value.get_obj_prop(key);
                    out.write_utf8_str(key)?;
                    serialize_value(value, out)?;
                }
                Ok(())
            },
            2,
        )
    } else if let Some(len) = value.array_len() {
        out.write_array(
            |out| {
                for i in 0..len {
                    serialize_value(value.get_at_index(i), out)?;
                }
                Ok(())
            },
            len,
        )
    } else {
        panic!("unexpected value");
    }
}
