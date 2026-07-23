//! Input (read) side of the provider.
//!
use crate::{decorate_for_target, Context};
use shopify_function_wasm_api_core::{
    read::{ErrorCode, NanBox, Val, ValueRef as NanBoxValueRef},
    InternedStringId,
};

pub(crate) mod nav;

#[derive(Clone, Copy)]
enum ScopeKind {
    Object(u32),
    Array(u32),
    Other,
    BadPointer,
    Invalid,
}

#[inline(always)]
fn decode_scope(scope: Val) -> ScopeKind {
    const F64_OFFSET: u32 = Val::BITS - 64;
    const PAYLOAD_SIZE: u32 = 50 + F64_OFFSET;
    const TAG_SHIFT: u32 = 46 + F64_OFFSET;
    const NAN_MASK: Val = (((1 as Val) << 13) - 1) << PAYLOAD_SIZE;
    const POINTER_MASK: Val = ((1 as Val) << usize::BITS) - 1;

    if scope & NAN_MASK != NAN_MASK {
        return ScopeKind::Other;
    }
    let tag = ((scope >> TAG_SHIFT) & 0xf) as u8;
    if !matches!(tag, 0 | 1 | 3 | 4 | 5 | 15) {
        return ScopeKind::Invalid;
    }
    let ptr = (scope & POINTER_MASK) as usize;
    let Ok(ptr) = u32::try_from(ptr) else {
        return ScopeKind::BadPointer;
    };
    match tag {
        4 => ScopeKind::Object(ptr),
        5 => ScopeKind::Array(ptr),
        _ => ScopeKind::Other,
    }
}

decorate_for_target! {
    fn shopify_function_input_get_obj_prop_buffer(len: usize) -> usize {
        Context::with_mut(|context| {
            if len <= context.input_obj_prop_buffer.len() {
                return context.input_obj_prop_buffer.as_mut_ptr() as usize;
            }
            let buffer = &mut context.input_obj_prop_overflow;
            if len > buffer.capacity() {
                buffer.reserve(len);
            }
            buffer.as_mut_ptr() as usize
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get() -> Val {
        Context::with_mut(|context| {
            if context.input_state.is_none() {
                let state = match nav::parse_input(&context.input_bytes) {
                    Ok(state) => state,
                    Err(_) => return NanBox::error(ErrorCode::ReadError).to_bits(),
                };
                nav::reset_caches_for_state(&mut context.input_caches, &state);
                context.input_state = Some(state);
            }
            let state = context.input_state.as_ref().unwrap();
            nav::decode_value(
                &context.input_bytes,
                state,
                &mut context.input_caches,
                state.root,
            )
            .unwrap_or_else(|_| NanBox::error(ErrorCode::ReadError))
            .to_bits()
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_obj_prop(
        scope: Val,
        ptr: usize,
        len: usize,
    ) -> Val {
        let object = match decode_scope(scope) {
            ScopeKind::Object(ptr) => ptr,
            ScopeKind::BadPointer => return NanBox::error(ErrorCode::ReadError).to_bits(),
            ScopeKind::Invalid => return NanBox::error(ErrorCode::DecodeError).to_bits(),
            _ => return NanBox::error(ErrorCode::NotAnObject).to_bits(),
        };
        let query = if len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }
        };
        Context::with_mut(|context| {
            let Some(state) = context.input_state.as_ref() else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            nav::find_property(
                &context.input_bytes,
                state,
                &mut context.input_caches,
                object,
                query,
            )
            .to_bits()
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_interned_obj_prop(
        scope: Val,
        interned_string_id: InternedStringId,
    ) -> Val {
        let object = match decode_scope(scope) {
            ScopeKind::Object(ptr) => ptr,
            ScopeKind::BadPointer => return NanBox::error(ErrorCode::ReadError).to_bits(),
            ScopeKind::Invalid => return NanBox::error(ErrorCode::DecodeError).to_bits(),
            _ => return NanBox::error(ErrorCode::NotAnObject).to_bits(),
        };
        Context::with_mut(|context| {
            let Some(state) = context.input_state.as_ref() else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            let query = context.string_interner.get(interned_string_id);
            nav::find_property(
                &context.input_bytes,
                state,
                &mut context.input_caches,
                object,
                query,
            )
            .to_bits()
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_at_index(
        scope: Val,
        index: usize,
    ) -> Val {
        let container = match decode_scope(scope) {
            ScopeKind::Object(ptr) | ScopeKind::Array(ptr) => ptr,
            ScopeKind::Other => return NanBox::error(ErrorCode::NotIndexable).to_bits(),
            ScopeKind::BadPointer | ScopeKind::Invalid => {
                return NanBox::error(ErrorCode::ReadError).to_bits()
            }
        };
        let index = match u32::try_from(index) {
            Ok(index) => index,
            Err(_) => return NanBox::error(ErrorCode::IndexOutOfBounds).to_bits(),
        };
        Context::with_mut(|context| {
            let Some(state) = context.input_state.as_ref() else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            nav::element_at(
                &context.input_bytes,
                state,
                &mut context.input_caches,
                container,
                index,
                true,
            )
            .unwrap_or_else(NanBox::error)
            .to_bits()
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_obj_key_at_index(
        scope: Val,
        index: usize,
    ) -> Val {
        let object = match decode_scope(scope) {
            ScopeKind::Object(ptr) => ptr,
            ScopeKind::Other | ScopeKind::Array(_) => {
                return NanBox::error(ErrorCode::NotAnObject).to_bits()
            }
            ScopeKind::BadPointer | ScopeKind::Invalid => {
                return NanBox::error(ErrorCode::ReadError).to_bits()
            }
        };
        let index = match u32::try_from(index) {
            Ok(index) => index,
            Err(_) => return NanBox::error(ErrorCode::IndexOutOfBounds).to_bits(),
        };
        Context::with_mut(|context| {
            let Some(state) = context.input_state.as_ref() else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            match nav::key_at(
                &context.input_bytes,
                state,
                &mut context.input_caches,
                object,
                index,
            ) {
                Ok((ptr, len)) => NanBox::string(ptr as usize, len as usize).to_bits(),
                Err(error) => NanBox::error(error).to_bits(),
            }
        })
    }
}

decorate_for_target! {
    fn shopify_function_input_get_val_len(scope: Val) -> usize {
        match NanBox::from_bits(scope).try_decode() {
            Ok(NanBoxValueRef::String { ptr, len }) => {
                if len < NanBox::MAX_VALUE_LENGTH {
                    len
                } else {
                    let Ok(content) = u32::try_from(ptr) else {
                        return usize::MAX;
                    };
                    Context::with(|context| {
                        nav::long_string_len(&context.input_caches, content)
                            .map_or(usize::MAX, |len| len as usize)
                    })
                }
            }
            Ok(
                NanBoxValueRef::Array { ptr, .. }
                | NanBoxValueRef::Object { ptr, .. },
            ) => {
                let Ok(container) = u32::try_from(ptr) else {
                    return usize::MAX;
                };
                Context::with_mut(|context| {
                    let Some(state) = context.input_state.as_ref() else {
                        return usize::MAX;
                    };
                    nav::container_meta(
                        &context.input_bytes,
                        state,
                        &mut context.input_caches,
                        container,
                    )
                    .map_or(usize::MAX, |meta| meta.count as usize)
                })
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn long_string_length_and_address_bounds() {
        // The Wasm NanBox saturates at 16,383 bytes; host NanBoxes have a
        // wider length field, but this still exercises the same input span.
        let text = "x".repeat(16_400);
        let bytes = fbf::to_vec(&json!(text)).unwrap();
        let input_len = bytes.len();
        crate::initialize_from_fbf_bytes(bytes);

        let root = shopify_function_input_get();
        let NanBoxValueRef::String { ptr, len } = NanBox::from_bits(root).try_decode().unwrap()
        else {
            panic!("expected string");
        };
        assert_eq!(len, text.len().min(NanBox::MAX_VALUE_LENGTH));
        assert_eq!(shopify_function_input_get_val_len(root), text.len());

        let address = shopify_function_input_get_utf8_str_addr(ptr);
        assert_ne!(address, 0);
        let content = unsafe { std::slice::from_raw_parts(address as *const u8, text.len()) };
        assert_eq!(content, text.as_bytes());
        assert_ne!(shopify_function_input_get_utf8_str_addr(input_len), 0);
        assert_eq!(shopify_function_input_get_utf8_str_addr(input_len + 1), 0);
    }

    #[test]
    fn descending_index_falls_back_from_cursor() {
        let bytes = fbf::to_vec(&json!([0, 1, 2, 3, 4, 5, 6, 7])).unwrap();
        crate::initialize_from_fbf_bytes(bytes);
        let root = shopify_function_input_get();

        for (index, expected) in [(6, 6.0), (2, 2.0), (7, 7.0), (0, 0.0)] {
            let value = shopify_function_input_get_at_index(root, index);
            assert_eq!(
                NanBox::from_bits(value).try_decode().unwrap(),
                NanBoxValueRef::Number(expected)
            );
        }
    }

    #[test]
    fn duplicate_shape_key_lookup_returns_first_value() {
        let mut bytes = Vec::from(fbf::format::MAGIC);
        bytes.extend_from_slice(&[
            fbf::format::VERSION,
            fbf::format::FLAG_SHAPE_TABLE,
            1, // shape count
            2, // key count
            fbf::format::FIXSTR_MIN + 1,
            b'x',
            fbf::format::FIXSTR_MIN + 1,
            b'x',
            fbf::format::SHAPE8,
            3, // payload: shape id and two values
            0,
            10,
            20,
        ]);
        crate::initialize_from_fbf_bytes(bytes);
        let root = shopify_function_input_get();

        for _ in 0..2 {
            let value = shopify_function_input_get_obj_prop(root, b"x".as_ptr() as usize, 1);
            assert_eq!(
                NanBox::from_bits(value).try_decode().unwrap(),
                NanBoxValueRef::Number(10.0)
            );
        }
        assert_eq!(shopify_function_input_get_val_len(root), 2);

        let key = shopify_function_input_get_obj_key_at_index(root, 1);
        let NanBoxValueRef::String { ptr, len } = NanBox::from_bits(key).try_decode().unwrap()
        else {
            panic!("expected string key");
        };
        let address = shopify_function_input_get_utf8_str_addr(ptr);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(address as *const u8, len) },
            b"x"
        );
    }
}
