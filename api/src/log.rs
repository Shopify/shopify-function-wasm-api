//! The log API for the Shopify Function Wasm API.

use std::cell::RefCell;

use crate::Context;

const CAPACITY: usize = 1024;

thread_local! {
    static LOGS: RefCell<Logs> = RefCell::new(Logs::default());
}

#[derive(Debug)]
pub(crate) struct Logs {
    buffer: [u8; CAPACITY],
    len: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; CAPACITY],
            len: 0,
        }
    }
}

impl Logs {
    fn append(&mut self, mut buffer: &[u8]) {
        let remaining_capacity = CAPACITY - self.len;
        if buffer.len() > remaining_capacity {
            buffer = &buffer[0..remaining_capacity];
        }
        self.buffer[self.len..(self.len + buffer.len())].copy_from_slice(buffer);
        self.len += buffer.len();
    }

    fn read_ptrs(&self) -> (*const u8, usize) {
        (self.buffer.as_ptr(), self.len)
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
