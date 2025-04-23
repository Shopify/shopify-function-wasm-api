use crate::Context;
use shopify_function_wasm_api_core::ContextPtr;
use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, ValueRef as NanBoxValueRef},
    InternedStringId,
};

mod lazy_value_ref;

pub(crate) use lazy_value_ref::LazyValueRef;

#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get(context: ContextPtr) -> u64 {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            match context.bump_allocator.alloc_try_with(|| {
                LazyValueRef::new(&context.input_bytes, 0, &context.bump_allocator)
                    .map(|(value, _)| value)
            }) {
                Ok(input_ref) => input_ref.encode().to_bits(),
                Err(e) => NanBox::error(e).to_bits(),
            }
        }
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_obj_prop"]
extern "C" fn shopify_function_input_get_obj_prop(
    context: ContextPtr,
    scope: u64,
    ptr: usize,
    len: usize,
) -> u64 {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
                    let query = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
                    let value = match LazyValueRef::mut_from_raw(obj_ptr as _) {
                        Ok(value) => value,
                        Err(e) => return NanBox::error(e).to_bits(),
                    };
                    match value.get_object_property(
                        query,
                        &context.input_bytes,
                        &context.bump_allocator,
                    ) {
                        Ok(Some(value)) => value.encode().to_bits(),
                        Ok(None) => NanBox::null().to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
                Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
            }
        }
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_interned_obj_prop"]
extern "C" fn shopify_function_input_get_interned_obj_prop(
    context: ContextPtr,
    scope: u64,
    interned_string_id: InternedStringId,
) -> u64 {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
                    let query = context.string_interner.get(interned_string_id);
                    let value = match LazyValueRef::mut_from_raw(obj_ptr as _) {
                        Ok(value) => value,
                        Err(e) => return NanBox::error(e).to_bits(),
                    };
                    match value.get_object_property(
                        query,
                        &context.input_bytes,
                        &context.bump_allocator,
                    ) {
                        Ok(Some(value)) => value.encode().to_bits(),
                        Ok(None) => NanBox::null().to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
                Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
            }
        }
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_at_index"]
extern "C" fn shopify_function_input_get_at_index(
    context: ContextPtr,
    scope: u64,
    index: u32,
) -> u64 {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Array { ptr, len: _ }) => {
                    let value = match LazyValueRef::mut_from_raw(ptr as _) {
                        Ok(value) => value,
                        Err(e) => return NanBox::error(e).to_bits(),
                    };
                    match value.get_at_index(
                        index as usize,
                        &context.input_bytes,
                        &context.bump_allocator,
                    ) {
                        Ok(value) => value.encode().to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnArray).to_bits(),
                Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
            }
        }
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_val_len"]
extern "C" fn shopify_function_input_get_val_len(context: ContextPtr, scope: u64) -> usize {
    match Context::ref_from_raw(context) {
        Ok(_) => {
            // don't actually need the context, but keeping it for consistency and to make it possible to use in the future if needed
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::String { ptr, .. } | NanBoxValueRef::Array { ptr, .. }) => {
                    let Ok(value) = LazyValueRef::mut_from_raw(ptr as _) else {
                        return 0;
                    };
                    value.get_value_length()
                }
                _ => 0,
            }
        }
        Err(_) => 0,
    }
}

#[export_name = "_shopify_function_input_get_utf8_str_addr"]
extern "C" fn shopify_function_input_get_utf8_str_addr(context: ContextPtr, ptr: usize) -> usize {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            let Ok(value) = LazyValueRef::mut_from_raw(ptr as _) else {
                return 0;
            };
            value.get_utf8_str_addr(&context.input_bytes)
        }
        Err(_) => 0,
    }
}
