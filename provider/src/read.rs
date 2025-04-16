use crate::Context;
use shopify_function_wasm_api_core::ContextPtr;
use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, ValueRef as NanBoxValueRef},
    InternedStringId,
};

mod msgpack_input;

pub(crate) use msgpack_input::MsgpackInput;

#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get(context: ContextPtr) -> u64 {
    match Context::ref_from_raw(context) {
        Ok(context) => {
            let input = &context.msgpack_input;
            input.encode_value(0).to_bits()
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
                    let input = &context.msgpack_input;
                    input.get_object_property(obj_ptr, query).to_bits()
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
                    let input = &context.msgpack_input;
                    input.get_object_property(obj_ptr, query).to_bits()
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
                    let input = &context.msgpack_input;
                    input.get_at_index(ptr, index as usize).to_bits()
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
        Ok(context) => {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::String { ptr, .. } | NanBoxValueRef::Array { ptr, .. }) => {
                    let input = &context.msgpack_input;
                    input.get_value_length(ptr)
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
            let input = &context.msgpack_input;
            input.get_utf8_str_addr(ptr)
        }
        Err(_) => 0,
    }
}
