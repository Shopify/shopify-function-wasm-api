use std::alloc::{alloc, dealloc, Layout};
use std::ptr::copy_nonoverlapping;

const ZERO_SIZE_ALLOCATION_PTR: *mut u8 = 1 as _;

// Allocation functions
#[export_name = "shopify_function_realloc"]
pub unsafe extern "C" fn shopify_function_realloc(
    original_ptr: *mut u8,
    original_size: usize,
    alignment: usize,
    new_size: usize,
) -> *mut std::ffi::c_void {
    assert!(new_size >= original_size);

    let new_mem = match new_size {
        0 => ZERO_SIZE_ALLOCATION_PTR,
        // this call to `alloc` is safe since `new_size` must be > 0
        _ => alloc(Layout::from_size_align(new_size, alignment).unwrap()),
    };

    if !original_ptr.is_null() && original_size != 0 {
        copy_nonoverlapping(original_ptr, new_mem, original_size);
        shopify_function_free(original_ptr, original_size, alignment);
    }
    new_mem as _
}

#[export_name = "shopify_function_free"]
pub unsafe extern "C" fn shopify_function_free(ptr: *mut u8, size: usize, alignment: usize) {
    if size > 0 {
        dealloc(ptr, Layout::from_size_align(size, alignment).unwrap())
    };
}
