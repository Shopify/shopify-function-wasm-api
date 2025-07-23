//! The log API for the Shopify Function Wasm API.

use std::cell::RefCell;

use crate::Context;

thread_local! {
    static LOGS: RefCell<Logs> = RefCell::new(Logs::default());
}

const CAPACITY: usize = 1024;

#[derive(Debug)]
pub(crate) struct Logs {
    buffer: [u8; CAPACITY],
    overflow_buffer: Vec<u8>,
    inline_len: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; CAPACITY],
            overflow_buffer: Vec::new(),
            inline_len: 0,
        }
    }
}

impl Logs {
    fn append(&mut self, buffer: &[u8]) {
        if self.overflow_buffer.is_empty() {
            // Try to fit in inline buffer first
            let remaining = CAPACITY - self.inline_len;
            if buffer.len() <= remaining {
                self.buffer[self.inline_len..self.inline_len + buffer.len()]
                    .copy_from_slice(buffer);
                self.inline_len += buffer.len();
                return;
            }
            // Move to overflow buffer
            self.overflow_buffer.reserve(self.inline_len + buffer.len());
            self.overflow_buffer
                .extend_from_slice(&self.buffer[..self.inline_len]);
        }
        self.overflow_buffer.extend_from_slice(buffer);
    }

    fn read_ptrs(&self) -> (*const u8, usize) {
        if self.overflow_buffer.is_empty() {
            (self.buffer.as_ptr(), self.inline_len)
        } else {
            (self.overflow_buffer.as_ptr(), self.overflow_buffer.len())
        }
    }
}

impl Context {
    /// Log `message`.
    pub fn log(&self, message: &str) {
        log(message)
    }
}

#[no_mangle]
extern "C" fn logs() -> u64 {
    LOGS.with_borrow(|logs| {
        let (ptr, len) = logs.read_ptrs();
        ((len as u64) << u32::BITS) | (ptr as u64)
    })
}

pub(crate) fn log(message: &str) {
    LOGS.with_borrow_mut(|logs| logs.append(message.as_bytes()))
}
