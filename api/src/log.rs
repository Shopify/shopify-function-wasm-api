//! The log API for the Shopify Function Wasm API.

/// Configures panics to write to the logging API.
pub fn init_panic_handler() {
    #[cfg(target_family = "wasm")]
    std::panic::set_hook(Box::new(|info| {
        let message = format!("{info}");
        log_utf8_str(&message);
    }));
}

/// Log `message`.
pub fn log_utf8_str(message: &str) {
    unsafe { crate::shopify_function_log_new_utf8_str(message.as_ptr(), message.len()) };
}
