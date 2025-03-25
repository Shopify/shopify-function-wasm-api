use once_cell::sync::OnceCell;
use shopify_function_wasm_api_core::read::{ErrorCode, NanBox, ValueRef as NanBoxValueRef};
use std::io::Read;

mod msgpack_input;

use msgpack_input::MsgpackInput;

static BYTES: OnceCell<Vec<u8>> = OnceCell::new();
static INPUT: OnceCell<MsgpackInput<&'static [u8]>> = OnceCell::new();

fn bytes() -> &'static [u8] {
    BYTES.get_or_init(|| {
        let mut bytes: Vec<u8> = vec![];
        let mut stdin = std::io::stdin();
        // Temporary use of stdin, to copy data into the Wasm linear memory.
        // Initial benchmarking doesn't seem to suggest that this represents
        // a source of performance overhead.
        stdin.read_to_end(&mut bytes).unwrap();

        bytes
    })
}

fn input() -> &'static MsgpackInput<&'static [u8]> {
    INPUT.get_or_init(|| MsgpackInput::new(bytes()))
}

#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get() -> u64 {
    input().encode_value(0).to_bits()
}

#[export_name = "_shopify_function_input_get_obj_prop"]
extern "C" fn shopify_function_input_get_obj_prop(scope: u64, ptr: usize, len: usize) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
            let query = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
            input().get_object_property(obj_ptr, query).to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
        Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_at_index"]
extern "C" fn shopify_function_input_get_at_index(scope: u64, index: u32) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Array { ptr, len: _ }) => {
            input().get_at_index(ptr, index as usize).to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnArray).to_bits(),
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_val_len"]
extern "C" fn shopify_function_input_get_val_len(scope: u64) -> usize {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::String { ptr, .. } | NanBoxValueRef::Array { ptr, .. }) => {
            input().get_value_length(ptr)
        }
        _ => 0,
    }
}

#[export_name = "_shopify_function_input_get_utf8_str_addr"]
extern "C" fn shopify_function_input_get_utf8_str_addr(ptr: usize) -> usize {
    input().get_utf8_str_addr(ptr)
}
