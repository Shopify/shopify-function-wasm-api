use crate::{decorate_for_target, Context};

const CAPACITY: usize = 1024;

#[derive(Debug)]
pub(crate) struct Logs {
    buffer: [u8; 1024],
    inline_len: usize,
    overflow_buffer: Vec<u8>,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; 1024],
            inline_len: 0,
            overflow_buffer: Vec::new(),
        }
    }
}

impl Logs {
    fn append(&mut self, len: usize) -> *const u8 {
        if self.overflow_buffer.is_empty() {
            // Try to fit in inline buffer first
            let remaining = CAPACITY - self.inline_len;
            if len <= remaining {
                let inline_len = self.inline_len;
                self.inline_len += len;
                return unsafe { self.buffer.as_ptr().add(inline_len) };
            }
            // Move to overflow buffer
            self.overflow_buffer.reserve(self.inline_len + len);
            self.overflow_buffer
                .extend_from_slice(&self.buffer[..self.inline_len]);
        }

        let buffer_len = self.overflow_buffer.len();
        self.overflow_buffer.resize(buffer_len + len, 0);
        unsafe { self.overflow_buffer.as_ptr().add(buffer_len) }
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        if self.overflow_buffer.is_empty() {
            self.buffer.as_ptr()
        } else {
            self.overflow_buffer.as_ptr()
        }
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn len(&self) -> usize {
        if self.overflow_buffer.is_empty() {
            self.inline_len
        } else {
            self.overflow_buffer.len()
        }
    }
}

impl Context {
    fn allocate_log(&mut self, len: usize) -> *const u8 {
        self.logs.append(len)
    }
}

decorate_for_target! {
    /// The most significant 32 bits are the length, the least significant 32 bits are the pointer.
    fn shopify_function_log_new_utf8_str(len: usize) -> usize {
        Context::with_mut(|context| {
            let ptr = context.allocate_log(len);
            ptr as usize
        })
    }
}
