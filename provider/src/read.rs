use once_cell::sync::OnceCell;
use rmp::{
    decode::{self, read_marker, Bytes},
    Marker,
};
use shopify_function_wasm_api_core::{ErrorCode, NanBox, ValueRef as NanBoxValueRef};
use std::io::Read;

mod msgpack_utils;

static BYTES: OnceCell<Vec<u8>> = OnceCell::new();

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

#[no_mangle]
#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get() -> u64 {
    encode_value(bytes()).to_bits()
}

#[no_mangle]
#[export_name = "_shopify_function_input_get_obj_prop"]
extern "C" fn shopify_function_input_get_obj_prop(scope: u64, ptr: *const u8, len: usize) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
            let query = unsafe { query_from_raw_parts(ptr, len) };
            let Some(offset) = obj_ptr.checked_sub(bytes().as_ptr() as usize) else {
                return NanBox::error(ErrorCode::PointerOutOfBounds).to_bits();
            };
            let len = bytes().len() - offset;
            let bytes = unsafe { std::slice::from_raw_parts(obj_ptr as *const u8, len) };
            let mut reader = Bytes::new(bytes);
            let Ok(map_len) = decode::read_map_len(&mut reader) else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            for _ in 0..map_len {
                let Ok((key, remainder)) = decode::read_str_from_slice(reader.remaining_slice())
                else {
                    return NanBox::error(ErrorCode::ReadError).to_bits();
                };
                reader = Bytes::new(remainder);
                if key == query {
                    return encode_value(reader.remaining_slice()).to_bits();
                }
                let Ok(()) = msgpack_utils::skip_value(&mut reader) else {
                    return NanBox::error(ErrorCode::ReadError).to_bits();
                };
            }
            NanBox::null().to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
        Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
    }
}

#[no_mangle]
#[export_name = "_shopify_function_input_get_utf8_str_offset"]
extern "C" fn shopify_function_input_get_utf8_str_offset(ptr: usize) -> u32 {
    let byte = ptr as *const u8;
    match Marker::from_u8(unsafe { *byte }) {
        Marker::FixStr(_) | Marker::Str8 => 1,
        Marker::Str16 => 3,
        Marker::Str32 => 5,
        _ => 0,
    }
}

#[no_mangle]
#[export_name = "_shopify_function_input_get_len"]
extern "C" fn shopify_function_input_get_len(scope: u64) -> u32 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::String { ptr, .. } | NanBoxValueRef::Array { ptr, .. }) => {
            let Some(offset) = ptr.checked_sub(bytes().as_ptr() as usize) else {
                return 0;
            };
            let bytes = &bytes()[offset..];
            match Marker::from_u8(bytes[0]) {
                Marker::FixStr(len) | Marker::FixArray(len) => len as u32,
                Marker::Str8 => bytes[1] as u32,
                Marker::Str16 | Marker::Array16 => u16::from_be_bytes([bytes[1], bytes[2]]) as u32,
                Marker::Str32 | Marker::Array32 => {
                    u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]])
                }
                _ => 0,
            }
        }
        _ => 0,
    }
}

fn encode_value(bytes: &[u8]) -> NanBox {
    let mut reader = Bytes::new(bytes);
    // clone the reader because other decode functions need to read the marker again
    match read_marker(&mut reader.clone()) {
        Ok(Marker::False) => NanBox::bool(false),
        Ok(Marker::True) => NanBox::bool(true),
        Ok(Marker::Null) => NanBox::null(),
        Ok(Marker::F32) => NanBox::number(decode::read_f32(&mut reader).unwrap().into()),
        Ok(Marker::F64) => NanBox::number(decode::read_f64(&mut reader).unwrap()),
        Ok(Marker::U8) => NanBox::number(decode::read_u8(&mut reader).unwrap() as f64),
        Ok(Marker::U16) => NanBox::number(decode::read_u16(&mut reader).unwrap() as f64),
        Ok(Marker::U32) => NanBox::number(decode::read_u32(&mut reader).unwrap() as f64),
        Ok(Marker::U64) => NanBox::number(decode::read_u64(&mut reader).unwrap() as f64),
        Ok(Marker::I8) => NanBox::number(decode::read_i8(&mut reader).unwrap() as f64),
        Ok(Marker::I16) => NanBox::number(decode::read_i16(&mut reader).unwrap() as f64),
        Ok(Marker::I32) => NanBox::number(decode::read_i32(&mut reader).unwrap() as f64),
        Ok(Marker::I64) => NanBox::number(decode::read_i64(&mut reader).unwrap() as f64),
        Ok(Marker::FixPos(n)) => NanBox::number(n as f64),
        Ok(Marker::FixNeg(n)) => NanBox::number(n as f64),
        Ok(Marker::FixStr(len)) => {
            let len = len as usize;
            NanBox::string(bytes.as_ptr() as usize, len)
        }
        Ok(Marker::Str8) => {
            let len = bytes[1] as usize;
            NanBox::string(bytes.as_ptr() as usize, len)
        }
        Ok(Marker::Str16) => {
            let len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
            NanBox::string(bytes.as_ptr() as usize, len)
        }
        Ok(Marker::Str32) => {
            let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            NanBox::string(bytes.as_ptr() as usize, len)
        }
        Ok(Marker::FixMap(_) | Marker::Map16 | Marker::Map32) => {
            NanBox::obj(bytes.as_ptr() as usize)
        }
        marker => todo!("marker not yet supported: {:?}", marker),
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
            let nanbox = encode_value(&bytes);
            assert_eq!(nanbox, NanBox::bool(b));
        });
    }

    #[test]
    fn test_encode_null_value() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        let nanbox = encode_value(&bytes);
        assert_eq!(nanbox, NanBox::null());
    }

    macro_rules! test_encode_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        let nanbox = encode_value(&bytes);
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
                    let nanbox = encode_value(&bytes);
                    let decoded = nanbox.try_decode().unwrap();
                    let ptr = bytes.as_ptr() as usize & u32::MAX as usize;
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
