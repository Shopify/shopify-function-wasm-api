use crate::{append_string_reference, decorate_for_target, Context, DoubleUsize};
use fbf::format::{
    FALSE, FIXARRAY0, FIXMAP0, FIXSTR_MAX_LEN, FIXSTR_MIN, FLOAT64, INT16, INT32, INT8, NIL,
    SEQARRAY, SEQFIXARRAY_MIN, SEQFIXMAP_MIN, SEQMAP, SEQSHAPE, STR16, STR32, STR8, TRUE, UINT16,
    UINT32, UINT8,
};
use shopify_function_wasm_api_core::{write::WriteResult, InternedStringId};

mod state;

pub(crate) use state::State;

impl Context {
    fn write_bool(&mut self, value: bool) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(if value { TRUE } else { FALSE });
        WriteResult::Ok
    }

    fn write_nil(&mut self) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(NIL);
        WriteResult::Ok
    }

    fn write_i32(&mut self, int: i32) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        match int {
            0..=127 => self.output_bytes.push(int as u8),
            -32..=-1 => self.output_bytes.push(int as i8 as u8),
            128..=255 => {
                self.output_bytes.push(UINT8);
                self.output_bytes.push(int as u8);
            }
            256..=65535 => {
                self.output_bytes.push(UINT16);
                self.output_bytes
                    .extend_from_slice(&(int as u16).to_le_bytes());
            }
            65536.. => {
                self.output_bytes.push(UINT32);
                self.output_bytes
                    .extend_from_slice(&(int as u32).to_le_bytes());
            }
            -128..=-33 => {
                self.output_bytes.push(INT8);
                self.output_bytes.push(int as i8 as u8);
            }
            -32768..=-129 => {
                self.output_bytes.push(INT16);
                self.output_bytes
                    .extend_from_slice(&(int as i16).to_le_bytes());
            }
            _ => {
                self.output_bytes.push(INT32);
                self.output_bytes.extend_from_slice(&int.to_le_bytes());
            }
        }
        WriteResult::Ok
    }

    fn write_f64(&mut self, float: f64) -> WriteResult {
        let result = self.write_state.write_non_string_scalar();
        if result != WriteResult::Ok {
            return result;
        }
        self.output_bytes.push(FLOAT64);
        self.output_bytes.extend_from_slice(&float.to_le_bytes());
        WriteResult::Ok
    }

    fn allocate_utf8_str(&mut self, len: usize) -> (WriteResult, *const u8) {
        let result = self.write_state.write_string();
        if result != WriteResult::Ok {
            return (result, std::ptr::null());
        }
        (WriteResult::Ok, self.allocate_utf8_str_unchecked(len))
    }

    fn allocate_utf8_str_unchecked(&mut self, len: usize) -> *const u8 {
        if len <= FIXSTR_MAX_LEN {
            self.output_bytes.push(FIXSTR_MIN + len as u8);
        } else if len <= u8::MAX as usize {
            self.output_bytes.push(STR8);
            self.output_bytes.push(len as u8);
        } else if len <= u16::MAX as usize {
            self.output_bytes.push(STR16);
            self.output_bytes
                .extend_from_slice(&(len as u16).to_le_bytes());
        } else {
            self.output_bytes.push(STR32);
            self.output_bytes
                .extend_from_slice(&(len as u32).to_le_bytes());
        }

        let original_len = self.output_bytes.len();
        // Fill in the new bytes with zeros; the trampoline will copy the string over them.
        self.output_bytes.resize(original_len + len, 0);
        self.output_bytes[original_len..].as_ptr()
    }

    fn start_object(&mut self, len: usize) -> WriteResult {
        let result = self
            .write_state
            .start_object(len, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }
        if len == 0 {
            self.output_bytes.push(FIXMAP0);
        } else if len <= 7 {
            self.output_bytes.push(SEQFIXMAP_MIN + (len as u8 - 1));
        } else {
            self.output_bytes.push(SEQMAP);
            fbf::varint::write(&mut self.output_bytes, len as u64);
        }
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
        if len == 0 {
            self.output_bytes.push(FIXARRAY0);
        } else if len <= 7 {
            self.output_bytes.push(SEQFIXARRAY_MIN + (len as u8 - 1));
        } else {
            self.output_bytes.push(SEQARRAY);
            fbf::varint::write(&mut self.output_bytes, len as u64);
        }
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

    fn start_shape_definition(&mut self, key_count: usize) -> WriteResult {
        if self.open_shape_definition.is_some() {
            return WriteResult::ShapeDefinitionError;
        }
        self.open_shape_definition = Some((key_count, self.shape_keys.len()));
        WriteResult::Ok
    }

    fn define_shape_key(&mut self, id: InternedStringId) -> WriteResult {
        let Some(&(key_count, key_start)) = self.open_shape_definition.as_ref() else {
            return WriteResult::ShapeDefinitionError;
        };
        if self.shape_keys.len() - key_start >= key_count {
            return WriteResult::ShapeDefinitionError;
        }

        let table_id = self
            .interned_string_table_ids
            .get(id)
            .copied()
            .flatten()
            .or_else(|| {
                let bytes = self.string_interner.get(id);
                self.string_table
                    .iter()
                    .position(|&existing_id| self.string_interner.get(existing_id) == bytes)
                    .map(|table_id| table_id as u32)
            })
            .unwrap_or_else(|| {
                let table_id = self.string_table.len() as u32;
                self.string_table.push(id);
                table_id
            });

        if self.interned_string_table_ids.len() <= id {
            self.interned_string_table_ids.resize(id + 1, None);
        }
        self.interned_string_table_ids[id] = Some(table_id);
        self.shape_keys.push(table_id);
        WriteResult::Ok
    }

    fn finish_shape_definition(&mut self) -> (WriteResult, usize) {
        let Some(&(key_count, key_start)) = self.open_shape_definition.as_ref() else {
            return (WriteResult::ShapeDefinitionError, 0);
        };
        if self.shape_keys.len() - key_start != key_count {
            return (WriteResult::ShapeDefinitionError, 0);
        }

        let shape_id = self.shapes.len();
        self.shapes.push(key_start..self.shape_keys.len());
        self.open_shape_definition = None;
        (WriteResult::Ok, shape_id)
    }

    fn start_shaped_object(&mut self, shape_id: usize) -> WriteResult {
        let Some(shape) = self.shapes.get(shape_id) else {
            return WriteResult::InvalidShapeId;
        };
        let key_count = shape.len();
        let result = self
            .write_state
            .start_shape(key_count, &mut self.write_parent_state_stack);
        if result != WriteResult::Ok {
            return result;
        }

        self.output_bytes.push(SEQSHAPE);
        fbf::varint::write(&mut self.output_bytes, shape_id as u64);
        WriteResult::Ok
    }

    fn finish_shaped_object(&mut self) -> WriteResult {
        self.write_state
            .finish_shape(&mut self.write_parent_state_stack)
    }

    fn write_interned_utf8_str(&mut self, id: InternedStringId) -> WriteResult {
        let result = self.write_state.write_string();
        if result != WriteResult::Ok {
            return result;
        }

        if let Some(table_id) = self.interned_string_table_ids.get(id).copied().flatten() {
            append_string_reference(&mut self.output_bytes, table_id);
            return WriteResult::Ok;
        }

        let string_data = self.string_interner.get(id);
        let len = string_data.len();
        let ptr = string_data.as_ptr();
        let output_ptr = self.allocate_utf8_str_unchecked(len);
        unsafe { std::ptr::copy_nonoverlapping(ptr, output_ptr as *mut u8, len) };
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
        id: InternedStringId,
    ) -> WriteResult {
        Context::with_mut(|context| {
            context.write_interned_utf8_str(id)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_shape_define_new(key_count: usize) -> WriteResult {
        Context::with_mut(|context| {
            context.start_shape_definition(key_count)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_shape_define_key(id: InternedStringId) -> WriteResult {
        Context::with_mut(|context| {
            context.define_shape_key(id)
        })
    }
}

decorate_for_target! {
    /// The most significant `usize` is the result, and the least significant `usize` is the shape ID.
    fn shopify_function_output_shape_define_finish() -> DoubleUsize {
        Context::with_mut(|context| {
            let (result, shape_id) = context.finish_shape_definition();
            ((result as DoubleUsize) << usize::BITS) | shape_id as DoubleUsize
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_new_shaped_object(shape_id: usize) -> WriteResult {
        Context::with_mut(|context| {
            context.start_shaped_object(shape_id)
        })
    }
}

decorate_for_target! {
    fn shopify_function_output_finish_shaped_object() -> WriteResult {
        Context::with_mut(|context| {
            context.finish_shaped_object()
        })
    }
}

#[cfg(not(target_family = "wasm"))]
pub fn shopify_function_output_finalize_and_return_bytes() -> (WriteResult, Vec<u8>) {
    Context::with_mut(|context| {
        if context.write_state != State::End {
            return (WriteResult::ValueNotFinished, Vec::new());
        }
        if context.open_shape_definition.is_some() {
            return (WriteResult::ShapeDefinitionError, Vec::new());
        }
        (WriteResult::Ok, context.assemble_output_payload())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_to_json(context: &Context) -> serde_json::Value {
        let bytes = context.assemble_output_payload();
        fbf::from_slice::<serde_json::Value>(&bytes).unwrap()
    }

    fn write_key(context: &mut Context, key: &str) -> WriteResult {
        let (result, ptr) = context.allocate_utf8_str(key.len());
        if result != WriteResult::Ok {
            return result;
        }
        unsafe { std::ptr::copy_nonoverlapping(key.as_ptr(), ptr as *mut u8, key.len()) };
        WriteResult::Ok
    }

    fn intern_string(context: &mut Context, value: &str) -> InternedStringId {
        let (id, ptr) = context.string_interner.preallocate(value.len());
        unsafe {
            std::ptr::copy_nonoverlapping(value.as_ptr(), ptr as *mut u8, value.len());
        }
        id
    }

    fn define_shape(context: &mut Context, keys: &[InternedStringId]) -> usize {
        assert_eq!(context.start_shape_definition(keys.len()), WriteResult::Ok);
        for &key in keys {
            assert_eq!(context.define_shape_key(key), WriteResult::Ok);
        }
        let (result, shape_id) = context.finish_shape_definition();
        assert_eq!(result, WriteResult::Ok);
        shape_id
    }

    #[test]
    fn test_write_context_bool() {
        let mut context = Context::new(Vec::new());
        context.write_bool(true);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_bool(true), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(&context);
        assert_eq!(json, serde_json::json!(true));
    }

    #[test]
    fn test_write_context_null() {
        let mut context = Context::new(Vec::new());
        context.write_nil();
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_nil(), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(&context);
        assert_eq!(json, serde_json::json!(null));
    }

    #[test]
    fn test_write_context_i32() {
        let mut context = Context::new(Vec::new());
        context.write_i32(42);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_i32(42), WriteResult::ValueAlreadyWritten);
        let json = bytes_to_json(&context);
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn test_i32_uses_smallest_fbf_encoding() {
        let cases: &[(i32, &[u8])] = &[
            (0, &[0x00]),
            (127, &[0x7f]),
            (-1, &[0xff]),
            (-32, &[0xe0]),
            (128, &[UINT8, 0x80]),
            (255, &[UINT8, 0xff]),
            (256, &[UINT16, 0x00, 0x01]),
            (65535, &[UINT16, 0xff, 0xff]),
            (65536, &[UINT32, 0x00, 0x00, 0x01, 0x00]),
            (i32::MAX, &[UINT32, 0xff, 0xff, 0xff, 0x7f]),
            (-33, &[INT8, 0xdf]),
            (-128, &[INT8, 0x80]),
            (-129, &[INT16, 0x7f, 0xff]),
            (-32768, &[INT16, 0x00, 0x80]),
            (-32769, &[INT32, 0xff, 0x7f, 0xff, 0xff]),
            (i32::MIN, &[INT32, 0x00, 0x00, 0x00, 0x80]),
        ];

        for &(value, expected) in cases {
            let mut context = Context::new(Vec::new());
            assert_eq!(context.write_i32(value), WriteResult::Ok);
            assert_eq!(context.output_bytes, expected, "value {value}");
            assert_eq!(bytes_to_json(&context), serde_json::json!(value));
        }
    }

    #[test]
    fn test_write_context_f64() {
        let mut context = Context::new(Vec::new());
        context.write_f64(42.0);
        assert_eq!(context.write_state, State::End);
        assert_eq!(context.write_f64(42.0), WriteResult::ValueAlreadyWritten);
        let mut expected = vec![FLOAT64];
        expected.extend_from_slice(&42.0_f64.to_le_bytes());
        assert_eq!(context.output_bytes, expected);
        let json = bytes_to_json(&context);
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
        let json = bytes_to_json(&context);
        assert_eq!(json, serde_json::json!(s));
    }

    #[test]
    fn test_utf8_string_fbf_length_encodings() {
        let cases: &[(usize, &[u8])] = &[
            (0, &[FIXSTR_MIN]),
            (31, &[FIXSTR_MIN + 31]),
            (32, &[STR8, 32]),
            (255, &[STR8, 255]),
            (256, &[STR16, 0x00, 0x01]),
            (65535, &[STR16, 0xff, 0xff]),
            (65536, &[STR32, 0x00, 0x00, 0x01, 0x00]),
        ];

        for &(len, expected_prefix) in cases {
            let mut context = Context::new(Vec::new());
            let (result, ptr) = context.allocate_utf8_str(len);
            assert_eq!(result, WriteResult::Ok);
            assert!(!ptr.is_null());
            assert!(context.output_bytes.starts_with(expected_prefix));
            assert_eq!(
                context.output_bytes.len(),
                expected_prefix.len() + len,
                "length {len}"
            );
            assert_eq!(
                bytes_to_json(&context).as_str().unwrap().len(),
                len,
                "length {len}"
            );
        }
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
        let json = bytes_to_json(&context);
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
        let json = bytes_to_json(&context);
        assert_eq!(json, serde_json::json!([true, []]));
    }

    #[test]
    fn test_array_and_map_use_sequential_fbf_framing() {
        let array_cases: &[(usize, &[u8])] = &[
            (0, &[FIXARRAY0]),
            (1, &[SEQFIXARRAY_MIN]),
            (7, &[SEQFIXARRAY_MIN + 6]),
            (8, &[SEQARRAY, 8]),
            (300, &[SEQARRAY, 0xac, 0x02]),
        ];
        for &(len, expected) in array_cases {
            let mut context = Context::new(Vec::new());
            assert_eq!(context.start_array(len), WriteResult::Ok);
            assert_eq!(context.output_bytes, expected, "array length {len}");
        }

        let map_cases: &[(usize, &[u8])] = &[
            (0, &[FIXMAP0]),
            (1, &[SEQFIXMAP_MIN]),
            (7, &[SEQFIXMAP_MIN + 6]),
            (8, &[SEQMAP, 8]),
            (300, &[SEQMAP, 0xac, 0x02]),
        ];
        for &(len, expected) in map_cases {
            let mut context = Context::new(Vec::new());
            assert_eq!(context.start_object(len), WriteResult::Ok);
            assert_eq!(context.output_bytes, expected, "map length {len}");
        }
    }

    #[test]
    fn test_write_shaped_object() {
        let mut context = Context::new(Vec::new());
        let x = intern_string(&mut context, "x");
        let y = intern_string(&mut context, "y");
        let shape_id = define_shape(&mut context, &[x, y]);
        assert_eq!(shape_id, 0);

        assert_eq!(context.start_shaped_object(shape_id), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(context.write_i32(2), WriteResult::Ok);
        assert_eq!(context.finish_shaped_object(), WriteResult::Ok);

        assert_eq!(context.output_bytes, vec![SEQSHAPE, 0, 1, 2]);
        assert_eq!(
            bytes_to_json(&context),
            serde_json::json!({ "x": 1, "y": 2 })
        );
    }

    #[test]
    fn test_write_shaped_objects_nested_in_array_and_object() {
        let mut context = Context::new(Vec::new());
        let x = intern_string(&mut context, "x");
        let y = intern_string(&mut context, "y");
        let shape_id = define_shape(&mut context, &[x, y]);

        assert_eq!(context.start_object(2), WriteResult::Ok);
        assert_eq!(write_key(&mut context, "items"), WriteResult::Ok);
        assert_eq!(context.start_array(1), WriteResult::Ok);
        assert_eq!(context.start_shaped_object(shape_id), WriteResult::Ok);
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(context.write_i32(2), WriteResult::Ok);
        assert_eq!(context.finish_shaped_object(), WriteResult::Ok);
        assert_eq!(context.finish_array(), WriteResult::Ok);

        assert_eq!(write_key(&mut context, "point"), WriteResult::Ok);
        assert_eq!(context.start_shaped_object(shape_id), WriteResult::Ok);
        assert_eq!(context.write_i32(3), WriteResult::Ok);
        assert_eq!(context.write_i32(4), WriteResult::Ok);
        assert_eq!(context.finish_shaped_object(), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);

        assert_eq!(
            bytes_to_json(&context),
            serde_json::json!({
                "items": [{ "x": 1, "y": 2 }],
                "point": { "x": 3, "y": 4 }
            })
        );
    }

    #[test]
    fn test_shaped_object_and_definition_errors() {
        let mut context = Context::new(Vec::new());
        assert_eq!(context.finish_shaped_object(), WriteResult::NotAShape);
        assert_eq!(context.start_shaped_object(0), WriteResult::InvalidShapeId);
        assert_eq!(context.write_state, State::Start);
        assert!(context.output_bytes.is_empty());

        let x = intern_string(&mut context, "x");
        let y = intern_string(&mut context, "y");
        assert_eq!(
            context.define_shape_key(x),
            WriteResult::ShapeDefinitionError
        );
        assert_eq!(
            context.finish_shape_definition(),
            (WriteResult::ShapeDefinitionError, 0)
        );
        assert_eq!(context.start_shape_definition(1), WriteResult::Ok);
        assert_eq!(
            context.start_shape_definition(1),
            WriteResult::ShapeDefinitionError
        );
        assert_eq!(
            context.finish_shape_definition(),
            (WriteResult::ShapeDefinitionError, 0)
        );
        assert_eq!(context.define_shape_key(x), WriteResult::Ok);
        assert_eq!(
            context.define_shape_key(y),
            WriteResult::ShapeDefinitionError
        );
        assert_eq!(context.finish_shape_definition(), (WriteResult::Ok, 0));

        assert_eq!(context.start_shaped_object(0), WriteResult::Ok);
        assert_eq!(
            context.finish_shaped_object(),
            WriteResult::ShapeLengthError
        );
        assert_eq!(context.write_i32(1), WriteResult::Ok);
        assert_eq!(context.finish_shaped_object(), WriteResult::Ok);
        assert_eq!(context.finish_shaped_object(), WriteResult::NotAShape);
    }

    #[test]
    fn test_finalize_returns_full_payload_and_rejects_open_shape_definition() {
        crate::initialize_from_fbf_bytes(Vec::new());
        let (result, bytes) = shopify_function_output_finalize_and_return_bytes();
        assert_eq!(result, WriteResult::ValueNotFinished);
        assert!(bytes.is_empty());

        assert_eq!(shopify_function_output_new_null(), WriteResult::Ok);
        assert_eq!(shopify_function_output_shape_define_new(0), WriteResult::Ok);
        let (result, bytes) = shopify_function_output_finalize_and_return_bytes();
        assert_eq!(result, WriteResult::ShapeDefinitionError);
        assert!(bytes.is_empty());

        let packed = shopify_function_output_shape_define_finish();
        assert_eq!((packed >> usize::BITS) as usize, WriteResult::Ok as usize);
        assert_eq!(packed as usize, 0);
        let (result, bytes) = shopify_function_output_finalize_and_return_bytes();
        assert_eq!(result, WriteResult::Ok);
        assert_eq!(
            fbf::from_slice::<serde_json::Value>(&bytes).unwrap(),
            serde_json::Value::Null
        );
    }

    #[test]
    fn test_shape_key_interning_writes_string_reference_in_map() {
        let mut context = Context::new(Vec::new());
        let x = intern_string(&mut context, "x");
        define_shape(&mut context, &[x]);

        assert_eq!(context.start_object(1), WriteResult::Ok);
        assert_eq!(context.write_interned_utf8_str(x), WriteResult::Ok);
        assert_eq!(context.write_i32(42), WriteResult::Ok);
        assert_eq!(context.finish_object(), WriteResult::Ok);

        assert!(context.output_bytes.contains(&0x99));
        assert_eq!(bytes_to_json(&context), serde_json::json!({ "x": 42 }));
    }
}
