use once_cell::sync::OnceCell;
use rmpv::{decode::read_value_ref, ValueRef};
use shopify_function_wasm_api_core::{ErrorCode, NanBox, ValueRef as NanBoxValueRef};
use std::io::Read;

static BYTES: OnceCell<Vec<u8>> = OnceCell::new();
static VALUE: OnceCell<ValueRef<'static>> = OnceCell::new();

fn bytes() -> &'static [u8] {
    BYTES.get_or_init(|| {
        let mut bytes: Vec<u8> = vec![];
        let mut stdin = std::io::stdin();
        // Temporary use of stdin, to copy data into the Wasm linear memory.
        // Initial benchmarking doesn't seem to suggest that this represents
        // a source of performance overhead.
        stdin.read_to_end(&mut bytes).unwrap();

        bytes
    })
}

fn value() -> &'static ValueRef<'static> {
    VALUE.get_or_init(|| {
        let mut reader = bytes();
        read_value_ref(&mut reader).unwrap()
    })
}

#[no_mangle]
#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get() -> u64 {
    encode_value(value()).to_bits()
}

#[no_mangle]
#[export_name = "_shopify_function_input_get_obj_prop"]
extern "C" fn shopify_function_input_get_obj_prop(scope: u64, ptr: *const u8, len: usize) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
            let query = unsafe { query_from_raw_parts(ptr, len) };
            let obj_ptr = obj_ptr as *const ValueRef<'_>;
            let value = unsafe { &*obj_ptr };
            let ValueRef::Map(map) = value else {
                panic!("expected map, got {:?}", value);
            };
            let boxed = match map
                .iter()
                .find(|(k, _)| matches!(k, ValueRef::String(s) if s.as_str() == Some(query)))
            {
                Some((_, v)) => encode_value(v),
                None => NanBox::null(),
            };
            boxed.to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
        Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
    }
}

fn encode_value(value: &ValueRef<'_>) -> NanBox {
    match value {
        ValueRef::Nil => NanBox::null(),
        ValueRef::Boolean(b) => NanBox::bool(*b),
        ValueRef::Integer(n) => NanBox::number(n.as_f64().expect("integer out of range")),
        ValueRef::F32(n) => NanBox::number(f64::from(*n)),
        ValueRef::F64(n) => NanBox::number(*n),
        ValueRef::String(s) => NanBox::string(s.as_str().expect("string is not valid UTF-8")),
        ValueRef::Map(_) => {
            let ptr = value as *const _ as usize;
            NanBox::obj(ptr)
        }
        ValueRef::Array(_) => todo!("array not yet supported"),
        ValueRef::Binary(_) => todo!("binary not yet supported"),
        ValueRef::Ext(_, _) => todo!("ext not yet supported"),
    }
}

unsafe fn query_from_raw_parts(ptr: *const u8, len: usize) -> &'static str {
    let slice = std::slice::from_raw_parts(ptr, len);
    std::str::from_utf8_unchecked(slice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::encode::{self, ByteBuf};
    use rmpv::decode::read_value_ref;
    use shopify_function_wasm_api_core::ValueRef;

    fn build_msgpack<E, F: FnOnce(&mut ByteBuf) -> Result<(), E>>(
        writer_fn: F,
    ) -> Result<Vec<u8>, E> {
        let mut buf = ByteBuf::new();
        writer_fn(&mut buf)?;
        Ok(buf.into_vec())
    }

    #[test]
    fn test_encode_bool_value() {
        [true, false].iter().for_each(|&b| {
            let bytes = build_msgpack(|w| encode::write_bool(w, b)).unwrap();
            let value_ref = read_value_ref(&mut bytes.as_slice()).unwrap();
            let nanbox = encode_value(&value_ref);
            assert_eq!(nanbox, NanBox::bool(b));
        });
    }

    #[test]
    fn test_encode_null_value() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        let value_ref = read_value_ref(&mut bytes.as_slice()).unwrap();
        let nanbox = encode_value(&value_ref);
        assert_eq!(nanbox, NanBox::null());
    }

    macro_rules! test_encode_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        let value_ref = read_value_ref(&mut bytes.as_slice()).unwrap();
                        let nanbox = encode_value(&value_ref);
                        assert_eq!(nanbox, NanBox::number(n as f64));
                    });
                }
            }
        };
        ($type:ty, $encode_type:ident) => {
            test_encode_number_type!($type, $encode_type, [$type::MIN, 0 as $type, $type::MAX]);
        };
        ($type:ty) => {
            paste::paste! {
                test_encode_number_type!($type, [<$type>]);
            }
        }
    }

    test_encode_number_type!(u8, pfix, [0, 1, 127]);
    test_encode_number_type!(u8);
    test_encode_number_type!(i8);
    test_encode_number_type!(i8, nfix, [-32, -1]);
    test_encode_number_type!(u16);
    test_encode_number_type!(i16);
    test_encode_number_type!(u32);
    test_encode_number_type!(i32);
    test_encode_number_type!(u64);
    test_encode_number_type!(i64);
    test_encode_number_type!(f32);
    test_encode_number_type!(f64);

    macro_rules! test_encode_str {
        ($len:expr, $encode_type:ident, $marker_and_len_size:literal) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    let bytes = build_msgpack(|w| encode::write_str(w, "a".repeat($len).as_str())).unwrap();
                    let value_ref = read_value_ref(&mut bytes.as_slice()).unwrap();
                    let nanbox = encode_value(&value_ref);
                    let decoded = nanbox.try_decode().unwrap();
                    let ptr = bytes[$marker_and_len_size..].as_ptr() as usize & u32::MAX as usize;
                    assert_eq!(
                        decoded,
                        ValueRef::String {
                            ptr,
                            len: bytes.len() - $marker_and_len_size
                        }
                    );
                }
            }
        };
    }

    test_encode_str!(31, fixstr, 1);
    test_encode_str!(u8::MAX as usize, str8, 2);
    // TODO: enable once we have support for longer strings
    // test_encode_str!(u16::MAX as usize, str16, 3);
    // test_encode_str!(u32::MAX as usize, str32, 5);
}
