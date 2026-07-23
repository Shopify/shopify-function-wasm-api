use crate::{decorate_for_target, Context};
use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, Val, ValueRef as NanBoxValueRef},
    InternedStringId,
};

pub(crate) mod nav;

use nav::{
    decode_value, get_key_at_index, lookup_property, memoize_container, navigate_to_child,
    ValueType,
};

// Return reusable provider memory for a property name copied by the
// trampoline. This is an internal provider/trampoline ABI, not a guest import.
decorate_for_target! {
    fn shopify_function_input_get_obj_prop_buffer(len: usize) -> usize {
        Context::with_mut(|context| {
            let buffer = &mut context.input_obj_prop_buffer;
            if len > buffer.capacity() {
                buffer.reserve(len - buffer.len());
            }
            // The trampoline initializes all `len` bytes before the lookup.
            unsafe { buffer.set_len(len) };
            buffer.as_mut_ptr() as usize
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get() -> Val {
        Context::with_mut(|context| {
            if context.input_root.is_none() {
                let mut reader = fbf::read::Reader::new(&context.input_bytes);
                let header = match fbf::read::parse_header(&mut reader) {
                    Ok(header) => header,
                    Err(_) => return NanBox::error(ErrorCode::ReadError).to_bits(),
                };
                let tables = match fbf::read::parse_prelude(&mut reader, header) {
                    Ok(tables) => tables,
                    Err(_) => return NanBox::error(ErrorCode::ReadError).to_bits(),
                };
                context.input_root = Some((tables, reader.pos));
            }

            let Context {
                input_root,
                input_bytes,
                long_string_lens,
                cursor_memo,
                ..
            } = &mut *context;
            let (tables, root_pos) = input_root.as_ref().unwrap();

            match decode_value(input_bytes, tables, *root_pos, input_bytes.len(), long_string_lens) {
                Ok((vtype, _)) => {
                    memoize_container(vtype, input_bytes.len(), cursor_memo);
                    encode_value_type(vtype, *root_pos).to_bits()
                }
                Err(e) => NanBox::error(e).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_obj_prop(
        scope: Val,
        ptr: usize,
        len: usize,
    ) -> Val {
        Context::with_mut(|context| {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Object { ptr: obj_offset, .. }) => {
                    let query = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
                    let Context {
                        input_root,
                        input_bytes,
                        cursor_memo,
                        long_string_lens,
                        ..
                    } = context;
                    let (tables, _) = match input_root.as_ref() {
                        Some(root) => (&root.0, root.1),
                        None => return NanBox::error(ErrorCode::ReadError).to_bits(),
                    };

                    match lookup_property(
                        input_bytes,
                        tables,
                        obj_offset,
                        query,
                        input_bytes.len(),
                        cursor_memo,
                        long_string_lens,
                    ) {
                        Ok(Some(value_offset)) => match decode_value(
                            input_bytes,
                            tables,
                            value_offset,
                            input_bytes.len(),
                            long_string_lens,
                        ) {
                            Ok((vtype, _)) => {
                                memoize_container(vtype, input_bytes.len(), cursor_memo);
                                encode_value_type(vtype, value_offset).to_bits()
                            }
                            Err(e) => NanBox::error(e).to_bits(),
                        },
                        Ok(None) => NanBox::null().to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
                Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_interned_obj_prop(
        scope: Val,
        interned_string_id: InternedStringId,
    ) -> Val {
        Context::with_mut(|context| {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Object { ptr: obj_offset, .. }) => {
                    let Context {
                        input_root,
                        input_bytes,
                        cursor_memo,
                        long_string_lens,
                        string_interner,
                        ..
                    } = context;
                    let (tables, _) = match input_root.as_ref() {
                        Some(root) => (&root.0, root.1),
                        None => return NanBox::error(ErrorCode::ReadError).to_bits(),
                    };
                    let query = string_interner.get(interned_string_id);

                    match lookup_property(
                        input_bytes,
                        tables,
                        obj_offset,
                        query,
                        input_bytes.len(),
                        cursor_memo,
                        long_string_lens,
                    ) {
                        Ok(Some(value_offset)) => match decode_value(
                            input_bytes,
                            tables,
                            value_offset,
                            input_bytes.len(),
                            long_string_lens,
                        ) {
                            Ok((vtype, _)) => {
                                memoize_container(vtype, input_bytes.len(), cursor_memo);
                                encode_value_type(vtype, value_offset).to_bits()
                            }
                            Err(e) => NanBox::error(e).to_bits(),
                        },
                        Ok(None) => NanBox::null().to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
                Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_at_index(
        scope: Val,
        index: usize,
    ) -> Val {
        Context::with_mut(|context| {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(
                    NanBoxValueRef::Array {
                        ptr: container_offset,
                        ..
                    }
                    | NanBoxValueRef::Object {
                        ptr: container_offset,
                        ..
                    },
                ) => {
                    let Context {
                        input_root,
                        input_bytes,
                        cursor_memo,
                        long_string_lens,
                        ..
                    } = context;
                    let (tables, _) = match input_root.as_ref() {
                        Some(root) => (&root.0, root.1),
                        None => return NanBox::error(ErrorCode::ReadError).to_bits(),
                    };

                    match navigate_to_child(
                        input_bytes,
                        tables,
                        container_offset,
                        index,
                        input_bytes.len(),
                        cursor_memo,
                        long_string_lens,
                    ) {
                        Ok(child_offset) => match decode_value(
                            input_bytes,
                            tables,
                            child_offset,
                            input_bytes.len(),
                            long_string_lens,
                        ) {
                            Ok((vtype, _)) => {
                                memoize_container(vtype, input_bytes.len(), cursor_memo);
                                encode_value_type(vtype, child_offset).to_bits()
                            }
                            Err(e) => NanBox::error(e).to_bits(),
                        },
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotIndexable).to_bits(),
                Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_obj_key_at_index(
        scope: Val,
        index: usize,
    ) -> Val {
        Context::with_mut(|context| {
            let v = NanBox::from_bits(scope);
            match v.try_decode() {
                Ok(NanBoxValueRef::Object { ptr: obj_offset, .. }) => {
                    let Context {
                        input_root,
                        input_bytes,
                        long_string_lens,
                        ..
                    } = context;
                    let (tables, _) = match input_root.as_ref() {
                        Some(root) => (&root.0, root.1),
                        None => return NanBox::error(ErrorCode::ReadError).to_bits(),
                    };

                    match get_key_at_index(
                        input_bytes,
                        tables,
                        obj_offset,
                        index,
                        input_bytes.len(),
                        long_string_lens,
                    ) {
                        Ok((ptr, len)) => NanBox::string(ptr, len).to_bits(),
                        Err(e) => NanBox::error(e).to_bits(),
                    }
                }
                Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
                Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_val_len(scope: Val) -> usize {
        let v = NanBox::from_bits(scope);
        match v.try_decode() {
            Ok(NanBoxValueRef::String { ptr, len }) => {
                if len < NanBox::MAX_VALUE_LENGTH {
                    len
                } else {
                    Context::with(|context| {
                        context.long_string_lens.get(ptr).unwrap_or(usize::MAX)
                    })
                }
            }
            Ok(
                NanBoxValueRef::Array { ptr: offset, .. }
                | NanBoxValueRef::Object { ptr: offset, .. },
            ) => Context::with_mut(|context| {
                let Context {
                    input_root,
                    input_bytes,
                    long_string_lens,
                    ..
                } = context;
                let (tables, _) = match input_root.as_ref() {
                    Some(root) => (&root.0, root.1),
                    None => return usize::MAX,
                };

                match decode_value(
                    input_bytes,
                    tables,
                    offset,
                    input_bytes.len(),
                    long_string_lens,
                ) {
                    Ok((vtype, _)) => get_value_length(&vtype),
                    Err(_) => usize::MAX,
                }
            }),
            _ => usize::MAX,
        }
    }
}

decorate_for_target! {
    fn shopify_function_input_get_utf8_str_addr(
        ptr: usize,
    ) -> usize {
        Context::with(|context| {
            context
                .input_bytes
                .get(ptr..)
                .map_or(0, |bytes| bytes.as_ptr() as usize)
        })
    }
}

/// Encode a ValueType into a NanBox, using the offset as the pointer.
fn encode_value_type(vtype: ValueType, offset: usize) -> NanBox {
    match vtype {
        ValueType::Null => NanBox::null(),
        ValueType::Bool(b) => NanBox::bool(b),
        ValueType::Number(n) => NanBox::number(n),
        ValueType::String { ptr, len } => NanBox::string(ptr, len),
        ValueType::Array { count, .. } => NanBox::array(offset, count),
        ValueType::Map { count, .. } | ValueType::Shape { count, .. } => NanBox::obj(offset, count),
    }
}

/// Get the length of a value (string byte length, array/object element count).
fn get_value_length(vtype: &ValueType) -> usize {
    match vtype {
        ValueType::String { len, .. } => *len,
        ValueType::Array { count, .. }
        | ValueType::Map { count, .. }
        | ValueType::Shape { count, .. } => *count,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn initialize_test_context(json_value: &serde_json::Value) -> Vec<u8> {
        let bytes = fbf::to_vec(json_value).unwrap();
        Context::with_mut(|context| {
            *context = Context::default();
            context.input_bytes = bytes.clone();
            context.input_root = None;
        });
        bytes
    }

    fn initialize_optimized_context(json_value: &serde_json::Value) -> Vec<u8> {
        let bytes = fbf::to_vec_optimized(json_value).unwrap();
        Context::with_mut(|context| {
            *context = Context::default();
            context.input_bytes = bytes.clone();
            context.input_root = None;
        });
        bytes
    }

    #[test]
    fn test_input_get_scalars() {
        for (input, expected) in [
            (json!(null), NanBox::null()),
            (json!(false), NanBox::bool(false)),
            (json!(true), NanBox::bool(true)),
            (json!(42), NanBox::number(42.0)),
            (json!(-17), NanBox::number(-17.0)),
            (json!(1.5), NanBox::number(1.5)),
        ] {
            initialize_test_context(&input);
            let result = shopify_function_input_get();
            assert_eq!(NanBox::from_bits(result), expected);
        }
    }

    #[test]
    fn test_input_get_string() {
        initialize_test_context(&json!("hello"));
        let result = shopify_function_input_get();
        let decoded = NanBox::from_bits(result).try_decode().unwrap();
        match decoded {
            NanBoxValueRef::String { ptr, len } => {
                assert_eq!(len, 5);
                let addr = shopify_function_input_get_utf8_str_addr(ptr);
                let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
                assert_eq!(bytes, b"hello");
            }
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn test_input_get_array() {
        initialize_test_context(&json!([1, 2, 3]));
        let root = shopify_function_input_get();
        let decoded = NanBox::from_bits(root).try_decode().unwrap();

        match decoded {
            NanBoxValueRef::Array { len, .. } => {
                assert_eq!(len, 3);
                assert_eq!(shopify_function_input_get_val_len(root), 3);

                for i in 0..3 {
                    let elem = shopify_function_input_get_at_index(root, i);
                    let val = NanBox::from_bits(elem).try_decode().unwrap();
                    assert_eq!(val, NanBoxValueRef::Number((i + 1) as f64));
                }

                // Out of bounds
                let result = shopify_function_input_get_at_index(root, 3);
                let decoded = NanBox::from_bits(result).try_decode().unwrap();
                assert_eq!(decoded, NanBoxValueRef::Error(ErrorCode::IndexOutOfBounds));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_input_get_object() {
        initialize_test_context(&json!({"a": 1, "b": "two", "c": true}));
        let root = shopify_function_input_get();
        let decoded = NanBox::from_bits(root).try_decode().unwrap();

        match decoded {
            NanBoxValueRef::Object { len, .. } => {
                assert_eq!(len, 3);

                // Get by property
                let val = shopify_function_input_get_obj_prop(root, b"a".as_ptr() as usize, 1);
                assert_eq!(
                    NanBox::from_bits(val).try_decode().unwrap(),
                    NanBoxValueRef::Number(1.0)
                );

                let val = shopify_function_input_get_obj_prop(root, b"b".as_ptr() as usize, 1);
                match NanBox::from_bits(val).try_decode().unwrap() {
                    NanBoxValueRef::String { ptr, len } => {
                        let addr = shopify_function_input_get_utf8_str_addr(ptr);
                        let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
                        assert_eq!(bytes, b"two");
                    }
                    _ => panic!("expected string"),
                }

                let val = shopify_function_input_get_obj_prop(root, b"c".as_ptr() as usize, 1);
                assert_eq!(
                    NanBox::from_bits(val).try_decode().unwrap(),
                    NanBoxValueRef::Bool(true)
                );

                // Missing property
                let val =
                    shopify_function_input_get_obj_prop(root, b"missing".as_ptr() as usize, 7);
                assert_eq!(NanBox::from_bits(val), NanBox::null());

                // Objects expose values, not alternating key/value slots, by index.
                let val = shopify_function_input_get_at_index(root, 1);
                match NanBox::from_bits(val).try_decode().unwrap() {
                    NanBoxValueRef::String { ptr, len } => {
                        let addr = shopify_function_input_get_utf8_str_addr(ptr);
                        let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
                        assert_eq!(bytes, b"two");
                    }
                    _ => panic!("expected string"),
                }
                let val = shopify_function_input_get_at_index(root, 2);
                assert_eq!(
                    NanBox::from_bits(val).try_decode().unwrap(),
                    NanBoxValueRef::Bool(true)
                );

                // Get key by index
                let key = shopify_function_input_get_obj_key_at_index(root, 1);
                match NanBox::from_bits(key).try_decode().unwrap() {
                    NanBoxValueRef::String { ptr, len } => {
                        let addr = shopify_function_input_get_utf8_str_addr(ptr);
                        let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
                        assert_eq!(bytes, b"b");
                    }
                    _ => panic!("expected string"),
                }
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn test_shaped_objects() {
        initialize_optimized_context(&json!([
            {"x": 10, "y": 20},
            {"x": 30, "y": 40}
        ]));

        let root = shopify_function_input_get();
        let first = shopify_function_input_get_at_index(root, 0);

        // Access shape properties
        let x = shopify_function_input_get_obj_prop(first, b"x".as_ptr() as usize, 1);
        assert_eq!(
            NanBox::from_bits(x).try_decode().unwrap(),
            NanBoxValueRef::Number(10.0)
        );

        let y = shopify_function_input_get_obj_prop(first, b"y".as_ptr() as usize, 1);
        assert_eq!(
            NanBox::from_bits(y).try_decode().unwrap(),
            NanBoxValueRef::Number(20.0)
        );

        // Access by index
        let val0 = shopify_function_input_get_at_index(first, 0);
        assert_eq!(
            NanBox::from_bits(val0).try_decode().unwrap(),
            NanBoxValueRef::Number(10.0)
        );

        // Get key by index
        let key0 = shopify_function_input_get_obj_key_at_index(first, 0);
        match NanBox::from_bits(key0).try_decode().unwrap() {
            NanBoxValueRef::String { ptr, len } => {
                let addr = shopify_function_input_get_utf8_str_addr(ptr);
                let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
                assert_eq!(bytes, b"x");
            }
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn test_nested_navigation() {
        initialize_test_context(&json!({
            "outer": {
                "inner": [1, 2, 3]
            }
        }));

        let root = shopify_function_input_get();
        let outer = shopify_function_input_get_obj_prop(root, b"outer".as_ptr() as usize, 5);
        let inner = shopify_function_input_get_obj_prop(outer, b"inner".as_ptr() as usize, 5);
        let elem1 = shopify_function_input_get_at_index(inner, 1);

        assert_eq!(
            NanBox::from_bits(elem1).try_decode().unwrap(),
            NanBoxValueRef::Number(2.0)
        );
    }

    #[test]
    fn test_sequential_containers() {
        let bytes = fbf::to_vec_with(
            &json!([1, 2, 3]),
            fbf::EncodeOptions {
                sequential_containers: true,
                ..Default::default()
            },
        )
        .unwrap();

        Context::with_mut(|context| {
            *context = Context::default();
            context.input_bytes = bytes;
            context.input_root = None;
        });

        let root = shopify_function_input_get();
        let elem0 = shopify_function_input_get_at_index(root, 0);
        let elem2 = shopify_function_input_get_at_index(root, 2);

        assert_eq!(
            NanBox::from_bits(elem0).try_decode().unwrap(),
            NanBoxValueRef::Number(1.0)
        );
        assert_eq!(
            NanBox::from_bits(elem2).try_decode().unwrap(),
            NanBoxValueRef::Number(3.0)
        );
    }

    #[test]
    fn test_string_references() {
        let bytes = fbf::to_vec_optimized(&json!({
            "key1": "value",
            "key2": "value",
            "key3": "value"
        }))
        .unwrap();

        Context::with_mut(|context| {
            *context = Context::default();
            context.input_bytes = bytes;
            context.input_root = None;
        });

        let root = shopify_function_input_get();
        let val1 = shopify_function_input_get_obj_prop(root, b"key1".as_ptr() as usize, 4);
        let val2 = shopify_function_input_get_obj_prop(root, b"key2".as_ptr() as usize, 4);

        // Both should resolve to the same string
        match (
            NanBox::from_bits(val1).try_decode().unwrap(),
            NanBox::from_bits(val2).try_decode().unwrap(),
        ) {
            (
                NanBoxValueRef::String { ptr: p1, len: l1 },
                NanBoxValueRef::String { ptr: p2, len: l2 },
            ) => {
                assert_eq!(l1, l2);
                let addr1 = shopify_function_input_get_utf8_str_addr(p1);
                let addr2 = shopify_function_input_get_utf8_str_addr(p2);
                let bytes1 = unsafe { std::slice::from_raw_parts(addr1 as *const u8, l1) };
                let bytes2 = unsafe { std::slice::from_raw_parts(addr2 as *const u8, l2) };
                assert_eq!(bytes1, bytes2);
                assert_eq!(bytes1, b"value");
            }
            _ => panic!("expected strings"),
        }
    }

    #[test]
    fn test_malformed_input() {
        // Truncated payload
        let mut bytes = Vec::from(fbf::format::MAGIC);
        bytes.push(fbf::format::VERSION);
        bytes.push(0);
        bytes.push(0xd3); // fixarray3
                          // Missing length and elements

        Context::with_mut(|context| {
            *context = Context::default();
            context.input_bytes = bytes;
            context.input_root = None;
        });

        let result = shopify_function_input_get();
        let decoded = NanBox::from_bits(result).try_decode().unwrap();
        assert!(matches!(decoded, NanBoxValueRef::Error(_)));
    }

    #[test]
    fn test_interned_string_lookup() {
        initialize_test_context(&json!({"test": 123}));

        let key_id = Context::with_mut(|context| {
            let (id, ptr) = context.string_interner.preallocate(4);
            unsafe {
                std::ptr::copy_nonoverlapping(b"test".as_ptr(), ptr as *mut u8, 4);
            }
            id
        });

        let root = shopify_function_input_get();
        let val = shopify_function_input_get_interned_obj_prop(root, key_id);
        assert_eq!(
            NanBox::from_bits(val).try_decode().unwrap(),
            NanBoxValueRef::Number(123.0)
        );
    }

    #[test]
    fn test_cursor_memo_optimization() {
        // Large array to test cursor caching
        let large_array: Vec<_> = (0..100).collect();
        initialize_test_context(&json!(large_array));

        let root = shopify_function_input_get();

        // Access elements in order - should build up cursor cache
        for i in 0..10 {
            let elem = shopify_function_input_get_at_index(root, i);
            assert_eq!(
                NanBox::from_bits(elem).try_decode().unwrap(),
                NanBoxValueRef::Number(i as f64)
            );
        }

        // Access an element we've passed - should use cached position
        let elem5 = shopify_function_input_get_at_index(root, 5);
        assert_eq!(
            NanBox::from_bits(elem5).try_decode().unwrap(),
            NanBoxValueRef::Number(5.0)
        );
    }
}
