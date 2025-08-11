use std::ptr;

use crate::{decorate_for_target, Context};

static mut LOG_RET_AREA: [usize; 5] = [0; 5];
const CAPACITY: usize = 1024;

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
    fn append(&mut self, mut len: usize) -> (usize, *const u8, usize, *const u8, usize) {
        let mut source_offset = 0;
        let dst_offset1 = unsafe { self.buffer.as_ptr().add(self.write_offset) };
        let len1;
        let mut dst_offset2 = ptr::null();
        let mut len2 = 0;

        // Need to strip off start of incoming buffer if the incoming buffer exceeds capacity.
        if len > CAPACITY {
            source_offset = len - CAPACITY;
            len = CAPACITY;
        }

        let space_to_end = CAPACITY - self.write_offset;
        if len <= space_to_end {
            // Incoming buffer fits in one block.
            len1 = len;
        } else {
            // Incoming data wrap will wrap around.
            len1 = space_to_end;
            dst_offset2 = self.buffer.as_ptr();
            len2 = len - space_to_end;
        }

        self.write_offset = (self.write_offset + len) % CAPACITY;

        if self.len + len <= CAPACITY {
            // No overwriting.
            self.len += len;
        } else {
            // Overwriting.
            let overwritten_bytes = self.len + len - CAPACITY;
            self.read_offset = (self.read_offset + overwritten_bytes) % CAPACITY;
            self.len = CAPACITY;
        }

        (source_offset, dst_offset1, len1, dst_offset2, len2)
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn read_ptrs(&self) -> (*const u8, usize, *const u8, usize) {
        let data_to_end = CAPACITY - self.read_offset;
        if self.len <= data_to_end {
            (
                unsafe { self.buffer.as_ptr().add(self.read_offset) },
                self.len,
                ptr::null(),
                0,
            )
        } else {
            (
                unsafe { self.buffer.as_ptr().add(self.read_offset) },
                data_to_end,
                self.buffer.as_ptr(),
                self.len - data_to_end,
            )
        }
    }
}

impl Context {
    fn allocate_log(&mut self, len: usize) -> (usize, *const u8, usize, *const u8, usize) {
        self.logs.append(len)
    }
}

decorate_for_target! {
    fn shopify_function_log_new_utf8_str(len: usize) -> *const usize {
        Context::with_mut(|context| {
            let (src_offset, ptr1, len1, ptr2, len2) = context.allocate_log(len);
            #[allow(static_mut_refs)] // This is _technically_ safe given this is single threaded.
            unsafe {
                LOG_RET_AREA[0] = src_offset;
                LOG_RET_AREA[1] = ptr1 as usize;
                LOG_RET_AREA[2] = len1;
                LOG_RET_AREA[3] = ptr2 as usize;
                LOG_RET_AREA[4] = len2;
                LOG_RET_AREA.as_ptr()
            }
        })
    }
}
