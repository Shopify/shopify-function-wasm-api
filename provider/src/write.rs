use crate::{context_from_raw, Context};
use rmp::encode;
use shopify_function_wasm_api_core::{write::WriteResult, ContextPtr};
use std::io::Write;

mod state;

pub(crate) use state::State;

impl Context {
    fn write_bool(&mut self, bool: bool) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_bool(&mut self.output_bytes, bool).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn write_nil(&mut self) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_nil(&mut self.output_bytes).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn write_i32(&mut self, int: i32) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_sint(&mut self.output_bytes, int as i64).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn write_f64(&mut self, float: f64) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_f64(&mut self.output_bytes, float).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn allocate_utf8_str(&mut self, len: usize) -> (WriteResult, *const u8) {
        let result = self.write_state.write_string();
        if result != WriteResult::Ok {
            return (result, std::ptr::null());
        }
        encode::write_str_len(&mut self.output_bytes, len as u32).unwrap(); // infallible unwrap
        let original_len = self.output_bytes.as_slice().len();
        // fill in the new bytes with zeros; the trampoline will copy the string to overwrite them
        self.output_bytes.as_mut_vec().resize(original_len + len, 0);
        (
            WriteResult::Ok,
            self.output_bytes.as_slice()[original_len..].as_ptr(),
        )
    }

    fn start_object(&mut self, len: usize) -> WriteResult {
        let result = self
            .write_state
            .start_object(len, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_map_len(&mut self.output_bytes, len as u32).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn finish_object(&mut self) -> WriteResult {
        let result = self
            .write_state
            .finish_object(&mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        WriteResult::Ok
    }

    fn start_array(&mut self, len: usize) -> WriteResult {
        let result = self
            .write_state
            .start_array(len, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        encode::write_array_len(&mut self.output_bytes, len as u32).unwrap(); // infallible unwrap
        WriteResult::Ok
    }

    fn finish_array(&mut self) -> WriteResult {
        let result = self
            .write_state
            .finish_array(&mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        WriteResult::Ok
    }
}

#[export_name = "_shopify_function_output_new_bool"]
extern "C" fn shopify_function_output_new_bool(context: ContextPtr, bool: u32) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().write_bool(bool != 0) }
}

#[export_name = "_shopify_function_output_new_null"]
extern "C" fn shopify_function_output_new_null(context: ContextPtr) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().write_nil() }
}

#[export_name = "_shopify_function_output_new_i32"]
extern "C" fn shopify_function_output_new_i32(context: ContextPtr, int: i32) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().write_i32(int) }
}

#[export_name = "_shopify_function_output_new_f64"]
extern "C" fn shopify_function_output_new_f64(context: ContextPtr, float: f64) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().write_f64(float) }
}

/// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
#[export_name = "_shopify_function_output_new_utf8_str"]
extern "C" fn shopify_function_output_new_utf8_str(context: ContextPtr, len: usize) -> u64 {
    let mut context = context_from_raw(context);
    let (result, ptr) = unsafe { context.as_mut().allocate_utf8_str(len) };
    ((result as u64) << 32) | ptr as u64
}

#[export_name = "_shopify_function_output_new_object"]
extern "C" fn shopify_function_output_new_object(context: ContextPtr, len: usize) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().start_object(len) }
}

#[export_name = "_shopify_function_output_finish_object"]
extern "C" fn shopify_function_output_finish_object(context: ContextPtr) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().finish_object() }
}

#[export_name = "_shopify_function_output_new_array"]
extern "C" fn shopify_function_output_new_array(context: ContextPtr, len: usize) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().start_array(len) }
}

#[export_name = "_shopify_function_output_finish_array"]
extern "C" fn shopify_function_output_finish_array(context: ContextPtr) -> WriteResult {
    let mut context = context_from_raw(context);
    unsafe { context.as_mut().finish_array() }
}

#[export_name = "_shopify_function_output_finalize"]
extern "C" fn shopify_function_output_finalize(context: ContextPtr) -> WriteResult {
    let mut context = context_from_raw(context);
    let Context {
        output_bytes,
        write_state,
        ..
    } = unsafe { &mut context.as_mut() };
    if *write_state != State::End {
        return WriteResult::ValueNotFinished;
    }
    let mut stdout = std::io::stdout();
    // Temporary use of stdout to copy from linear memory to the host.
    // Preliminary benchmarking doesn't seem to suggest that this operation
    // represents a considerable overhead.
    if stdout.write_all(output_bytes.as_slice()).is_err() {
        return WriteResult::IoError;
    }
    if stdout.flush().is_err() {
        return WriteResult::IoError;
    }
    let _ = unsafe { Box::from_raw(context.as_ptr()) }; // drop the context
    WriteResult::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_to_json(bytes: &[u8]) -> serde_json::Value {
        let value = rmp_serde::from_slice(bytes).unwrap();
        serde_json::from_value(value).unwrap()
    }

    fn write_key(context: &mut Context, key: &str) -> WriteResult {
        let (result, ptr) = context.allocate_utf8_str(key.len());
        if result != WriteResult::Ok {
            return result;
        }
        unsafe { std::ptr::copy_nonoverlapping(key.as_ptr(), ptr as *mut u8, key.len()) };
        WriteResult::Ok
    }

    #[test]
    fn test_write_context_bool() {
        let mut context = Context::new(Vec::new());
        context.write_bool(true);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_bool(true), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!(true));
    }

    #[test]
    fn test_write_context_null() {
        let mut context = Context::new(Vec::new());
        context.write_nil();
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_nil(), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!(null));
    }

    #[test]
    fn test_write_context_i32() {
        let mut context = Context::new(Vec::new());
        context.write_i32(42);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_i32(42), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn test_write_context_f64() {
        let mut context = Context::new(Vec::new());
        context.write_f64(42.0);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_f64(42.0), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!(42.0));
    }

    #[test]
    fn test_write_context_utf8_str() {
        let mut context = Context::new(Vec::new());
        let s = "hello";
        let (result, ptr) = context.allocate_utf8_str(s.len());
        assert_eq!(result, WriteResult::Ok);
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), ptr as *mut u8, s.len());
        }
        let (result, ptr) = context.allocate_utf8_str(s.len());
        assert_eq!(result, WriteResult::ValueAlreadyWritten);
        assert_eq!(ptr, std::ptr::null());
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!(s));
    }

    #[test]
    fn test_write_context_object() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_object(2), WriteResult::Ok);
        assert_eq!(context.write_bool(true), WriteResult::ExpectedKey);
        assert_eq!(write_key(&mut context, "key"), WriteResult::Ok);
        assert_eq!(context.write_bool(false), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::ObjectLengthError);
        assert_eq!(write_key(&mut context, "other_key"), WriteResult::Ok);
        assert_eq!(context.start_object(0), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);
        assert_eq!(context.start_object(0), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!({ "key": false, "other_key": {} }));
    }

    #[test]
    fn test_write_context_array() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_array(2), WriteResult::Ok);
        assert_eq!(context.write_bool(true), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::ArrayLengthError);
        assert_eq!(context.start_array(0), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);
        assert_eq!(context.start_array(0), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(context.output_bytes.as_slice());
        assert_eq!(json, serde_json::json!([true, []]));
    }
}
