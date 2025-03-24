use core::ffi::c_void;
use shopify_function_wasm_api_core::InternedStringId;
use std::cell::UnsafeCell;
pub(crate) struct StringInterner {
    buf: UnsafeCell<Vec<u8>>,
    spans: UnsafeCell<Vec<(usize, usize)>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            buf: Default::default(),
            spans: Default::default(),
        }
    }

    pub fn preallocate(&self, len: usize) -> (InternedStringId, *const c_void) {
        // SAFETY: We know the Wasm environment is single-threaded, so we don't
        // need to worry about concurrent access.
        // Since we only ever push to the end of the vector, we don't need to worry
        // that the spans reference invalid indexes.
        // In theory the pointer returned by `as_ptr` is invalidated by a subsequent
        // resize, but since the trampoline writes to the buffer immediately after
        // the call to `preallocate`, we know that the pointer will be valid.
        let buf = unsafe { &mut *self.buf.get() };
        let spans = unsafe { &mut *self.spans.get() };
        let offset = buf.len();
        buf.resize(offset + len, 0);
        let id = spans.len();
        spans.push((offset, len));
        (id, buf[offset..].as_ptr() as *const c_void)
    }

    pub fn get(&self, id: InternedStringId) -> &[u8] {
        let buf = unsafe { &*self.buf.get() };
        let spans = unsafe { &*self.spans.get() };
        let (offset, len) = spans[id];
        &buf[offset..offset + len]
    }
}

/// SAFETY: We know the Wasm environment is single-threaded, so the concept of
/// `Sync` is basically meaningless.
unsafe impl Sync for StringInterner {}
