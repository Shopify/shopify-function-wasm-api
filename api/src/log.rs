//! The log API for the Shopify Function Wasm API.

use crate::Context;

pub(super) fn log_utf8_str(message: &str) {
    unsafe { crate::shopify_function_log_new_utf8_str(message.as_ptr(), message.len()) };
}

impl Context {
    /// Log `message`.
    pub fn log(&mut self, message: &str) {
        log_utf8_str(message)
    }
}
