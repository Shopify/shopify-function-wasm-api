use std::ptr::NonNull;

use shopify_function_wasm_api_core::{
    read::{NanBox, ValueRef},
    ContextPtr,
};

use crate::{
    shopify_function_context_new, shopify_function_input_get, shopify_function_input_get_at_index,
    shopify_function_input_get_interned_obj_prop, shopify_function_input_get_obj_prop,
    shopify_function_input_get_val_len, shopify_function_input_read_utf8_str,
    shopify_function_intern_utf8_str, ContextError, InternedStringId,
};

pub struct Value {
    pub(crate) context: NonNull<ContextPtr>,
    pub(crate) nan_box: NanBox,
}

impl Value {
    pub fn intern_utf8_str(&self, s: &str) -> InternedStringId {
        let len = s.len();
        let ptr = s.as_ptr();
        let id = unsafe { shopify_function_intern_utf8_str(self.context.as_ptr() as _, ptr, len) };
        InternedStringId(id)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Bool(b)) => Some(b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.nan_box.try_decode(), Ok(ValueRef::Null))
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

    pub fn get_obj_prop(&self, prop: &str) -> Self {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Object { .. }) => {
                let scope = unsafe {
                    shopify_function_input_get_obj_prop(
                        self.context.as_ptr() as _,
                        self.nan_box.to_bits(),
                        prop.as_ptr(),
                        prop.len(),
                    )
                };
                Self {
                    context: self.context,
                    nan_box: NanBox::from_bits(scope),
                }
            }
            _ => Self {
                context: self.context,
                nan_box: NanBox::null(),
            },
        }
    }

    pub fn get_interned_obj_prop(&self, interned_string_id: InternedStringId) -> Self {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Object { .. }) => {
                let scope = unsafe {
                    shopify_function_input_get_interned_obj_prop(
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
                nan_box: NanBox::null(),
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

pub struct Context(pub(crate) ContextPtr);

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
