use core::ffi::c_void;
use shopify_function_wasm_api_core::write::WriteResult;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error")]
    IoError,
}

fn map_result(result: WriteResult) -> Result<(), Error> {
    match result {
        WriteResult::Ok => Ok(()),
        WriteResult::IoError => Err(Error::IoError),
    }
}

pub struct ValueSerializer(*mut c_void);

impl ValueSerializer {
    pub fn new() -> Self {
        Self(unsafe { crate::shopify_function_output_new() as *mut _ })
    }

    pub fn write_bool(&mut self, value: bool) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_bool(self.0 as _, value as u32) })
    }

    pub fn write_null(&mut self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_null(self.0 as _) })
    }

    pub fn write_i32(&mut self, value: i32) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_i32(self.0 as _, value) })
    }

    pub fn write_f64(&mut self, value: f64) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_f64(self.0 as _, value) })
    }

    pub fn write_utf8_str(&mut self, value: &str) -> Result<(), Error> {
        map_result(unsafe {
            crate::shopify_function_output_new_utf8_str(self.0 as _, value.as_ptr(), value.len())
        })
    }

    pub fn write_object<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
        len: usize,
    ) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_new_object(self.0 as _, len) })?;
        f(self)?;
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<(), Error> {
        map_result(unsafe { crate::shopify_function_output_finalize(self.0 as _) })
    }
}

impl Default for ValueSerializer {
    fn default() -> Self {
        Self::new()
    }
}
