use std::collections::HashMap;

use crate::Context;
use crate::InternedStringId;
use shopify_function_wasm_api_core::write::WriteResult;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error")]
    IoError,
    #[error("Expected a key")]
    ExpectedKey,
    #[error("Object length error")]
    ObjectLengthError,
    #[error("Value already written")]
    ValueAlreadyWritten,
    #[error("Not an object")]
    NotAnObject,
    #[error("Value not finished")]
    ValueNotFinished,
    #[error("Array length error")]
    ArrayLengthError,
    #[error("Not an array")]
    NotAnArray,
}

fn map_result(result: WriteResult) -> Result<(), Error> {
    match result {
        WriteResult::Ok => Ok(()),
        WriteResult::IoError => Err(Error::IoError),
        WriteResult::ExpectedKey => Err(Error::ExpectedKey),
        WriteResult::ObjectLengthError => Err(Error::ObjectLengthError),
        WriteResult::ValueAlreadyWritten => Err(Error::ValueAlreadyWritten),
        WriteResult::NotAnObject => Err(Error::NotAnObject),
        WriteResult::ValueNotFinished => Err(Error::ValueNotFinished),
        WriteResult::ArrayLengthError => Err(Error::ArrayLengthError),
        WriteResult::NotAnArray => Err(Error::NotAnArray),
    }
}

impl Context {
    pub fn write_bool(&mut self, value: bool) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_bool(self.0 as _, value as u32) })
    }

    pub fn write_null(&mut self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_null(self.0 as _) })
    }

    pub fn write_i32(&mut self, value: i32) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_i32(self.0 as _, value) })
    }

    pub fn write_f64(&mut self, value: f64) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_f64(self.0 as _, value) })
    }

    pub fn write_utf8_str(&mut self, value: &str) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_utf8_str(self.0 as _, value.as_ptr(), value.len())
        })
    }

    pub fn write_interned_utf8_str(&mut self, id: InternedStringId) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_interned_utf8_str(self.0 as _, id.as_usize())
        })
    }

    pub fn write_object<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_object(self.0 as _, len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_object(self.0 as _) })
    }

    pub fn write_array<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_array(self.0 as _, len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_array(self.0 as _) })
    }

    pub fn finalize_output(self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_finalize(self.0 as _) })
    }

    #[cfg(not(target_family = "wasm"))]
    /// Finalize the output and return the serialized value as a `serde_json::Value`.
    /// This is only available in non-WASM targets, and therefore only recommended for use in tests.
    pub fn finalize_output_and_return(self) -> Result<serde_json::Value, Error> {
        let (result, bytes) = shopify_function_wasm_api_provider::write::shopify_function_output_finalize_and_return_msgpack_bytes(self.0 as _);
        map_result(result).and_then(|_| rmp_serde::from_slice(&bytes).map_err(|_| Error::IoError))
    }
}

pub trait Serialize {
    fn serialize(&self, context: &mut Context) -> Result<(), Error>;
}

impl Serialize for bool {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_bool(*self)
    }
}

impl Serialize for () {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_null()
    }
}

impl Serialize for i32 {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_i32(*self)
    }
}

impl Serialize for f64 {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_f64(*self)
    }
}

impl Serialize for str {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_utf8_str(self)
    }
}

impl Serialize for String {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_utf8_str(self)
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_array(
            |context| {
                for item in self {
                    item.serialize(context)?;
                }
                Ok(())
            },
            self.len(),
        )
    }
}

impl<T: Serialize> Serialize for [T] {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_array(
            |context| {
                for item in self {
                    item.serialize(context)?;
                }
                Ok(())
            },
            self.len(),
        )
    }
}

impl<K: AsRef<str>, V: Serialize> Serialize for HashMap<K, V> {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        context.write_object(
            |context| {
                for (key, value) in self {
                    key.as_ref().serialize(context)?;
                    value.serialize(context)?;
                }
                Ok(())
            },
            self.len(),
        )
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, context: &mut Context) -> Result<(), Error> {
        match self {
            Some(value) => value.serialize(context),
            None => context.write_null(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialize_and_return<T: Serialize + ?Sized>(value: &T) -> serde_json::Value {
        let mut context = Context::new_with_input(serde_json::json!({}));
        value.serialize(&mut context).unwrap();
        context.finalize_output_and_return().unwrap()
    }

    #[test]
    fn test_bool_serialize() {
        [true, false].into_iter().for_each(|value| {
            let result = serialize_and_return(&value);
            assert_eq!(result, serde_json::json!(value));
        });
    }

    #[test]
    fn test_void_serialize() {
        let result = serialize_and_return(&());
        assert_eq!(result, serde_json::json!(null));
    }

    #[test]
    fn test_i32_serialize() {
        [0, 1, -1, i32::MAX, i32::MIN]
            .into_iter()
            .for_each(|value| {
                let result = serialize_and_return(&value);
                assert_eq!(result, serde_json::json!(value));
            });
    }

    #[test]
    fn test_f64_serialize() {
        [0.0, 1.0, -1.0, f64::MAX, f64::MIN]
            .into_iter()
            .for_each(|value| {
                let result = serialize_and_return(&value);
                assert_eq!(result, serde_json::json!(value));
            });
    }

    #[test]
    fn test_str_serialize() {
        ["", "a", "Hello, world!"].into_iter().for_each(|value| {
            let result = serialize_and_return(value);
            assert_eq!(result, serde_json::json!(value));
        });
    }

    #[test]
    fn test_slice_serialize() {
        let value: &[i32] = &[1, 2, 3];
        let result = serialize_and_return(value);
        assert_eq!(result, serde_json::json!(value));
    }

    #[test]
    fn test_string_serialize() {
        let value = String::from("Hello, world!");
        let result = serialize_and_return(&value);
        assert_eq!(result, serde_json::json!(value));
    }

    #[test]
    fn test_option_serialize() {
        [Some(1), None].into_iter().for_each(|option| {
            let result = serialize_and_return(&option);
            assert_eq!(result, serde_json::json!(option));
        });
    }
}
