//! The read API for the Shopify Function Wasm API.
//!
//! This consists primarily of the `Deserialize` trait for converting [`Value`] into other types.

use crate::Value;
use std::collections::HashMap;

/// An error that can occur when deserializing a value.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The value is not of the expected type.
    #[error("Invalid type")]
    InvalidType,
}

/// A trait for types that can be deserialized from a [`Value`].
///
/// # Example
/// ```rust
/// use shopify_function_wasm_api::{Context, Deserialize, Value, read::Error};
///
/// #[derive(Debug, PartialEq)]
/// struct MyStruct {
///     value: i32,
/// }
///
/// impl Deserialize for MyStruct {
///     fn deserialize(value: &Value) -> Result<Self, Error> {
///         if !value.is_obj() {
///             return Err(Error::InvalidType);
///         }
///         let value = i32::deserialize(&value.get_obj_prop("value"))?;
///         Ok(MyStruct { value })
///     }
/// }
///
/// let context = Context::new_with_input(serde_json::json!({ "value": 1 }));
/// let value = context.input_get().unwrap();
/// let my_struct = MyStruct::deserialize(&value).unwrap();
/// assert_eq!(my_struct, MyStruct { value: 1 });
/// ```
pub trait Deserialize: Sized {
    /// Deserialize a value from a [`Value`].
    fn deserialize(value: &Value) -> Result<Self, Error>;
}

impl Deserialize for Value {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        Ok(*value)
    }
}

impl Deserialize for bool {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_bool().ok_or(Error::InvalidType)
    }
}

macro_rules! impl_deserialize_for_int {
    ($ty:ty) => {
        impl Deserialize for $ty {
            fn deserialize(value: &Value) -> Result<Self, Error> {
                value
                    .as_number()
                    .and_then(|n| {
                        if n.trunc() == n && n >= <$ty>::MIN as f64 && n <= <$ty>::MAX as f64 {
                            Some(n as $ty)
                        } else {
                            None
                        }
                    })
                    .ok_or(Error::InvalidType)
            }
        }
    };
}

impl_deserialize_for_int!(i8);
impl_deserialize_for_int!(i16);
impl_deserialize_for_int!(i32);
impl_deserialize_for_int!(i64);
impl_deserialize_for_int!(u8);
impl_deserialize_for_int!(u16);
impl_deserialize_for_int!(u32);
impl_deserialize_for_int!(u64);
impl_deserialize_for_int!(usize);
impl_deserialize_for_int!(isize);

impl Deserialize for f64 {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_number().ok_or(Error::InvalidType)
    }
}

impl Deserialize for String {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_string().ok_or(Error::InvalidType)
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::deserialize(value)?))
        }
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        if let Some(len) = value.array_len() {
            let mut vec = Vec::with_capacity(len);
            for i in 0..len {
                vec.push(T::deserialize(&value.get_at_index(i))?);
            }
            Ok(vec)
        } else {
            Err(Error::InvalidType)
        }
    }
}

impl<T: Deserialize> Deserialize for HashMap<String, T> {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        let Some(obj_len) = value.obj_len() else {
            return Err(Error::InvalidType);
        };

        let mut map = HashMap::new();

        for i in 0..obj_len {
            let key = value.get_obj_key_at_index(i).ok_or(Error::InvalidType)?;
            let value = value.get_at_index(i);
            map.insert(key, T::deserialize(&value)?);
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    fn deserialize_json_value<T: Deserialize>(value: serde_json::Value) -> Result<T, Error> {
        let context = Context::new_with_input(value);
        let value = context.input_get().unwrap();
        T::deserialize(&value)
    }

    #[test]
    fn test_deserialize_bool() {
        [true, false].iter().for_each(|&b| {
            let value = serde_json::json!(b);
            let result: bool = deserialize_json_value(value).unwrap();
            assert_eq!(result, b);
        });
    }

    #[test]
    fn test_deserialize_hash_map() {
        let value = serde_json::json!({
            "key1": "value1",
            "key2": "value2",
        });
        let result: HashMap<String, String> = deserialize_json_value(value).unwrap();
        let expected = HashMap::from([
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ]);
        assert_eq!(result, expected);
    }
}
