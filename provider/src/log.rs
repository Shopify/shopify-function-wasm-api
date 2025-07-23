use crate::{decorate_for_target, Context};

impl Context {
    fn allocate_log(&mut self, len: usize) -> *const u8 {
        let write_offset = self.logs.len();
        self.logs.resize(write_offset + len, 0);
        unsafe { self.logs.as_ptr().add(write_offset) }
    }
}

decorate_for_target! {
    /// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
    fn shopify_function_log_new_utf8_str(len: usize) -> usize {
        Context::with_mut(|context| {
            let ptr = context.allocate_log(len);
            ptr as usize
        })
    }
}
