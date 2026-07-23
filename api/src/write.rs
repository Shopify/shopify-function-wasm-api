//! The write API for the Shopify Function Wasm API.
//!
//! This consists primarily of the `Serialize` trait for writing values to a [`Context`].

use std::collections::HashMap;

use crate::Context;
use crate::InternedStringId;
use shopify_function_wasm_api_core::write::WriteResult;

/// An identifier for a shape defined with [`Context::define_shape`] or
/// [`Context::define_shape_from_interned`].
///
/// A shape declares the ordered keys used by one or more shaped objects written in the same
/// context.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShapeId(usize);

/// An error that can occur when writing a value.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An I/O error occurred.
    #[error("I/O error")]
    IoError,
    /// Expected a string value, corresponding to a key in an object, but got a different type.
    #[error("Expected a key")]
    ExpectedKey,
    /// The object length was not honoured. This can occur if you write more key-value pairs than the length specified,
    /// or if you try to finalize the object without writing the specified number of key-value pairs.
    #[error("Object length error")]
    ObjectLengthError,
    /// The value was already written. This can occur if you write the value multiple times.
    #[error("Value already written")]
    ValueAlreadyWritten,
    /// The value is not an object, but was expected to be one based on the current context (e.g. when attempting to finalize an object).
    #[error("Not an object")]
    NotAnObject,
    /// The value was not finished, but `Context::finalize_output` was called.
    #[error("Value not finished")]
    ValueNotFinished,
    /// The array length was not honoured. This can occur if you write more values than the length specified,
    /// or if you try to finalize the array without writing the specified number of values.
    #[error("Array length error")]
    ArrayLengthError,
    /// The value is not an array, but was expected to be one based on the current context.
    #[error("Not an array")]
    NotAnArray,
    /// The value is not a shaped object, but was expected to be one based on the current context.
    #[error("Not a shaped object")]
    NotAShape,
    /// The shaped object length was not honoured. This occurs when the number of values written
    /// does not match the number of keys in the shape.
    #[error("Shaped object length error")]
    ShapeLengthError,
    /// The shape ID does not refer to a shape defined in the current context.
    #[error("Invalid shape ID")]
    InvalidShapeId,
    /// The shape definition operation was invalid.
    #[error("Shape definition error")]
    ShapeDefinitionError,
    /// An unknown error occurred. This occurs when a new error code is added that this version of the API does not know about.
    #[error("Unknown error")]
    Unknown,
}

fn map_result(result: usize) -> Result<(), Error> {
    match WriteResult::from_repr(result) {
        Some(WriteResult::Ok) => Ok(()),
        Some(WriteResult::IoError) => Err(Error::IoError),
        Some(WriteResult::ExpectedKey) => Err(Error::ExpectedKey),
        Some(WriteResult::ObjectLengthError) => Err(Error::ObjectLengthError),
        Some(WriteResult::ValueAlreadyWritten) => Err(Error::ValueAlreadyWritten),
        Some(WriteResult::NotAnObject) => Err(Error::NotAnObject),
        Some(WriteResult::ValueNotFinished) => Err(Error::ValueNotFinished),
        Some(WriteResult::ArrayLengthError) => Err(Error::ArrayLengthError),
        Some(WriteResult::NotAnArray) => Err(Error::NotAnArray),
        Some(WriteResult::NotAShape) => Err(Error::NotAShape),
        Some(WriteResult::ShapeLengthError) => Err(Error::ShapeLengthError),
        Some(WriteResult::InvalidShapeId) => Err(Error::InvalidShapeId),
        Some(WriteResult::ShapeDefinitionError) => Err(Error::ShapeDefinitionError),
        None => Err(Error::Unknown),
    }
}

impl Context {
    /// Write a boolean value.
    pub fn write_bool(&mut self, value: bool) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_bool(value as u32) })
    }

    /// Write a null value.
    pub fn write_null(&mut self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_null() })
    }

    /// Write an i32 value.
    pub fn write_i32(&mut self, value: i32) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_i32(value) })
    }

    /// Write a f64 value.
    pub fn write_f64(&mut self, value: f64) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_f64(value) })
    }

    /// Write a UTF-8 string value.
    pub fn write_utf8_str(&mut self, value: &str) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_utf8_str(value.as_ptr(), value.len())
        })
    }

    /// Write an interned UTF-8 string value.
    pub fn write_interned_utf8_str(&mut self, id: InternedStringId) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_interned_utf8_str(id.as_usize()) })
    }

    /// Define a shape from UTF-8 keys.
    ///
    /// The returned shape can be reused to write multiple objects with the same keys. Values in a
    /// shaped object must be written in the same order as the keys passed here.
    pub fn define_shape(&mut self, keys: &[&str]) -> Result<ShapeId, Error> {
        map_result(unsafe { crate::shopify_function_output_shape_define_new(keys.len()) })?;
        for key in keys {
            let id = self.intern_utf8_str(key);
            map_result(unsafe { crate::shopify_function_output_shape_define_key(id.as_usize()) })?;
        }
        self.finish_shape_definition()
    }

    /// Define a shape from previously interned UTF-8 string IDs.
    ///
    /// The returned shape can be reused to write multiple objects with the same keys. Values in a
    /// shaped object must be written in the same order as the keys passed here.
    pub fn define_shape_from_interned(
        &mut self,
        keys: &[InternedStringId],
    ) -> Result<ShapeId, Error> {
        map_result(unsafe { crate::shopify_function_output_shape_define_new(keys.len()) })?;
        for id in keys {
            map_result(unsafe { crate::shopify_function_output_shape_define_key(id.as_usize()) })?;
        }
        self.finish_shape_definition()
    }

    fn finish_shape_definition(&mut self) -> Result<ShapeId, Error> {
        let packed = unsafe { crate::shopify_function_output_shape_define_finish() };
        let result = (packed >> usize::BITS) as usize;
        let shape_id = packed as usize;
        map_result(result)?;
        Ok(ShapeId(shape_id))
    }

    /// Write an object. You must provide the exact number of key-value pairs you will write.
    pub fn write_object<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_object(len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_object() })
    }

    /// Write an array. You must provide the exact number of values you will write.
    pub fn write_array<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_array(len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_array() })
    }

    /// Write a shaped object using values in the order declared by its shape.
    ///
    /// Exactly one value must be written for each key in the shape. A shape can be reused to write
    /// multiple objects.
    ///
    /// # Example
    /// ```rust
    /// use shopify_function_wasm_api::Context;
    ///
    /// let mut context = Context::new_with_input(serde_json::json!({}));
    /// let shape = context.define_shape(&["value"]).unwrap();
    /// context
    ///     .write_shaped_object(shape, |ctx| ctx.write_i32(1))
    ///     .unwrap();
    /// let output = context.finalize_output_and_return().unwrap();
    /// assert_eq!(output, serde_json::json!({ "value": 1 }));
    /// ```
    pub fn write_shaped_object<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        shape: ShapeId,
        f: F,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_shaped_object(shape.0) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_shaped_object() })
    }

    #[cfg(not(target_family = "wasm"))]
    /// Finalize the output and return the serialized value as a `serde_json::Value`.
    /// This is only available in non-Wasm targets, and therefore only recommended for use in tests.
    pub fn finalize_output_and_return(self) -> Result<serde_json::Value, Error> {
        let (result, bytes) =
            shopify_function_provider::write::shopify_function_output_finalize_and_return_bytes();
        map_result(result as usize)
            .and_then(|_| fbf::from_slice::<serde_json::Value>(&bytes).map_err(|_| Error::IoError))
    }
}

/// A trait for types that can be serialized.
///
/// # Example
/// ```rust
/// use shopify_function_wasm_api::{Context, Serialize, write::Error};
///
/// struct MyStruct {
///     value: i32,
/// }
///
/// impl Serialize for MyStruct {
///     fn serialize(&self, context: &mut Context) -> Result<(), Error> {
///         context.write_object(|ctx| {
///             ctx.write_utf8_str("value")?;
///             ctx.write_i32(self.value)
///         }, 1)
///     }
/// }
///
/// let mut context = Context::new_with_input(serde_json::json!({}));
/// let my_struct = MyStruct { value: 1 };
/// my_struct.serialize(&mut context).unwrap();
/// let output = context.finalize_output_and_return().unwrap();
/// let expected = serde_json::json!({ "value": 1 });
/// assert_eq!(output, expected);
/// ```
pub trait Serialize {
    /// Serialize the value.
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

    #[test]
    fn test_shaped_objects_nested_in_arrays_and_objects() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let shape = context.define_shape(&["x", "y"]).unwrap();

        context
            .write_object(
                |ctx| {
                    ctx.write_utf8_str("items")?;
                    ctx.write_array(
                        |ctx| {
                            ctx.write_shaped_object(shape, |ctx| {
                                ctx.write_i32(1)?;
                                ctx.write_i32(2)
                            })
                        },
                        1,
                    )?;
                    ctx.write_utf8_str("point")?;
                    ctx.write_shaped_object(shape, |ctx| {
                        ctx.write_i32(3)?;
                        ctx.write_i32(4)
                    })
                },
                2,
            )
            .unwrap();

        assert_eq!(
            context.finalize_output_and_return().unwrap(),
            serde_json::json!({
                "items": [{ "x": 1, "y": 2 }],
                "point": { "x": 3, "y": 4 }
            })
        );
    }

    #[test]
    fn test_reuse_shape_for_multiple_objects() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let x = context.intern_utf8_str("x");
        let y = context.intern_utf8_str("y");
        let shape = context.define_shape_from_interned(&[x, y]).unwrap();

        context
            .write_array(
                |ctx| {
                    ctx.write_shaped_object(shape, |ctx| {
                        ctx.write_i32(1)?;
                        ctx.write_i32(2)
                    })?;
                    ctx.write_shaped_object(shape, |ctx| {
                        ctx.write_i32(3)?;
                        ctx.write_i32(4)
                    })
                },
                2,
            )
            .unwrap();

        assert_eq!(
            context.finalize_output_and_return().unwrap(),
            serde_json::json!([{ "x": 1, "y": 2 }, { "x": 3, "y": 4 }])
        );
    }

    #[test]
    fn test_shaped_object_wrong_value_count() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let shape = context.define_shape(&["value"]).unwrap();

        let result = context.write_shaped_object(shape, |_| Ok(()));

        assert!(matches!(result, Err(Error::ShapeLengthError)));
    }

    #[test]
    fn test_shaped_object_with_undefined_shape() {
        let shape = {
            let mut context = Context::new_with_input(serde_json::json!({}));
            context.define_shape(&["value"]).unwrap()
        };
        let mut context = Context::new_with_input(serde_json::json!({}));

        let result = context.write_shaped_object(shape, |_| Ok(()));

        assert!(matches!(result, Err(Error::InvalidShapeId)));
    }

    #[test]
    fn test_shape_write_results_map_to_distinct_errors() {
        assert!(matches!(
            map_result(WriteResult::NotAShape as usize),
            Err(Error::NotAShape)
        ));
        assert!(matches!(
            map_result(WriteResult::ShapeDefinitionError as usize),
            Err(Error::ShapeDefinitionError)
        ));
    }
}
