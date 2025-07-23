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
    read_offset: usize,
    write_offset: usize,
    len: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; CAPACITY],
            read_offset: 0,
            write_offset: 0,
            len: 0,
        }
    }
}

impl Logs {
    fn append(&mut self, mut buffer: &[u8]) {
        // Need to strip off start of incoming buffer if the incoming buffer exceeds capacity.
        if buffer.len() > CAPACITY {
            buffer = &buffer[(buffer.len() - CAPACITY)..];
        }

        let space_to_end = CAPACITY - self.write_offset;
        if buffer.len() <= space_to_end {
            // Incoming buffer fits in one block.
            self.buffer[self.write_offset..(self.write_offset + buffer.len())]
                .copy_from_slice(buffer);
        } else {
            // Incoming data wrap will wrap around.
            self.buffer[self.write_offset..].copy_from_slice(&buffer[..space_to_end]);
            self.buffer[..(buffer.len() - space_to_end)].copy_from_slice(&buffer[space_to_end..]);
        }

        self.write_offset = (self.write_offset + buffer.len()) % CAPACITY;

        if self.len + buffer.len() <= CAPACITY {
            // No overwriting.
            self.len += buffer.len();
        } else {
            // Overwriting.
            let overwritten_bytes = self.len + buffer.len() - CAPACITY;
            self.read_offset = (self.read_offset + overwritten_bytes) % CAPACITY;
            self.len = CAPACITY;
        }
    }

    fn read_ptrs(&self) -> (*const u8, u16, u16) {
        (
            self.buffer.as_ptr(),
            self.read_offset as u16,
            self.len as u16,
        )
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
        let (ptr, read_offset, len) = logs.read_ptrs();
        ((len as u64) << (u32::BITS + u16::BITS))
            | ((read_offset as u64) << u32::BITS)
            | (ptr as u64)
    })
}

pub(crate) fn log(message: &str) {
    LOGS.with_borrow_mut(|logs| logs.append(message.as_bytes()))
}
