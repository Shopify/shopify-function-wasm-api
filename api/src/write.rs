use core::ffi::c_void;

pub struct ValueSerializer(*mut c_void);

impl ValueSerializer {
    pub fn new() -> Self {
        Self(unsafe { crate::shopify_function_output_new() as *mut _ })
    }

    pub fn write_bool(&mut self, value: bool) {
        unsafe { crate::shopify_function_output_new_bool(self.0 as _, value as u32) };
    }

    pub fn finalize(&mut self) {
        unsafe { crate::shopify_function_output_finalize(self.0 as _) };
    }
}

impl Default for ValueSerializer {
    fn default() -> Self {
        Self::new()
    }
}
