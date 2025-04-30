use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, Val, ValueRef},
    write::WriteResult,
    ContextPtr,
};
use std::ptr::NonNull;

pub mod read;
pub mod write;

pub use read::Deserialize;
pub use write::Serialize;

#[cfg(target_family = "wasm")]
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

#[cfg(not(target_family = "wasm"))]
mod provider_fallback {
    use super::{ContextPtr, Val, WriteResult};

    // Read API.
    pub(crate) unsafe fn shopify_function_input_get(context: ContextPtr) -> Val {
        shopify_function_wasm_api_provider::read::shopify_function_input_get(context)
    }
    pub(crate) unsafe fn shopify_function_input_get_val_len(
        context: ContextPtr,
        scope: Val,
    ) -> usize {
        shopify_function_wasm_api_provider::read::shopify_function_input_get_val_len(context, scope)
    }
    pub(crate) unsafe fn shopify_function_input_read_utf8_str(
        context: ContextPtr,
        src: usize,
        out: *mut u8,
        len: usize,
    ) {
        let src =
            shopify_function_wasm_api_provider::read::shopify_function_input_get_utf8_str_addr(
                context, src,
            );
        std::ptr::copy(src as _, out, len);
    }
    pub(crate) unsafe fn shopify_function_input_get_obj_prop(
        context: ContextPtr,
        scope: Val,
        ptr: *const u8,
        len: usize,
    ) -> Val {
        shopify_function_wasm_api_provider::read::shopify_function_input_get_obj_prop(
            context, scope, ptr as _, len,
        )
    }
    pub(crate) unsafe fn shopify_function_input_get_interned_obj_prop(
        context: ContextPtr,
        scope: Val,
        interned_string_id: shopify_function_wasm_api_core::InternedStringId,
    ) -> Val {
        shopify_function_wasm_api_provider::read::shopify_function_input_get_interned_obj_prop(
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
        shopify_function_wasm_api_provider::read::shopify_function_input_get_at_index(
            context, scope, index,
        )
    }
    pub(crate) unsafe fn shopify_function_input_get_obj_key_at_index(
        context: ContextPtr,
        scope: Val,
        index: usize,
    ) -> Val {
        shopify_function_wasm_api_provider::read::shopify_function_input_get_obj_key_at_index(
            context, scope, index,
        )
    }

    // Write API.
    pub(crate) unsafe fn shopify_function_output_new_bool(
        context: ContextPtr,
        bool: u32,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_bool(context, bool)
    }
    pub(crate) unsafe fn shopify_function_output_new_null(context: ContextPtr) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_null(context)
    }
    pub(crate) unsafe fn shopify_function_output_finalize(context: ContextPtr) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_finalize(context)
    }
    pub(crate) unsafe fn shopify_function_output_new_i32(
        context: ContextPtr,
        int: i32,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_i32(context, int)
    }
    pub(crate) unsafe fn shopify_function_output_new_f64(
        context: ContextPtr,
        float: f64,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_f64(context, float)
    }
    pub(crate) unsafe fn shopify_function_output_new_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> WriteResult {
        let result =
            shopify_function_wasm_api_provider::write::shopify_function_output_new_utf8_str(
                context, len,
            );
        let write_result =
            WriteResult::from_repr((result >> usize::BITS) as usize).expect("Invalid write result");
        let dst = result as usize;
        if write_result == WriteResult::Ok {
            std::ptr::copy(ptr as _, dst as _, len);
        }
        write_result
    }
    pub(crate) unsafe fn shopify_function_output_new_interned_utf8_str(
        context: ContextPtr,
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_interned_utf8_str(
            context, id,
        )
    }
    pub(crate) unsafe fn shopify_function_output_new_object(
        context: ContextPtr,
        len: usize,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_object(context, len)
    }
    pub(crate) unsafe fn shopify_function_output_finish_object(context: ContextPtr) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_finish_object(context)
    }
    pub(crate) unsafe fn shopify_function_output_new_array(
        context: ContextPtr,
        len: usize,
    ) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_new_array(context, len)
    }
    pub(crate) unsafe fn shopify_function_output_finish_array(context: ContextPtr) -> WriteResult {
        shopify_function_wasm_api_provider::write::shopify_function_output_finish_array(context)
    }

    // Other.
    pub(crate) unsafe fn shopify_function_intern_utf8_str(
        context: ContextPtr,
        ptr: *const u8,
        len: usize,
    ) -> usize {
        let result =
            shopify_function_wasm_api_provider::shopify_function_intern_utf8_str(context, len);
        let id = (result >> usize::BITS) as usize;
        let dst = result as usize;
        std::ptr::copy(ptr as _, dst as _, len);
        id
    }
}
#[cfg(not(target_family = "wasm"))]
use provider_fallback::*;

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

#[derive(Copy, Clone)]
pub struct Value {
    context: NonNull<ContextPtr>,
    nan_box: NanBox,
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
                Some(len)
            }
            _ => None,
        }
    }

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
                Some(len)
            }
            _ => None,
        }
    }

    pub fn get_at_index(&self, index: usize) -> Self {
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
                let value = Self {
                    context: self.context,
                    nan_box: NanBox::from_bits(scope),
                };
                value.as_string()
            }
            _ => None,
        }
    }

    pub fn as_error(&self) -> Option<ErrorCode> {
        match self.nan_box.try_decode() {
            Ok(ValueRef::Error(e)) => Some(e),
            _ => None,
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
    #[cfg(target_family = "wasm")]
    pub fn new() -> Self {
        Self(unsafe { shopify_function_context_new() })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn new() -> Self {
        panic!("Cannot run in non-WASM environment; use `new_with_input` instead")
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn new_with_input(input: serde_json::Value) -> Self {
        let bytes = rmp_serde::to_vec(&input).unwrap();
        Self(
            shopify_function_wasm_api_provider::shopify_function_context_new_from_msgpack_bytes(
                bytes,
            ),
        )
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
