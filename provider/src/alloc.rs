use std::alloc::{alloc, Layout};

const ZERO_SIZE_ALLOCATION_PTR: *mut u8 = 1 as _;

// Allocation functions
#[export_name = "_shopify_function_alloc"]
pub unsafe extern "C" fn shopify_function_alloc(size: usize) -> *mut std::ffi::c_void {
    let new_mem = match size {
        0 => ZERO_SIZE_ALLOCATION_PTR,
        // this call to `alloc` is safe since `size` must be > 0
        _ => alloc(Layout::from_size_align(size, 1).unwrap()),
    };
    new_mem as _
}
