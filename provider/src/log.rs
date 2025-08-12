use std::ptr;

use crate::{decorate_for_target, Context};

static mut LOG_RET_AREA: [usize; 5] = [0; 5];
// One more byte so we can check if we're truncating.
const CAPACITY: usize = 1025;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_fits_in_buffer() {
        let mut logs = Logs::default();
        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(100);

        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 100);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
        assert_eq!(logs.len, 100);
        assert_eq!(logs.write_offset, 100);
        assert_eq!(logs.read_offset, 0);
    }

    #[test]
    fn test_append_exceeds_capacity() {
        let mut logs = Logs::default();
        let large_len = CAPACITY + 100;

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(large_len);

        assert_eq!(source_offset, 100);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, CAPACITY);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.write_offset, 0);
        assert_eq!(logs.read_offset, 0);
    }

    #[test]
    fn test_append_zero_length() {
        let mut logs = Logs::default();
        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(0);

        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 0);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
        assert_eq!(logs.len, 0);
        assert_eq!(logs.write_offset, 0);
        assert_eq!(logs.read_offset, 0);
    }

    #[test]
    fn test_append_exact_capacity() {
        let mut logs = Logs::default();
        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(CAPACITY);

        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, CAPACITY);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.write_offset, 0);
        assert_eq!(logs.read_offset, 0);
    }

    #[test]
    fn test_append_multiple_operations() {
        let mut logs = Logs::default();

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(300);
        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 300);
        assert_eq!(ptr2, ptr::null());
        assert_eq!(len2, 0);
        assert_eq!(logs.len, 300);
        assert_eq!(logs.write_offset, 300);
        assert_eq!(logs.read_offset, 0);

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(200);
        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, unsafe { logs.buffer.as_ptr().add(300) });
        assert_eq!(len1, 200);
        assert_eq!(ptr2, ptr::null());
        assert_eq!(len2, 0);
        assert_eq!(logs.len, 500);
        assert_eq!(logs.write_offset, 500);
        assert_eq!(logs.read_offset, 0);

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(600); // Total would be 1100, exceeds CAPACITY (1025)
        assert_eq!(source_offset, 0);
        assert_eq!(ptr1, unsafe { logs.buffer.as_ptr().add(500) });
        assert_eq!(len1, 525);
        assert_eq!(ptr2, logs.buffer.as_ptr());
        assert_eq!(len2, 75);
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.write_offset, 75); // (500 + 600) % CAPACITY
        assert_eq!(logs.read_offset, 75); // Advanced by overflow amount
    }
}
