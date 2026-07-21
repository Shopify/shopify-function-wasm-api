use crate::{decorate_for_target, Context, DoubleUsize};
use shopify_function_wasm_api_core::write::WriteResult;

mod state;

pub(crate) use state::State;

/// The 5-byte FBF payload header: magic (`"FBF"`) + version (`1`) + flags (`0`).
///
/// The streaming writer emits neither a string table nor a shape table, so the
/// flags byte is always zero.
pub(crate) const HEADER: [u8; 5] = [b'F', b'B', b'F', 0x01, 0x00];

/// Number of bytes reserved for a container's fixed-width length field.
///
/// The writer always emits the widest counted container form (`array32` /
/// `map32`). This keeps the container header size known up front — before its
/// children are written — so its byte-length prefix can be back-filled in place
/// once the container closes, avoiding a per-container scratch buffer and the
/// copy-up-the-tree that a smallest-width encoder would require.
const LENGTH_FIELD_WIDTH: usize = 4;

const ARRAY32_TAG: u8 = 0x95;
const MAP32_TAG: u8 = 0x98;

/// Creates a fresh output buffer pre-seeded with the FBF payload header.
///
/// The header lives at the front of the buffer for the whole write, so values
/// are appended directly after it with no final prepend/shift.
pub(crate) fn new_output_buffer() -> Vec<u8> {
    let mut buffer = Vec::with_capacity(1024);
    buffer.extend_from_slice(&HEADER);
    buffer
}

#[derive(Debug, PartialEq)]
pub(crate) enum OutputContainerKind {
    Array,
    Object,
}

#[derive(Debug)]
pub(crate) struct OutputContainer {
    kind: OutputContainerKind,
    /// Offset of the reserved 4-byte length field within `output_bytes`.
    length_field_offset: usize,
}

fn put_uint(out: &mut Vec<u8>, value: u64) {
    match value {
        0..=0x7f => out.push(value as u8),
        0x80..=0xff => {
            out.push(0x87);
            out.push(value as u8);
        }
        0x100..=0xffff => {
            out.push(0x88);
            out.extend_from_slice(&(value as u16).to_le_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(0x89);
            out.extend_from_slice(&(value as u32).to_le_bytes());
        }
        _ => {
            out.push(0x8a);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn put_i32(out: &mut Vec<u8>, value: i32) {
    match value {
        0..=0x7f => out.push(value as u8),
        -32..=-1 => out.push((value + 256) as u8),
        -128..=-33 => {
            out.push(0x83);
            out.push(value as i8 as u8);
        }
        -32_768..=-129 | 128..=32_767 => {
            out.push(0x84);
            out.extend_from_slice(&(value as i16).to_le_bytes());
        }
        _ if value < 0 => {
            out.push(0x85);
            out.extend_from_slice(&value.to_le_bytes());
        }
        _ => put_uint(out, value as u64),
    }
}

fn put_str_header(out: &mut Vec<u8>, len: usize) -> bool {
    if len <= 31 {
        out.push(0xa2 + len as u8);
    } else if len <= u8::MAX as usize {
        out.push(0x8d);
        out.push(len as u8);
    } else if len <= u16::MAX as usize {
        out.push(0x8e);
        out.extend_from_slice(&(len as u16).to_le_bytes());
    } else if len <= u32::MAX as usize {
        out.push(0x8f);
        out.extend_from_slice(&(len as u32).to_le_bytes());
    } else {
        return false;
    }
    true
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

impl Context {
    fn start_container(&mut self, tag: u8, count: usize, kind: OutputContainerKind) {
        let out = &mut self.output_bytes;
        out.push(tag);
        let length_field_offset = out.len();
        out.extend_from_slice(&[0; LENGTH_FIELD_WIDTH]);
        // The count varint is the first part of the container payload.
        write_varint(out, count as u64);
        self.output_container_stack.push(OutputContainer {
            kind,
            length_field_offset,
        });
    }

    fn finish_container(&mut self, expected: OutputContainerKind) -> WriteResult {
        let Some(container) = self.output_container_stack.pop() else {
            return match expected {
                OutputContainerKind::Array => WriteResult::NotAnArray,
                OutputContainerKind::Object => WriteResult::NotAnObject,
            };
        };
        if container.kind != expected {
            return match expected {
                OutputContainerKind::Array => WriteResult::NotAnArray,
                OutputContainerKind::Object => WriteResult::NotAnObject,
            };
        }
        // The length field counts every payload byte after it: the count varint
        // plus all child values already appended in place.
        let payload_start = container.length_field_offset + LENGTH_FIELD_WIDTH;
        let payload_len = self.output_bytes.len() - payload_start;
        let Ok(payload_len) = u32::try_from(payload_len) else {
            return WriteResult::IoError;
        };
        self.output_bytes[container.length_field_offset..payload_start]
            .copy_from_slice(&payload_len.to_le_bytes());
        WriteResult::Ok
    }

    fn write_bool(&mut self, bool: bool) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(if bool { 0x82 } else { 0x81 });
        WriteResult::Ok
    }

    fn write_nil(&mut self) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(0x80);
        WriteResult::Ok
    }

    fn write_i32(&mut self, int: i32) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        put_i32(&mut self.output_bytes, int);
        WriteResult::Ok
    }

    fn write_f64(&mut self, float: f64) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(0x8c);
        self.output_bytes.extend_from_slice(&float.to_le_bytes());
        WriteResult::Ok
    }

    fn allocate_utf8_str(&mut self, len: usize) -> (WriteResult, *const u8) {
        let result = self.write_state.write_string();
        if result != WriteResult::Ok {
            return (result, std::ptr::null());
        }
        let out = &mut self.output_bytes;
        if !put_str_header(out, len) {
            return (WriteResult::IoError, std::ptr::null());
        }
        let offset = out.len();
        out.resize(offset + len, 0);
        (WriteResult::Ok, out[offset..].as_ptr())
    }

    fn start_object(&mut self, len: usize) -> WriteResult {
        let result = self
            .write_state
            .start_object(len, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        self.start_container(MAP32_TAG, len, OutputContainerKind::Object);
        WriteResult::Ok
    }

    fn finish_object(&mut self) -> WriteResult {
        let result = self
            .write_state
            .finish_object(&mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        self.finish_container(OutputContainerKind::Object)
    }

    fn start_array(&mut self, len: usize) -> WriteResult {
        let result = self
            .write_state
            .start_array(len, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        self.start_container(ARRAY32_TAG, len, OutputContainerKind::Array);
        WriteResult::Ok
    }

    fn finish_array(&mut self) -> WriteResult {
        let result = self
            .write_state
            .finish_array(&mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        self.finish_container(OutputContainerKind::Array)
    }

    fn write_interned_utf8_str(
        &mut self,
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> WriteResult {
        let string_data = self.string_interner.get(id);
        let len = string_data.len();
        let ptr = string_data.as_ptr();
        let (result, output_ptr) = self.allocate_utf8_str(len);
        if result != WriteResult::Ok {
            return result;
        }
        unsafe { std::ptr::copy_nonoverlapping(ptr, output_ptr as *mut u8, len) };
        WriteResult::Ok
    }

    pub(crate) fn finalize_output_bytes(&mut self) -> WriteResult {
        if self.write_state != State::End || !self.output_container_stack.is_empty() {
            return WriteResult::ValueNotFinished;
        }
        WriteResult::Ok
    }
}

decorate_for_target! {
    fn shopify_function_output_new_bool(bool: u32) -> WriteResult {
        Context::with_mut(|context| {
            context.write_bool(bool != 0)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_null() -> WriteResult {
        Context::with_mut(|context| {
            context.write_nil()
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_i32(int: i32) -> WriteResult {
        Context::with_mut(|context| {
            context.write_i32(int)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_f64(float: f64) -> WriteResult {
        Context::with_mut(|context| {
            context.write_f64(float)
        })
    }
}

decorate_for_target! {
    /// The most significant 32 bits are the result, the least significant 32 bits are the pointer.
    fn shopify_function_output_new_utf8_str(len: usize) -> DoubleUsize {
        Context::with_mut(|context| {
            let (result, ptr) = context.allocate_utf8_str(len);
            ((result as DoubleUsize) << usize::BITS) | ptr as DoubleUsize
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_object(
        len: usize,
    ) -> WriteResult {
        Context::with_mut(|context| {
            context.start_object(len)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_finish_object() -> WriteResult {
        Context::with_mut(|context| {
            context.finish_object()
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_array(
        len: usize,
    ) -> WriteResult {
        Context::with_mut(|context| {
            context.start_array(len)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_finish_array() -> WriteResult {
        Context::with_mut(|context| {
            context.finish_array()
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_interned_utf8_str(
        id: shopify_function_wasm_api_core::InternedStringId,
    ) -> WriteResult {
        Context::with_mut(|context| {
            context.write_interned_utf8_str(id)
        })
    }
}

#[cfg(not(target_family = "wasm"))]
pub fn shopify_function_output_finalize_and_return_fbf_bytes() -> (WriteResult, Vec<u8>) {
    Context::with_mut(|context| {
        let result = context.finalize_output_bytes();
        if result != WriteResult::Ok {
            return (result, Vec::new());
        }
        (WriteResult::Ok, context.output_bytes.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_to_json(bytes: &[u8]) -> serde_json::Value {
        fbf::from_slice(bytes).unwrap()
    }

    fn write_key(context: &mut Context, key: &str) -> WriteResult {
        let (result, ptr) = context.allocate_utf8_str(key.len());
        if result != WriteResult::Ok {
            return result;
        }
        unsafe { std::ptr::copy_nonoverlapping(key.as_ptr(), ptr as *mut u8, key.len()) };
        WriteResult::Ok
    }

    fn finalize_context(context: &mut Context) -> serde_json::Value {
        assert_eq!(context.finalize_output_bytes(), WriteResult::Ok);
        bytes_to_json(&context.output_bytes)
    }

    #[test]
    fn test_write_context_bool() {
        let mut context = Context::new(Vec::new());
        context.write_bool(true);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_bool(true), WriteResult::ValueAlreadyWritten);
        assert_eq!(finalize_context(&mut context), serde_json::json!(true));
    }

    #[test]
    fn test_write_context_null() {
        let mut context = Context::new(Vec::new());
        context.write_nil();
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_nil(), WriteResult::ValueAlreadyWritten);
        assert_eq!(finalize_context(&mut context), serde_json::json!(null));
    }

    #[test]
    fn test_write_context_i32() {
        let mut context = Context::new(Vec::new());
        context.write_i32(42);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_i32(42), WriteResult::ValueAlreadyWritten);
        assert_eq!(finalize_context(&mut context), serde_json::json!(42));
    }

    #[test]
    fn test_write_context_f64() {
        let mut context = Context::new(Vec::new());
        context.write_f64(42.0);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_f64(42.0), WriteResult::ValueAlreadyWritten);
        assert_eq!(finalize_context(&mut context), serde_json::json!(42.0));
    }

    #[test]
    fn test_write_context_string() {
        let mut context = Context::new(Vec::new());
        let (result, ptr) = context.allocate_utf8_str(5);
        assert_eq!(result, WriteResult::Ok);
        unsafe { std::ptr::copy_nonoverlapping(b"hello".as_ptr(), ptr as *mut u8, 5) };
        assert_eq!(context.write_state, State::End);
        assert_eq!(finalize_context(&mut context), serde_json::json!("hello"));
    }

    #[test]
    fn test_write_context_array() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_array(2), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(context.write_i32(2), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);
        assert_eq!(finalize_context(&mut context), serde_json::json!([1, 2]));
    }

    #[test]
    fn test_write_context_nested() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_object(2), WriteResult::Ok);
        assert_eq!(write_key(&mut context, "a"), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(write_key(&mut context, "nested"), WriteResult::Ok);
        assert_eq!(context.start_array(3), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(context.write_bool(false), WriteResult::Ok);
        assert_eq!(context.write_nil(), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);
        assert_eq!(
            finalize_context(&mut context),
            serde_json::json!({"a": 1, "nested": [1, false, null]})
        );
    }

    #[test]
    fn test_write_context_object() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_object(2), WriteResult::Ok);
        assert_eq!(write_key(&mut context, "a"), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(write_key(&mut context, "b"), WriteResult::Ok);
        assert_eq!(context.write_bool(true), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);
        assert_eq!(
            finalize_context(&mut context),
            serde_json::json!({"a": 1, "b": true})
        );
    }

    #[test]
    fn test_write_context_empty_array() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.start_array(0), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);
        assert_eq!(finalize_context(&mut context), serde_json::json!([]));
    }
}
