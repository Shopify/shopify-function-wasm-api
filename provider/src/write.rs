use core::ptr::NonNull;
use rmp::encode::{self, ByteBuf};
use shopify_function_wasm_api_core::write::{WriteContext as WriteContextPtr, WriteResult};
use std::io::Write;

#[derive(Default)]
struct WriteContext {
    bytes: ByteBuf,
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
    let bytes = unsafe { &mut context.as_mut().bytes };
    encode::write_bool(bytes, bool != 0).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_null"]
extern "C" fn shopify_function_output_new_null(context: WriteContextPtr) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
    encode::write_nil(bytes).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_i32"]
extern "C" fn shopify_function_output_new_i32(context: WriteContextPtr, int: i32) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
    encode::write_sint(bytes, int as i64).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_new_f64"]
extern "C" fn shopify_function_output_new_f64(context: WriteContextPtr, float: f64) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
    encode::write_f64(bytes, float).unwrap(); // infallible unwrap
    WriteResult::Ok
}

#[export_name = "_shopify_function_output_finalize"]
extern "C" fn shopify_function_output_finalize(context: WriteContextPtr) -> WriteResult {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
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
