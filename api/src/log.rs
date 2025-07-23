//! The log API for the Shopify Function Wasm API.

use std::cell::RefCell;

use crate::Context;

thread_local! {
    static LOGS: RefCell<Logs> = RefCell::new(Logs::default());
}

#[derive(Debug)]
pub(crate) struct Logs {
    buffer: Vec<u8>,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
        }
    }
}

impl Logs {
    fn append(&mut self, buffer: &[u8]) {
        self.buffer.extend_from_slice(buffer);
    }

    fn read_ptrs(&self) -> (*const u8, usize) {
        (self.buffer.as_ptr(), self.buffer.len())
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
