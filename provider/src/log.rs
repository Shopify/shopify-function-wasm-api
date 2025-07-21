use crate::{decorate_for_target, Context, DoubleUsize};

const CAPACITY: usize = 1024;

#[derive(Debug)]
pub(crate) struct Logs {
    buffer: [u8; CAPACITY],
    write_offset: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; CAPACITY],
            write_offset: 0,
        }
    }
}

impl Logs {
    fn append(&mut self, len: usize) -> (*const u8, usize) {
        let mut ret_len = len;
        let remaining_capacity = CAPACITY - self.write_offset;
        if len > remaining_capacity {
            ret_len = remaining_capacity;
        }
        let write_offset = self.write_offset;
        self.write_offset += ret_len;
        (unsafe { self.buffer.as_ptr().add(write_offset) }, ret_len)
    }

    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    pub(crate) fn len(&self) -> usize {
        self.write_offset
    }
}

impl Context {
    fn allocate_log(&mut self, len: usize) -> (*const u8, usize) {
        self.logs.append(len)
    }
}

decorate_for_target! {
    /// The most significant 32 bits are the length, the least significant 32 bits are the pointer.
    fn shopify_function_log_new_utf8_str(len: usize) -> DoubleUsize {
        Context::with_mut(|context| {
            let (ptr, len) = context.allocate_log(len);
            ((len as DoubleUsize) << usize::BITS) | ptr as DoubleUsize
        })
    }
}
