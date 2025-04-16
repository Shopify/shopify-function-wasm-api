use core::ffi::c_void;
use shopify_function_wasm_api_core::InternedStringId;

pub(crate) struct StringInterner {
    buf: Vec<u8>,
    spans: Vec<(usize, usize)>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            buf: Default::default(),
            spans: Default::default(),
        }
    }

    pub fn preallocate(&mut self, len: usize) -> (InternedStringId, *const c_void) {
        let offset = self.buf.len();
        self.buf.resize(offset + len, 0);
        let id = self.spans.len();
        self.spans.push((offset, len));
        (id, self.buf[offset..].as_ptr() as *const c_void)
    }

    pub fn get(&self, id: InternedStringId) -> &[u8] {
        let (offset, len) = self.spans[id];
        &self.buf[offset..offset + len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preallocate() {
        let mut interner = StringInterner::new();

        let (id, ptr) = interner.preallocate(5);
        assert_eq!(id, 0);
        assert!(!ptr.is_null());
        assert_eq!(interner.buf.len(), 5);
        assert_eq!(interner.spans.len(), 1);
        assert_eq!(interner.spans[0], (0, 5));

        let (id2, ptr2) = interner.preallocate(10);
        assert_eq!(id2, 1);
        assert!(!ptr2.is_null());
        assert_eq!(interner.buf.len(), 15);
        assert_eq!(interner.spans.len(), 2);
        assert_eq!(interner.spans[1], (5, 10));
    }

    #[test]
    fn test_get() {
        let mut interner = StringInterner::new();

        let (id, ptr) = interner.preallocate(5);
        let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, 5) };
        slice.copy_from_slice(b"hello");

        let retrieved = interner.get(id);
        assert_eq!(retrieved, b"hello");

        let (id2, ptr2) = interner.preallocate(6);
        let slice2 = unsafe { std::slice::from_raw_parts_mut(ptr2 as *mut u8, 6) };
        slice2.copy_from_slice(b"world!");

        assert_eq!(interner.get(id), b"hello");
        assert_eq!(interner.get(id2), b"world!");
    }

    #[test]
    #[should_panic]
    fn test_get_invalid_id() {
        let interner = StringInterner::new();
        interner.get(0); // Should panic as no strings have been interned
    }
}
