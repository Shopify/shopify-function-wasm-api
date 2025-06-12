use crate::{decorate_for_target, Context, DoubleUsize};
use shopify_function_wasm_api_core::ContextPtr;

pub enum LogResult {
    Ok,
    IoError,
}

pub(crate) struct LogBuffer {
    entries: Vec<u8>,
    write_index: usize,
}

impl LogBuffer {
    pub fn new() -> Self {
        let buffer = Self {
            entries: Vec::with_capacity(1024), // what should the initial capacity be?
            write_index: 0,
        };
        buffer
    }

    fn allocate(&mut self, len: usize) -> usize {
        self.entries.append(&mut vec![0; len]);
        let mut ptr = self.entries.as_mut_ptr() as usize;
        ptr += self.write_index;
        self.write_index += len;
        ptr
    }
}

decorate_for_target! {
    fn shopify_function_log_utf8_str(context: ContextPtr, len: usize) -> DoubleUsize {
        match Context::mut_from_raw(context) {
            Ok(context) => {
                let ptr = context.logs.allocate(len);
                ((LogResult::Ok as DoubleUsize) << usize::BITS) | ptr as DoubleUsize
            }
            Err(_) => (LogResult::IoError as DoubleUsize) << usize::BITS,
        }
    }
}
