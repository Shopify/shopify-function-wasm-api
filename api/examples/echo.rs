use shopify_function_wasm_api::{
    read::Error as ReadError, write::Error as WriteError, Context, Deserialize, InternedStringId,
    Serialize, Value as ApiValue,
};
use std::{error::Error, sync::OnceLock};

fn main() -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();
    let input = context.input_get()?;

    let value = Value::deserialize(&input)?;
    let result = echo(value);
    result.serialize(&mut context)?;
    context.finalize_output()?;

    Ok(())
}

fn echo(value: Value) -> Value {
    value
}

#[derive(Debug, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Integer(i32),
    Float(f64),
    String(String),
    /// There is no way to dynamically get the keys of an object, so we just hardcode these two known keys.
    Object {
        foo_value: Box<Self>,
        bar_value: Box<Self>,
    },
    Array(Vec<Self>),
}

static FOO_INTERNED_STRING_ID: OnceLock<InternedStringId> = OnceLock::new();
static BAR_INTERNED_STRING_ID: OnceLock<InternedStringId> = OnceLock::new();

impl Deserialize for Value {
    fn deserialize(value: &ApiValue) -> Result<Self, ReadError> {
        if value.is_null() {
            Ok(Value::Null)
        } else if let Some(b) = value.as_bool() {
            Ok(Value::Bool(b))
        } else if let Some(n) = value.as_number() {
            if n.trunc() == n && n >= i32::MIN as f64 && n <= i32::MAX as f64 {
                Ok(Value::Integer(n as i32))
            } else {
                Ok(Value::Float(n))
            }
        } else if let Some(s) = value.as_string() {
            Ok(Value::String(s.to_string()))
        } else if value.is_obj() {
            let foo_interned_string_id =
                *FOO_INTERNED_STRING_ID.get_or_init(|| value.intern_utf8_str("foo"));
            let bar_interned_string_id =
                *BAR_INTERNED_STRING_ID.get_or_init(|| value.intern_utf8_str("bar"));
            Ok(Value::Object {
                foo_value: Box::new(Self::deserialize(
                    &value.get_interned_obj_prop(foo_interned_string_id),
                )?),
                bar_value: Box::new(Self::deserialize(
                    &value.get_interned_obj_prop(bar_interned_string_id),
                )?),
            })
        } else if let Some(arr_len) = value.array_len() {
            let mut arr = Vec::with_capacity(arr_len);
            for i in 0..arr_len {
                arr.push(Self::deserialize(&value.get_at_index(i))?);
            }
            Ok(Value::Array(arr))
        } else {
            Err(ReadError::InvalidType)
        }
    }
}

impl Serialize for Value {
    fn serialize(&self, out: &mut Context) -> Result<(), WriteError> {
        match self {
            Value::Null => out.write_null(),
            Value::Bool(b) => out.write_bool(*b),
            Value::Integer(n) => out.write_i32(*n),
            Value::Float(n) => out.write_f64(*n),
            Value::String(s) => out.write_utf8_str(s),
            Value::Object {
                foo_value,
                bar_value,
            } => out.write_object(
                |ctx| {
                    let foo_interned_string_id =
                        *FOO_INTERNED_STRING_ID.get_or_init(|| ctx.intern_utf8_str("foo"));
                    let bar_interned_string_id =
                        *BAR_INTERNED_STRING_ID.get_or_init(|| ctx.intern_utf8_str("bar"));
                    ctx.write_interned_utf8_str(foo_interned_string_id)?;
                    foo_value.serialize(ctx)?;
                    ctx.write_interned_utf8_str(bar_interned_string_id)?;
                    bar_value.serialize(ctx)?;
                    Ok(())
                },
                2,
            ),
            Value::Array(arr) => out.write_array(
                |out| {
                    for value in arr {
                        value.serialize(out)?;
                    }
                    Ok(())
                },
                arr.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let input = serde_json::json!({});
        let context = Context::new_with_input(input);
        let api_value = context.input_get().unwrap();
        let input: Value = Deserialize::deserialize(&api_value).unwrap();
        let result = echo(input);

        assert_eq!(
            result,
            Value::Object {
                foo_value: Box::new(Value::Null),
                bar_value: Box::new(Value::Null),
            }
        );
    }
}
