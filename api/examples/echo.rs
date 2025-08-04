use shopify_function_wasm_api::{
    read::Error as ReadError, write::Error as WriteError, CachedInternedStringId, Context,
    Deserialize, Serialize, Value as ApiValue,
};
use std::error::Error;

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
    Object(Vec<(String, Self)>),
    Array(Vec<Self>),
}

static FOO_INTERNED_STRING_ID: CachedInternedStringId = CachedInternedStringId::new("foo");
static BAR_INTERNED_STRING_ID: CachedInternedStringId = CachedInternedStringId::new("bar");

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
        } else if let Some(obj_len) = value.obj_len() {
            let mut object = Vec::new();
            for i in 0..obj_len {
                let key = value.get_obj_key_at_index(i).expect("Failed to get key");
                // special case to exercise string interning and get_obj_prop
                let raw_value = match key.as_str() {
                    "foo" => {
                        let interned_string_id = FOO_INTERNED_STRING_ID.load();
                        value.get_interned_obj_prop(interned_string_id)
                    }
                    "bar" => {
                        let interned_string_id = BAR_INTERNED_STRING_ID.load();
                        value.get_interned_obj_prop(interned_string_id)
                    }
                    "abc" | "def" => value.get_obj_prop(key.as_str()),
                    _ => value.get_at_index(i),
                };
                let value = Self::deserialize(&raw_value)?;
                object.push((key, value));
            }
            Ok(Value::Object(object))
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
            Value::Object(object) => out.write_object(
                |ctx| {
                    for (key, value) in object {
                        match key.as_str() {
                            "foo" => {
                                let interned_string_id = FOO_INTERNED_STRING_ID.load();
                                ctx.write_interned_utf8_str(interned_string_id)?;
                            }
                            "bar" => {
                                let interned_string_id = BAR_INTERNED_STRING_ID.load();
                                ctx.write_interned_utf8_str(interned_string_id)?;
                            }
                            _ => ctx.write_utf8_str(key)?,
                        }
                        value.serialize(ctx)?;
                    }
                    Ok(())
                },
                object.len(),
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

        assert_eq!(result, Value::Object(Vec::new()));
    }
}
