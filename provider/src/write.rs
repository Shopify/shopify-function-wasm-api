use core::ptr::NonNull;
use rmp::encode::{self, ByteBuf};
use std::ffi::c_void;
use std::io::Write;

#[derive(Default)]
struct WriteContext {
    bytes: ByteBuf,
}

type WriteContextPtr = *mut c_void;

fn write_context_from_raw(context: WriteContextPtr) -> NonNull<WriteContext> {
    unsafe { NonNull::new_unchecked(context as _) }
}

#[no_mangle]
#[export_name = "_shopify_function_output_new"]
extern "C" fn shopify_function_output_new() -> WriteContextPtr {
    Box::into_raw(Box::new(WriteContext::default())) as _
}

#[no_mangle]
#[export_name = "_shopify_function_output_new_bool"]
extern "C" fn shopify_function_output_new_bool(context: WriteContextPtr, bool: u32) -> i32 {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
    encode::write_bool(bytes, bool != 0).unwrap();
    0
}

#[no_mangle]
#[export_name = "_shopify_function_output_finalize"]
extern "C" fn shopify_function_output_finalize(context: WriteContextPtr) -> i32 {
    let mut context = write_context_from_raw(context);
    let bytes = unsafe { &mut context.as_mut().bytes };
    let mut stdout = std::io::stdout();
    // Temporary use of stdout to copy from linear memory to the host.
    // Preliminary benchmarking doesn't seem to suggest that this operation
    // represents a considerable overhead.
    stdout
        .write_all(bytes.as_slice())
        .expect("write to succeed");
    stdout.flush().expect("flush to succeed");
    unsafe { drop(Box::from_raw(context.as_ptr())) };
    0
}
