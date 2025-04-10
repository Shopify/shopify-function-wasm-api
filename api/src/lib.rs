use shopify_function_wasm_api_core::{
    read::{NanBox, Val, ValueRef},
    write::WriteResult,
    ContextPtr,
};
use std::ptr::NonNull;

pub mod write;

#[link(wasm_import_module = "shopify_function_v0.0.1")]
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
        interned_string_id: shopify_function_wasm_api_core::InternedStringId,
    ) -> Val;
    fn shopify_function_input_get_at_index(context: ContextPtr, scope: Val, index: usize) -> Val;

    // Write API.
    fn shopify_function_output_new_bool(context: ContextPtr, bool: u32) -> WriteResult;
    fn shopify_function_output_new_null(context: ContextPtr) -> WriteResult;
    fn shopify_function_output_finalize(context: ContextPtr) -> WriteResult;
    fn shopify_function_output_new_i32(context: ContextPtr, int: i32) -> WriteResult;
    fn shopify_function_output_new_f64(context: ContextPtr, float: f64) -> WriteResult;
    fn shopify_function_output_new_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> WriteResult;
    fn shopify_function_output_new_interned_utf8_str(
        context: ContextPtr,
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> WriteResult;
    fn shopify_function_output_new_object(context: ContextPtr, len: usize) -> WriteResult;
    fn shopify_function_output_finish_object(context: ContextPtr) -> WriteResult;
    fn shopify_function_output_new_array(context: ContextPtr, len: usize) -> WriteResult;
    fn shopify_function_output_finish_array(context: ContextPtr) -> WriteResult;

    // Other.
    fn shopify_function_intern_utf8_str(context: ContextPtr, ptr: *const u8, len: usize) -> usize;
}

#[derive(Clone, Copy)]
pub struct InternedStringId(shopify_function_wasm_api_core::InternedStringId);

impl InternedStringId {
    pub fn from_usize(id: usize) -> Self {
        Self(id)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

pub struct Value {
    context: NonNull<ContextPtr>,
    nan_box: NanBox,
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Bool(b)) => Some(b),
            _ => None,
        }
    }

    pub fn as_null(&self) -> Option<()> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Null) => Some(()),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Number(n)) => Some(n),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::String { ptr, len }) => {
                let len = if len as u64 == NanBox::MAX_VALUE_LENGTH {
                    unsafe {
                        shopify_function_input_get_val_len(
                            self.context.as_ptr() as _,
                            self.nan_box.to_bits(),
                        ) as usize
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

    pub fn is_obj(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Object { .. }))
    }

    pub fn get_obj_prop(&self, interned_string_id: InternedStringId) -> Self {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Object { .. }) => {
                let scope = unsafe {
                    shopify_function_input_get_obj_prop(
                        self.context.as_ptr() as _,
                        self.nan_box.to_bits(),
                        interned_string_id.as_usize(),
                    )
                };
                Self {
                    context: self.context,
                    nan_box: NanBox::from_bits(scope),
                }
            }
            _ => Self {
                context: self.context,
                nan_box: NanBox::from_bits(self.nan_box.to_bits()),
            },
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Array { .. }))
    }

    pub fn array_len(&self) -> Option<usize> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Array { len, .. }) => {
                let len = if len as u64 == NanBox::MAX_VALUE_LENGTH {
                    unsafe {
                        shopify_function_input_get_val_len(
                            self.context.as_ptr() as _,
                            self.nan_box.to_bits(),
                        ) as usize
                    }
                } else {
                    len
                };
                Some(len)
            }
            _ => None,
        }
    }

    pub fn get_at_index(&self, index: usize) -> Value {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Array { .. }) => {
                let scope = unsafe {
                    shopify_function_input_get_at_index(
                        self.context.as_ptr() as _,
                        self.nan_box.to_bits(),
                        index,
                    )
                };
                Self {
                    context: self.context,
                    nan_box: NanBox::from_bits(scope),
                }
            }
            _ => Self {
                context: self.context,
                nan_box: NanBox::from_bits(self.nan_box.to_bits()),
            },
        }
    }
}

pub struct Context(ContextPtr);

#[derive(Debug)]
pub enum ContextError {
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
    pub fn new() -> Self {
        Self(unsafe { shopify_function_context_new() })
    }

    pub fn input_get(&self) -> Result<Value, ContextError> {
        let val = unsafe { shopify_function_input_get(self.0) };
        NonNull::new(self.0 as _)
            .ok_or(ContextError::NullPointer)
            .map(|context| Value {
                context,
                nan_box: NanBox::from_bits(val),
            })
    }

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
