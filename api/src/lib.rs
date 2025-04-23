use shopify_function_wasm_api_core::{read::Val, write::WriteResult, ContextPtr};

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
        ptr: *const u8,
        len: usize,
    ) -> Val;
    fn shopify_function_input_get_interned_obj_prop(
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

#[cfg(target_family = "wasm")]
mod wasm;
#[cfg(target_family = "wasm")]
pub use wasm::{Context, Value};

#[cfg(not(target_family = "wasm"))]
mod local;
#[cfg(not(target_family = "wasm"))]
pub use local::{Context, Value};
