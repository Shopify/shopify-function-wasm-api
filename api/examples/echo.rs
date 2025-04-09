use shopify_function_wasm_api::{write::Error as WriteError, Context, InternedStringId, Value};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();
    let input = context.input_get()?;

    let foo_key = context.intern_utf8_str("foo");
    let bar_key = context.intern_utf8_str("bar");
    let known_keys = [foo_key, bar_key];

    serialize_value(input, &mut context, &known_keys)?;
    context.finalize_output()?;

    Ok(())
}

fn serialize_value(
    value: Value,
    out: &mut Context,
    known_keys: &[InternedStringId],
) -> Result<(), WriteError> {
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
                for key in known_keys {
                    let value = value.get_obj_prop(*key);
                    out.write_interned_utf8_str(*key)?;
                    serialize_value(value, out, known_keys)?;
                }
                Ok(())
            },
            2,
        )
    } else if let Some(len) = value.array_len() {
        out.write_array(
            |out| {
                for i in 0..len {
                    serialize_value(value.get_at_index(i), out, known_keys)?;
                }
                Ok(())
            },
            len,
        )
    } else {
        panic!("unexpected value");
    }
}
