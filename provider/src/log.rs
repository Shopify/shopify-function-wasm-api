use std::ptr;

use crate::{decorate_for_target, Context};

static mut LOG_RET_AREA: [usize; 5] = [0; 5];
// One more byte so we can check if we're truncating.
const CAPACITY: usize = 1001;

// A kind of ring buffer implementation. Since all reads are guaranteed to
// start after all writes have finished, we can simplify the
// implementation by only using a single offset for reads and writes.
#[derive(Debug)]
pub(crate) struct Logs {
    buffer: [u8; CAPACITY],
    offset: usize,
    len: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            buffer: [0; CAPACITY],
            offset: 0,
            len: 0,
        }
    }
}

impl Logs {
    fn append(&mut self, mut len: usize) -> (usize, *const u8, usize, *const u8, usize) {
        let mut source_offset = 0;
        let dst_offset1 = unsafe { self.buffer.as_ptr().add(self.offset) };
        let len1;
        let mut dst_offset2 = ptr::null();
        let mut len2 = 0;

        // Need to strip off start of incoming buffer if the incoming buffer exceeds capacity.
        if len > CAPACITY {
            source_offset = len - CAPACITY;
            len = CAPACITY;
        }

        let space_to_end = CAPACITY - self.offset;
        if len <= space_to_end {
            // Incoming buffer fits in one block.
            len1 = len;
            self.len = (self.len + len).min(CAPACITY);
        } else {
            // Incoming data wrap will wrap around.
            len1 = space_to_end;
            dst_offset2 = self.buffer.as_ptr();
            len2 = len - space_to_end;
            self.len = CAPACITY;
        }

        self.offset = (self.offset + len) % CAPACITY;

        (source_offset, dst_offset1, len1, dst_offset2, len2)
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn read_ptrs(&self) -> (*const u8, usize, *const u8, usize) {
        // _After_ filling the buffer, the read offset will _always_ be the
        // same as the write offset.
        let read_offset = if self.len < CAPACITY { 0 } else { self.offset };

        if read_offset == 0 {
            (self.buffer.as_ptr(), self.len, ptr::null(), 0)
        } else {
            let data_to_end = CAPACITY - read_offset;
            (
                unsafe { self.buffer.as_ptr().add(self.offset) },
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
        assert_eq!(logs.len, 100);
        assert_eq!(logs.offset, 100);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 100);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
    }

    #[test]
    fn test_append_exceeds_capacity() {
        let mut logs = Logs::default();
        let large_len = CAPACITY + 100;

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(large_len);

        assert_eq!(source_offset, 100);
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, CAPACITY);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
    }

    #[test]
    fn test_append_zero_length() {
        let mut logs = Logs::default();
        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(0);

        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, 0);
        assert_eq!(logs.offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 0);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
    }

    #[test]
    fn test_append_exact_capacity() {
        let mut logs = Logs::default();
        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(CAPACITY);

        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.offset, 0);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, CAPACITY);
        assert_eq!(len2, 0);
        assert!(ptr2.is_null());
    }

    #[test]
    fn test_append_multiple_operations() {
        let mut logs = Logs::default();

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(300);
        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, 300);
        assert_eq!(logs.offset, 300);
        assert_eq!(ptr1, logs.buffer.as_ptr());
        assert_eq!(len1, 300);
        assert_eq!(ptr2, ptr::null());
        assert_eq!(len2, 0);

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(200);
        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, 500);
        assert_eq!(logs.offset, 500);
        assert_eq!(ptr1, unsafe { logs.buffer.as_ptr().add(300) });
        assert_eq!(len1, 200);
        assert_eq!(ptr2, ptr::null());
        assert_eq!(len2, 0);

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(600); // Total would be 1100, exceeds capacity (1001)
        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.offset, 99); // (500 + 600) % CAPACITY
        assert_eq!(ptr1, unsafe { logs.buffer.as_ptr().add(500) });
        assert_eq!(len1, 501);
        assert_eq!(ptr2, logs.buffer.as_ptr());
        assert_eq!(len2, 99);

        let (source_offset, ptr1, len1, ptr2, len2) = logs.append(100); // Total would be 1200
        assert_eq!(source_offset, 0);
        assert_eq!(logs.len, CAPACITY);
        assert_eq!(logs.offset, 199); // (500 + 600 + 100) % CAPACITY
        assert_eq!(ptr1, unsafe { logs.buffer.as_ptr().add(99) });
        assert_eq!(len1, 100);
        assert_eq!(ptr2, ptr::null());
        assert_eq!(len2, 0);
    }
}
