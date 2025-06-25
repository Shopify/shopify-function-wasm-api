use crate::{decorate_for_target, Context, DoubleUsize};
use shopify_function_wasm_api_core::log::LogResult;

impl Context {
    fn allocate_log(&mut self, len: usize) -> *const u8 {
        let write_offset = self.logs.len();
        self.logs.append(&mut vec![0; len]);
        unsafe { self.logs.as_ptr().add(write_offset) }
    }
}

decorate_for_target! {
    /// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
    fn shopify_function_log_new_utf8_str(len: usize) -> DoubleUsize {
        Context::with_mut(|context| {
            let ptr = context.allocate_log(len);
            let result = LogResult::Ok;
            ((result as DoubleUsize) << usize::BITS) | ptr as DoubleUsize
        })
    }
}
