use core::ptr::NonNull;
use rmp::encode::{self, ByteBuf};
use shopify_function_wasm_api_core::write::{WriteContext as WriteContextPtr, WriteResult};
use std::io::Write;

mod state;

use state::State;

#[derive(Default)]
struct WriteContext {
    bytes: ByteBuf,
    state: State,
}

fn write_context_from_raw(context: WriteContextPtr) -> NonNull<WriteContext> {
    unsafe { NonNull::new_unchecked(context as _) }
}

#[export_name = "_shopify_function_output_new"]
extern "C" fn shopify_function_output_new() -> WriteContextPtr {
    Box::into_raw(Box::new(WriteContext::default())) as _
}

#[export_name = "_shopify_function_output_new_bool"]
extern "C" fn shopify_function_output_new_bool(context: WriteContextPtr, bool: u32) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
    let result = state.write_non_string_scalar();
    if result != WriteResult::Ok {
        return result;
    }
    encode::write_bool(bytes, bool != 0).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_null"]
extern "C" fn shopify_function_output_new_null(context: WriteContextPtr) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
    let result = state.write_non_string_scalar();
    if result != WriteResult::Ok {
        return result;
    }
    encode::write_nil(bytes).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_i32"]
extern "C" fn shopify_function_output_new_i32(context: WriteContextPtr, int: i32) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
    let result = state.write_non_string_scalar();
    if result != WriteResult::Ok {
        return result;
    }
    encode::write_sint(bytes, int as i64).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_f64"]
extern "C" fn shopify_function_output_new_f64(context: WriteContextPtr, float: f64) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
    let result = state.write_non_string_scalar();
    if result != WriteResult::Ok {
        return result;
    }
    encode::write_f64(bytes, float).unwrap(); // infallible unwrap
    WriteResult::Ok
}

/// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
#[export_name = "_shopify_function_output_new_utf8_str"]
extern "C" fn shopify_function_output_new_utf8_str(context: WriteContextPtr, len: usize) -> u64 {
    let (result, ptr): (WriteResult, *const u8) = {
        let mut context = write_context_from_raw(context);
        let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
        let result = state.write_string();
        if result != WriteResult::Ok {
            (result, std::ptr::null())
        } else {
            encode::write_str_len(bytes, len as u32).unwrap(); // infallible unwrap
            let original_len = bytes.as_slice().len();
            // fill in the new bytes with zeros; the trampoline will copy the string to overwrite them
            bytes.as_mut_vec().resize(original_len + len, 0);
            (WriteResult::Ok, bytes.as_slice()[original_len..].as_ptr())
        }
    };
    ((result as u64) << 32) | ptr as u64
}

#[export_name = "_shopify_function_output_new_object"]
fn shopify_function_output_new_object(context: WriteContextPtr, len: usize) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state } = unsafe { &mut context.as_mut() };
    let result = state.start_object(len);
    if result != WriteResult::Ok {
        return result;
    }
    encode::write_map_len(bytes, len as u32).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_finish_object"]
extern "C" fn shopify_function_output_finish_object(context: WriteContextPtr) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { state, .. } = unsafe { &mut context.as_mut() };
    state.finish_object()
}

#[export_name = "_shopify_function_output_finalize"]
extern "C" fn shopify_function_output_finalize(context: WriteContextPtr) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let WriteContext { bytes, state, .. } = unsafe { &mut context.as_mut() };
    if *state != State::End {
        return WriteResult::ValueNotFinished;
    }
    let mut stdout = std::io::stdout();
    // Temporary use of stdout to copy from linear memory to the host.
    // Preliminary benchmarking doesn't seem to suggest that this operation
    // represents a considerable overhead.
    if stdout.write_all(bytes.as_slice()).is_err() {
        return WriteResult::IoError;
    }
    if stdout.flush().is_err() {
        return WriteResult::IoError;
    }
    let _ = unsafe { Box::from_raw(context.as_ptr()) }; // drop the context
    WriteResult::Ok
}
