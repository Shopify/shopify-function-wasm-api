use crate::{decorate_for_target, DoubleUsize};
use shopify_function_wasm_api_core::log::LogResult;
use std::cell::RefCell;

thread_local! {
    static LOGS: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(1024));
}

fn allocate_log(len: usize) -> *const u8 {
    LOGS.with_borrow_mut(|logs| {
        let write_index = logs.len();
        logs.append(&mut vec![0; len]);
        unsafe { logs.as_ptr().add(write_index) }
    })
}

decorate_for_target! {
    /// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
    fn shopify_function_log_new_utf8_str(len: usize) -> DoubleUsize {
        let ptr = allocate_log(len);
        let result = LogResult::Ok;
        ((result as DoubleUsize) << usize::BITS) | ptr as DoubleUsize
    }
}

/// The most significant 32 bits are the pointer, the least significant 32 bits are the length.
pub fn shopify_function_retrieve_logs() -> DoubleUsize {
    LOGS.with_borrow(|logs| {
        let ptr = logs.as_ptr();
        let len = logs.len();
        ((ptr as DoubleUsize) << usize::BITS) | len as DoubleUsize
    })
}

#[cfg(target_family = "wasm")]
#[export_name = "shopify_function_retrieve_logs"]
extern "C" fn shopify_function_retrieve_logs2() -> DoubleUsize {
    shopify_function_retrieve_logs()
}
