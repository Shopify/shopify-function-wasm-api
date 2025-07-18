//! # Shopify Function Wasm API
//!
//! This crate provides a high-level API for interfacing with the Shopify Function Wasm API.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use shopify_function_wasm_api::{Context, Serialize, Deserialize, Value};
//! use std::error::Error;
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut context = Context::new();
//!     let input = context.input_get()?;
//!     let value: i32 = Deserialize::deserialize(&input)?;
//!     value.serialize(&mut context)?;
//!     context.finalize_output()?;
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]

use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, Val, ValueRef},
    ContextPtr,
};
use std::ptr::NonNull;
#[cfg(target_pointer_width = "32")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_pointer_width = "64")]
use std::sync::Mutex;

pub mod read;
pub mod write;

pub use read::Deserialize;
pub use write::Serialize;

#[cfg(target_family = "wasm")]
#[link(wasm_import_module = "shopify_function_v1")]
extern "C" {
    // Common API.
    fn shopify_function_context_new() -> ContextPtr;

    // Read API.
    fn shopify_function_input_get(context: ContextPtr) -> Val;
    fn shopify_function_input_get_val_len(context: ContextPtr, scope: Val) -> usize;
    fn shopify_function_input_read_utf8_str(
        context: ContextPtr,
        src: usize,
        out: *mut u8,
        len: usize,
    );
    fn shopify_function_input_get_obj_prop(
        context: ContextPtr,
        scope: Val,
        ptr: *const u8,
        len: usize,
    ) -> Val;
    fn shopify_function_input_get_interned_obj_prop(
        context: ContextPtr,
        scope: Val,
        interned_string_id: shopify_function_wasm_api_core::InternedStringId,
    ) -> Val;
    fn shopify_function_input_get_at_index(context: ContextPtr, scope: Val, index: usize) -> Val;
    fn shopify_function_input_get_obj_key_at_index(
        context: ContextPtr,
        scope: Val,
        index: usize,
    ) -> Val;

    // Write API.
    fn shopify_function_output_new_bool(context: ContextPtr, bool: u32) -> usize;
    fn shopify_function_output_new_null(context: ContextPtr) -> usize;
    fn shopify_function_output_finalize(context: ContextPtr) -> usize;
    fn shopify_function_output_new_i32(context: ContextPtr, int: i32) -> usize;
    fn shopify_function_output_new_f64(context: ContextPtr, float: f64) -> usize;
    fn shopify_function_output_new_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> usize;
    fn shopify_function_output_new_interned_utf8_str(
        context: ContextPtr,
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> usize;
    fn shopify_function_output_new_object(context: ContextPtr, len: usize) -> usize;
    fn shopify_function_output_finish_object(context: ContextPtr) -> usize;
    fn shopify_function_output_new_array(context: ContextPtr, len: usize) -> usize;
    fn shopify_function_output_finish_array(context: ContextPtr) -> usize;

    // Other.
    fn shopify_function_intern_utf8_str(context: ContextPtr, ptr: *const u8, len: usize) -> usize;
}

#[cfg(not(target_family = "wasm"))]
mod provider_fallback {
    use super::{ContextPtr, Val};
    use shopify_function_wasm_api_core::write::WriteResult;

    // Read API.
    pub(crate) unsafe fn shopify_function_input_get(context: ContextPtr) -> Val {
        shopify_function_provider::read::shopify_function_input_get(context)
    }
    pub(crate) unsafe fn shopify_function_input_get_val_len(
        context: ContextPtr,
        scope: Val,
    ) -> usize {
        shopify_function_provider::read::shopify_function_input_get_val_len(context, scope)
    }
    pub(crate) unsafe fn shopify_function_input_read_utf8_str(
        context: ContextPtr,
        src: usize,
        out: *mut u8,
        len: usize,
    ) {
        let src =
            shopify_function_provider::read::shopify_function_input_get_utf8_str_addr(context, src);
        std::ptr::copy(src as _, out, len);
    }
    pub(crate) unsafe fn shopify_function_input_get_obj_prop(
        context: ContextPtr,
        scope: Val,
        ptr: *const u8,
        len: usize,
    ) -> Val {
        shopify_function_provider::read::shopify_function_input_get_obj_prop(
            context, scope, ptr as _, len,
        )
    }
    pub(crate) unsafe fn shopify_function_input_get_interned_obj_prop(
        context: ContextPtr,
        scope: Val,
        interned_string_id: shopify_function_wasm_api_core::InternedStringId,
    ) -> Val {
        shopify_function_provider::read::shopify_function_input_get_interned_obj_prop(
            context,
            scope,
            interned_string_id,
        )
    }
    pub(crate) unsafe fn shopify_function_input_get_at_index(
        context: ContextPtr,
        scope: Val,
        index: usize,
    ) -> Val {
        shopify_function_provider::read::shopify_function_input_get_at_index(context, scope, index)
    }
    pub(crate) unsafe fn shopify_function_input_get_obj_key_at_index(
        context: ContextPtr,
        scope: Val,
        index: usize,
    ) -> Val {
        shopify_function_provider::read::shopify_function_input_get_obj_key_at_index(
            context, scope, index,
        )
    }

    // Write API.
    pub(crate) unsafe fn shopify_function_output_new_bool(context: ContextPtr, bool: u32) -> usize {
        shopify_function_provider::write::shopify_function_output_new_bool(context, bool) as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_null(context: ContextPtr) -> usize {
        shopify_function_provider::write::shopify_function_output_new_null(context) as usize
    }
    pub(crate) unsafe fn shopify_function_output_finalize(context: ContextPtr) -> usize {
        shopify_function_provider::write::shopify_function_output_finalize(context) as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_i32(context: ContextPtr, int: i32) -> usize {
        shopify_function_provider::write::shopify_function_output_new_i32(context, int) as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_f64(context: ContextPtr, float: f64) -> usize {
        shopify_function_provider::write::shopify_function_output_new_f64(context, float) as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> usize {
        let result =
            shopify_function_provider::write::shopify_function_output_new_utf8_str(context, len);
        let write_result = (result >> usize::BITS) as usize;
        let dst = result as usize;
        if write_result == WriteResult::Ok as usize {
            std::ptr::copy(ptr as _, dst as _, len);
        }
        write_result
    }
    pub(crate) unsafe fn shopify_function_output_new_interned_utf8_str(
        context: ContextPtr,
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> usize {
        shopify_function_provider::write::shopify_function_output_new_interned_utf8_str(context, id)
            as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_object(
        context: ContextPtr,
        len: usize,
    ) -> usize {
        shopify_function_provider::write::shopify_function_output_new_object(context, len) as usize
    }
    pub(crate) unsafe fn shopify_function_output_finish_object(context: ContextPtr) -> usize {
        shopify_function_provider::write::shopify_function_output_finish_object(context) as usize
    }
    pub(crate) unsafe fn shopify_function_output_new_array(
        context: ContextPtr,
        len: usize,
    ) -> usize {
        shopify_function_provider::write::shopify_function_output_new_array(context, len) as usize
    }
    pub(crate) unsafe fn shopify_function_output_finish_array(context: ContextPtr) -> usize {
        shopify_function_provider::write::shopify_function_output_finish_array(context) as usize
    }

    // Other.
    pub(crate) unsafe fn shopify_function_intern_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> usize {
        let result = shopify_function_provider::shopify_function_intern_utf8_str(context, len);
        let id = (result >> usize::BITS) as usize;
        let dst = result as usize;
        std::ptr::copy(ptr as _, dst as _, len);
        id
    }
}
#[cfg(not(target_family = "wasm"))]
use provider_fallback::*;

/// An identifier for an interned UTF-8 string.
///
/// This is returned by [`Context::intern_utf8_str`], and can be used for both reading and writing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InternedStringId(shopify_function_wasm_api_core::InternedStringId);

impl InternedStringId {
    fn as_usize(&self) -> usize {
        self.0
    }
}

/// A mechanism for caching interned string IDs.
pub struct CachedInternedStringId {
    value: &'static str,
    #[cfg(target_pointer_width = "32")]
    interned_string_id_and_context: AtomicU64,
    #[cfg(target_pointer_width = "64")]
    interned_string_id_and_context: Mutex<(usize, usize)>,
}

impl CachedInternedStringId {
    /// Create a new cached interned string ID.
    pub const fn new(value: &'static str) -> Self {
        Self {
            value,
            #[cfg(target_pointer_width = "32")]
            interned_string_id_and_context: AtomicU64::new(u64::MAX),
            #[cfg(target_pointer_width = "64")]
            interned_string_id_and_context: Mutex::new((usize::MAX, usize::MAX)),
        }
    }

    /// Load the interned string ID from a context.
    pub fn load_from_context(&self, context: &Context) -> InternedStringId {
        self.load_from_context_ptr(context.0)
    }

    /// Load the interned string ID from a value.
    pub fn load_from_value(&self, value: &Value) -> InternedStringId {
        self.load_from_context_ptr(value.context.as_ptr() as _)
    }

    #[cfg(target_pointer_width = "32")]
    fn load_from_context_ptr(&self, context: ContextPtr) -> InternedStringId {
        let interned_string_id_and_context =
            self.interned_string_id_and_context.load(Ordering::Relaxed);
        if interned_string_id_and_context & (u32::MAX as u64) == context as u64 {
            InternedStringId((interned_string_id_and_context >> 32) as usize)
        } else {
            let id = unsafe {
                shopify_function_intern_utf8_str(context, self.value.as_ptr(), self.value.len())
            };
            let interned_string_id_and_context = ((id as u64) << 32) | (context as u64);
            self.interned_string_id_and_context
                .store(interned_string_id_and_context, Ordering::Relaxed);
            InternedStringId(id as usize)
        }
    }

    #[cfg(target_pointer_width = "64")]
    fn load_from_context_ptr(&self, context: ContextPtr) -> InternedStringId {
        let mut interned_string_id_and_context =
            self.interned_string_id_and_context.lock().unwrap();
        if interned_string_id_and_context.1 != context as usize {
            let id = unsafe {
                shopify_function_intern_utf8_str(context, self.value.as_ptr(), self.value.len())
            };
            interned_string_id_and_context.0 = id;
            interned_string_id_and_context.1 = context as usize;
        }
        InternedStringId(interned_string_id_and_context.0)
    }
}

/// A value read from the input.
///
/// This can be any of the following types:
/// - boolean
/// - number
/// - string
/// - null
/// - object
/// - array
/// - error
#[derive(Copy, Clone)]
pub struct Value {
    context: NonNull<ContextPtr>,
    nan_box: NanBox,
}

impl Value {
    fn new_child(&self, nan_box: NanBox) -> Self {
        Self {
            context: self.context,
            nan_box,
        }
    }

    /// Intern a string. This is just a convenience method equivalent to calling [`Context::intern_utf8_str`], if you don't have a [`Context`] easily accessible.
    pub fn intern_utf8_str(&self, s: &str) -> InternedStringId {
        let len = s.len();
        let ptr = s.as_ptr();
        let id = unsafe { shopify_function_intern_utf8_str(self.context.as_ptr() as _, ptr, len) };
        InternedStringId(id)
    }

    /// Get the value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Bool(b)) => Some(b),
            _ => None,
        }
    }

    /// Check if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Null))
    }

    /// Get the value as a number, if it is one. Note that this will apply to both integers and floats.
    pub fn as_number(&self) -> Option<f64> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Number(n)) => Some(n),
            _ => None,
        }
    }

    /// Get the value as a string, if it is one.
    pub fn as_string(&self) -> Option<String> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::String { ptr, len }) => {
                let len = if len == NanBox::MAX_VALUE_LENGTH {
                    unsafe {
                        shopify_function_input_get_val_len(
                            self.context.as_ptr() as _,
                            self.nan_box.to_bits(),
                        )
                    }
                } else {
                    len
                };
                let mut buf = vec![0; len];
                unsafe {
                    shopify_function_input_read_utf8_str(
                        self.context.as_ptr() as _,
                        ptr as _,
                        buf.as_mut_ptr(),
                        len,
                    )
                };
                Some(unsafe { String::from_utf8_unchecked(buf) })
            }
            _ => None,
        }
    }

    /// Check if the value is an object.
    pub fn is_obj(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Object { .. }))
    }

    /// Get a property from the object.
    pub fn get_obj_prop(&self, prop: &str) -> Self {
        let scope = unsafe {
            shopify_function_input_get_obj_prop(
                self.context.as_ptr() as _,
                self.nan_box.to_bits(),
                prop.as_ptr(),
                prop.len(),
            )
        };
        self.new_child(NanBox::from_bits(scope))
    }

    /// Get a property from the object by its interned string ID.
    pub fn get_interned_obj_prop(&self, interned_string_id: InternedStringId) -> Self {
        let scope = unsafe {
            shopify_function_input_get_interned_obj_prop(
                self.context.as_ptr() as _,
                self.nan_box.to_bits(),
                interned_string_id.as_usize(),
            )
        };
        self.new_child(NanBox::from_bits(scope))
    }

    /// Check if the value is an array.
    pub fn is_array(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Array { .. }))
    }

    /// Get the length of the array, if it is one.
    pub fn array_len(&self) -> Option<usize> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Array { len, .. }) => {
                let len = if len == NanBox::MAX_VALUE_LENGTH {
                    unsafe {
                        shopify_function_input_get_val_len(
                            self.context.as_ptr() as _,
                            self.nan_box.to_bits(),
                        )
                    }
                } else {
                    len
                };
                if len == usize::MAX {
                    None
                } else {
                    Some(len)
                }
            }
            _ => None,
        }
    }

    /// Get the length of the object, if it is one.
    pub fn obj_len(&self) -> Option<usize> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Object { len, .. }) => {
                let len = if len == NanBox::MAX_VALUE_LENGTH {
                    unsafe {
                        shopify_function_input_get_val_len(
                            self.context.as_ptr() as _,
                            self.nan_box.to_bits(),
                        )
                    }
                } else {
                    len
                };
                if len == usize::MAX {
                    None
                } else {
                    Some(len)
                }
            }
            _ => None,
        }
    }

    /// Get an element from the array or object by its index.
    pub fn get_at_index(&self, index: usize) -> Self {
        let scope = unsafe {
            shopify_function_input_get_at_index(
                self.context.as_ptr() as _,
                self.nan_box.to_bits(),
                index,
            )
        };
        self.new_child(NanBox::from_bits(scope))
    }

    /// Get the key of an object by its index.
    pub fn get_obj_key_at_index(&self, index: usize) -> Option<String> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Object { .. }) => {
                let scope = unsafe {
                    shopify_function_input_get_obj_key_at_index(
                        self.context.as_ptr() as _,
                        self.nan_box.to_bits(),
                        index,
                    )
                };
                let value = self.new_child(NanBox::from_bits(scope));
                value.as_string()
            }
            _ => None,
        }
    }

    /// Get the error code, if it is one.
    pub fn as_error(&self) -> Option<ErrorCode> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Error(e)) => Some(e),
            _ => None,
        }
    }
}

/// A context for reading and writing values.
///
/// This is created by calling [`Context::new`], and is used to read values from the input and write values to the output.
pub struct Context(ContextPtr);

/// An error that can occur when creating a [`Context`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ContextError {
    /// The pointer to the context is null.
    NullPointer,
}

impl std::error::Error for ContextError {}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextError::NullPointer => write!(f, "Null pointer encountered"),
        }
    }
}

impl Context {
    /// Create a new context.
    ///
    /// This is only intended to be invoked when compiled to a Wasm target.
    ///
    /// # Panics
    /// This will panic if called from a non-Wasm environment.
    pub fn new() -> Self {
        #[cfg(not(target_family = "wasm"))]
        panic!("Cannot run in non-WASM environment; use `new_with_input` instead");

        #[cfg(target_family = "wasm")]
        Self(unsafe { shopify_function_context_new() })
    }

    /// Create a new context from a JSON value, which will be the top-level value of the input.
    ///
    /// This is only available when compiled to a non-Wasm target, for usage in unit tests.
    #[cfg(not(target_family = "wasm"))]
    pub fn new_with_input(input: serde_json::Value) -> Self {
        let bytes = rmp_serde::to_vec(&input).unwrap();
        Self(shopify_function_provider::shopify_function_context_new_from_msgpack_bytes(bytes))
    }

    /// Get the top-level value of the input.
    pub fn input_get(&self) -> Result<Value, ContextError> {
        let val = unsafe { shopify_function_input_get(self.0) };
        NonNull::new(self.0 as _)
            .ok_or(ContextError::NullPointer)
            .map(|context| Value {
                context,
                nan_box: NanBox::from_bits(val),
            })
    }

    /// Intern a string. This can lead to performance gains if you are using the same string multiple times,
    /// as it saves unnecessary string copies. For example, if you are reading the same property from multiple objects,
    /// or serializing the same key on an object, you can intern the string once and reuse it.
    pub fn intern_utf8_str(&self, s: &str) -> InternedStringId {
        let len = s.len();
        let ptr = s.as_ptr();
        let id = unsafe { shopify_function_intern_utf8_str(self.0 as _, ptr, len) };
        InternedStringId(id)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interned_string_id_cache() {
        let cached_interned_string_id = CachedInternedStringId::new("test");

        let context = Context::new_with_input(serde_json::json!({}));
        let id = cached_interned_string_id.load_from_context(&context);
        let id2 = cached_interned_string_id.load_from_context(&context);
        assert_eq!(id, id2);

        let context2 = Context::new_with_input(serde_json::json!({}));
        // the first interned string ID from a new context is always 0,
        // so we need to mix up the order of interning to make this test work
        context2.intern_utf8_str("test");
        let id3 = cached_interned_string_id.load_from_context(&context2);
        assert_ne!(id, id3);
    }

    #[test]
    fn test_array_len_with_null_ptr() {
        let context = Context::new_with_input(serde_json::json!({}));
        let value = Value {
            context: NonNull::new(context.0 as _).unwrap(),
            nan_box: NanBox::array(0, NanBox::MAX_VALUE_LENGTH),
        };
        let len = value.array_len();
        assert_eq!(len, None);
    }

    #[test]
    fn test_array_len_with_non_length_eligible_nan_box() {
        let context = Context::new_with_input(serde_json::json!({}));
        let value = Value {
            context: NonNull::new(context.0 as _).unwrap(),
            nan_box: NanBox::null(),
        };
        let len = value.array_len();
        assert_eq!(len, None);
    }

    #[test]
    fn test_obj_len_with_null_ptr() {
        let context = Context::new_with_input(serde_json::json!({}));
        let value = Value {
            context: NonNull::new(context.0 as _).unwrap(),
            nan_box: NanBox::obj(0, NanBox::MAX_VALUE_LENGTH),
        };
        let len = value.obj_len();
        assert_eq!(len, None);
    }

    #[test]
    fn test_obj_len_with_non_length_eligible_nan_box() {
        let context = Context::new_with_input(serde_json::json!({}));
        let value = Value {
            context: NonNull::new(context.0 as _).unwrap(),
            nan_box: NanBox::null(),
        };
        let len = value.obj_len();
        assert_eq!(len, None);
    }
}
