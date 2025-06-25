//! The write API for the Shopify Function Wasm API.
//!
//! This consists primarily of the `Serialize` trait for writing values to a [`Context`].

use std::collections::HashMap;

use crate::Context;
use crate::InternedStringId;
use shopify_function_wasm_api_core::write::WriteResult;

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
        None => Err(Error::Unknown),
    }
}

/// A helper for counting fields before writing objects with conditional fields.
pub struct FieldCounter {
    count: usize,
}

impl FieldCounter {
    /// Create a new field counter.
    pub fn new() -> Self {
        Self { count: 0 }
    }

    /// Get the current field count.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Count a field if the value would be written.
    pub fn count_field<T: Serialize + ?Sized>(&mut self, _value: &T) {
        self.count += 1;
    }

    /// Count a field only if the optional value is Some.
    pub fn count_optional_field<T: Serialize>(&mut self, value: &Option<T>) {
        if value.is_some() {
            self.count += 1;
        }
    }
}

/// A helper for writing objects with conditional fields that automatically handles field counting and key writing.
pub struct ConditionalObjectWriter {
    fields: Vec<(String, Box<dyn FnOnce(&mut Context) -> Result<(), Error>>)>,
}

impl ConditionalObjectWriter {
    /// Create a new conditional object writer.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Get the number of fields that will be written.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Add a field that will always be written.
    pub fn field<T: Serialize + ToOwned + ?Sized>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), Error>
    where
        T::Owned: Serialize + 'static,
    {
        let key = key.to_string();
        let value = value.to_owned();
        self.fields
            .push((key, Box::new(move |ctx| value.serialize(ctx))));
        Ok(())
    }

    /// Add a field only if the optional value is Some.
    pub fn optional_field<T: Serialize + Clone + 'static>(
        &mut self,
        key: &str,
        value: &Option<T>,
    ) -> Result<(), Error> {
        if let Some(ref val) = value {
            let key = key.to_string();
            let val = val.clone();
            self.fields
                .push((key, Box::new(move |ctx| val.serialize(ctx))));
        }
        Ok(())
    }
}

impl Context {
    /// Write a boolean value.
    pub fn write_bool(&mut self, value: bool) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_bool(self.0 as _, value as u32) })
    }

    /// Write a null value.
    pub fn write_null(&mut self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_null(self.0 as _) })
    }

    /// Write an i32 value.
    pub fn write_i32(&mut self, value: i32) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_i32(self.0 as _, value) })
    }

    /// Write a f64 value.
    pub fn write_f64(&mut self, value: f64) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_f64(self.0 as _, value) })
    }

    /// Write a UTF-8 string value.
    pub fn write_utf8_str(&mut self, value: &str) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_utf8_str(self.0 as _, value.as_ptr(), value.len())
        })
    }

    /// Write an interned UTF-8 string value.
    pub fn write_interned_utf8_str(&mut self, id: InternedStringId) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_interned_utf8_str(self.0 as _, id.as_usize())
        })
    }

    /// Write an object. You must provide the exact number of key-value pairs you will write.
    pub fn write_object<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_object(self.0 as _, len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_object(self.0 as _) })
    }

    /// Write an object with conditional fields using a two-pass approach.
    ///
    /// This method first counts the fields that will be written, then writes the object.
    /// Use this with `SerializeOptional` to skip null fields and reduce output size.
    pub fn write_object_conditional<C, W>(
        &mut self,
        count_fields: C,
        write_fields: W,
    ) -> Result<(), Error>
    where
        C: FnOnce(&mut FieldCounter),
        W: FnOnce(&mut Self) -> Result<(), Error>,
    {
        // First pass: count fields
        let mut counter = FieldCounter::new();
        count_fields(&mut counter);

        // Second pass: write object with correct field count
        self.write_object(write_fields, counter.count())
    }

    /// Write an object with conditional fields
    pub fn write_object_with_conditional_fields<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut ConditionalObjectWriter) -> Result<(), Error>,
    {
        let mut writer = ConditionalObjectWriter::new();
        f(&mut writer)?;

        let field_count = writer.field_count();
        map_result(unsafe { crate::shopify_function_output_new_object(self.0 as _, field_count) })?;

        for (key, value_fn) in writer.fields {
            self.write_utf8_str(&key)?;
            value_fn(self)?;
        }

        map_result(unsafe { crate::shopify_function_output_finish_object(self.0 as _) })
    }

    /// Write an array. You must provide the exact number of values you will write.
    pub fn write_array<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_array(self.0 as _, len) })?;
        f(self)?;
        map_result(unsafe { crate::shopify_function_output_finish_array(self.0 as _) })
    }

    /// Finalize the output. This must be called exactly once, and must be called after all other writes.
    pub fn finalize_output(self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_finalize(self.0 as _) })
    }

    #[cfg(not(target_family = "wasm"))]
    /// Finalize the output and return the serialized value as a `serde_json::Value`.
    /// This is only available in non-Wasm targets, and therefore only recommended for use in tests.
    pub fn finalize_output_and_return(self) -> Result<serde_json::Value, Error> {
        let (result, bytes) = shopify_function_provider::write::shopify_function_output_finalize_and_return_msgpack_bytes(self.0 as _);
        map_result(result as usize)
            .and_then(|_| rmp_serde::from_slice(&bytes).map_err(|_| Error::IoError))
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

/// A trait for types that can optionally skip serialization when None.
///
/// This trait allows for conditional field inclusion in objects, which can help
/// reduce output size and avoid writing null values for optional fields.
pub trait SerializeOptional {
    /// Serialize the value if present, skip if None.
    ///
    /// Returns `true` if a value was written, `false` if the field was skipped.
    fn serialize_optional(&self, context: &mut Context) -> Result<bool, Error>;
}

impl<T: Serialize> SerializeOptional for Option<T> {
    fn serialize_optional(&self, context: &mut Context) -> Result<bool, Error> {
        match self {
            Some(value) => {
                value.serialize(context)?;
                Ok(true) // Field was written
            }
            None => Ok(false), // Field was skipped
        }
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
    fn test_serialize_optional_some() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let option_value: Option<i32> = Some(42);

        let was_written = option_value.serialize_optional(&mut context).unwrap();
        assert!(was_written);

        let result = context.finalize_output_and_return().unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    #[test]
    fn test_serialize_optional_none() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let option_value: Option<i32> = None;

        let was_written = option_value.serialize_optional(&mut context).unwrap();
        assert!(!was_written);

        // Since nothing was written, we can't finalize - this is expected behavior
        // The SerializeOptional trait is meant to be used within objects
    }

    #[test]
    fn test_field_counter() {
        let mut counter = FieldCounter::new();
        assert_eq!(counter.count(), 0);

        counter.count_field("test");
        assert_eq!(counter.count(), 1);

        let some_option: Option<i32> = Some(42);
        let none_option: Option<i32> = None;

        counter.count_optional_field(&some_option);
        assert_eq!(counter.count(), 2);

        counter.count_optional_field(&none_option);
        assert_eq!(counter.count(), 2); // Should not increment for None
    }

    #[test]
    fn test_write_object_conditional() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let optional_field: Option<i32> = None;
        let other_optional: Option<String> = Some("test".to_string());
        let required_field = "required_value";

        context
            .write_object_conditional(
                |counter| {
                    // Count fields that will be written
                    counter.count_field(required_field);
                    counter.count_optional_field(&optional_field);
                    counter.count_optional_field(&other_optional);
                },
                |ctx| {
                    // Write required field
                    ctx.write_utf8_str("required")?;
                    required_field.serialize(ctx)?;

                    // Write optional fields only if they have values
                    if optional_field.serialize_optional(ctx)? {
                        ctx.write_utf8_str("optional")?;
                    }

                    if other_optional.serialize_optional(ctx)? {
                        ctx.write_utf8_str("test")?;
                    }

                    Ok(())
                },
            )
            .unwrap();

        let result = context.finalize_output_and_return().unwrap();

        // Verify the result contains exactly the expected fields
        assert!(result.is_object());
        let obj = result.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get("required").unwrap(), "required_value");
        assert_eq!(obj.get("test").unwrap(), "test");
        assert!(!obj.contains_key("optional")); // Verify null field was omitted
    }

    #[test]
    fn test_write_object_with_conditional_fields_ergonomic() {
        let mut context = Context::new_with_input(serde_json::json!({}));
        let optional_field: Option<i32> = None;
        let other_optional: Option<String> = Some("test".to_string());
        let required_field = "required_value";

        context
            .write_object_with_conditional_fields(|writer| {
                writer.field("required", required_field)?;
                writer.optional_field("optional", &optional_field)?;
                writer.optional_field("test", &other_optional)?;
                Ok(())
            })
            .unwrap();

        let result = context.finalize_output_and_return().unwrap();

        // Verify the result contains exactly the expected fields
        assert!(result.is_object());
        let obj = result.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get("required").unwrap(), "required_value");
        assert_eq!(obj.get("test").unwrap(), "test");
        assert!(!obj.contains_key("optional")); // Verify null field was omitted
    }
}
