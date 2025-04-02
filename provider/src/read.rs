use once_cell::sync::OnceCell;
use rmp::{decode, Marker};
use shopify_function_wasm_api_core::read::{ErrorCode, NanBox, ValueRef as NanBoxValueRef};
use std::io::{Cursor, Read};

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

fn cursor(position: usize) -> Result<Cursor<&'static [u8]>, ErrorCode> {
    if position > bytes().len() {
        return Err(ErrorCode::ByteArrayOutOfBounds);
    }
    let mut cursor = Cursor::new(bytes());
    cursor.set_position(position as u64);
    Ok(cursor)
}

trait CursorExt<'a> {
    fn remainder(&self) -> &'a [u8];
    fn advance(&mut self, len: usize);
    fn peek_marker(&self) -> Result<Marker, ErrorCode>;
    fn read_marker(&mut self) -> Result<Marker, ErrorCode>;
    fn read_byte(&mut self, offset: usize) -> Result<u8, ErrorCode>;
    fn read_u16(&mut self, offset: usize) -> Result<u16, ErrorCode>;
    fn read_u32(&mut self, offset: usize) -> Result<u32, ErrorCode>;
}

impl<'a> CursorExt<'a> for Cursor<&'a [u8]> {
    fn remainder(&self) -> &'a [u8] {
        &self.get_ref()[self.position() as usize..]
    }

    fn advance(&mut self, len: usize) {
        self.set_position(self.position() + len as u64);
    }

    fn peek_marker(&self) -> Result<Marker, ErrorCode> {
        let bytes = self.remainder();
        if bytes.is_empty() {
            return Err(ErrorCode::ReadError);
        }
        Ok(Marker::from_u8(bytes[0]))
    }

    fn read_marker(&mut self) -> Result<Marker, ErrorCode> {
        let byte = self.read_byte(0)?;
        Ok(Marker::from_u8(byte))
    }

    fn read_byte(&mut self, offset: usize) -> Result<u8, ErrorCode> {
        let bytes = self.remainder();
        if offset >= bytes.len() {
            return Err(ErrorCode::ReadError);
        }
        self.advance(offset + 1);
        Ok(bytes[offset])
    }

    fn read_u16(&mut self, offset: usize) -> Result<u16, ErrorCode> {
        if self.position() as usize + offset + 2 > self.get_ref().len() {
            return Err(ErrorCode::ReadError);
        }
        let bytes = &self.get_ref()[self.position() as usize + offset..];
        self.advance(offset + 2);
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self, offset: usize) -> Result<u32, ErrorCode> {
        if self.position() as usize + offset + 4 > self.get_ref().len() {
            return Err(ErrorCode::ReadError);
        }
        let bytes = &self.get_ref()[self.position() as usize + offset..];
        self.advance(offset + 4);
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

#[export_name = "_shopify_function_input_get"]
extern "C" fn shopify_function_input_get() -> u64 {
    encode_value(&mut Cursor::new(bytes())).to_bits()
}

#[export_name = "_shopify_function_input_get_obj_prop"]
extern "C" fn shopify_function_input_get_obj_prop(scope: u64, ptr: usize, len: usize) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Object { ptr: obj_ptr }) => {
            let mut cursor = match cursor(obj_ptr) {
                Ok(cursor) => cursor,
                Err(e) => return NanBox::error(e).to_bits(),
            };
            let query = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
            let Ok(map_len) = decode::read_map_len(&mut cursor) else {
                return NanBox::error(ErrorCode::ReadError).to_bits();
            };
            for _ in 0..map_len {
                let Ok(len) = decode::read_str_len(&mut cursor) else {
                    return NanBox::error(ErrorCode::ReadError).to_bits();
                };
                let key = &cursor.remainder()[..len as usize];
                cursor.advance(len as usize);
                if key == query {
                    return encode_value(&mut cursor).to_bits();
                }
                let Ok(()) = msgpack_utils::skip_value(&mut cursor) else {
                    return NanBox::error(ErrorCode::ReadError).to_bits();
                };
            }
            NanBox::null().to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnObject).to_bits(),
        Err(_) => NanBox::error(ErrorCode::DecodeError).to_bits(),
    }
}

#[export_name = "_shopify_function_input_get_at_index"]
extern "C" fn shopify_function_input_get_at_index(scope: u64, index: u32) -> u64 {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::Array { ptr, len: _ }) => {
            let mut cursor = match cursor(ptr) {
                Ok(cursor) => cursor,
                Err(e) => return NanBox::error(e).to_bits(),
            };

            // Read array marker
            let array_len = match decode::read_array_len(&mut cursor) {
                Ok(array_len) => array_len as usize,
                Err(decode::ValueReadError::TypeMismatch(_)) => {
                    return NanBox::error(ErrorCode::NotAnArray).to_bits()
                }
                Err(_) => return NanBox::error(ErrorCode::ReadError).to_bits(),
            };

            if (index as usize) >= array_len {
                return NanBox::error(ErrorCode::IndexOutOfBounds).to_bits();
            }

            // Skip elements until we reach the desired index
            for _i in 0..index {
                // Skip the current value
                if msgpack_utils::skip_value(&mut cursor).is_err() {
                    return NanBox::error(ErrorCode::ReadError).to_bits();
                }
            }

            // Return the element at the desired index
            encode_value(&mut cursor).to_bits()
        }
        Ok(_) => NanBox::error(ErrorCode::NotAnArray).to_bits(),
        Err(_) => NanBox::error(ErrorCode::ReadError).to_bits(),
    }
}

fn encode_value(cursor: &mut Cursor<&[u8]>) -> NanBox {
    let marker_position = cursor.position() as usize;
    match cursor.peek_marker() {
        Ok(Marker::False) => NanBox::bool(false),
        Ok(Marker::True) => NanBox::bool(true),
        Ok(Marker::Null) => NanBox::null(),
        Ok(Marker::F32) => NanBox::number(decode::read_f32(cursor).unwrap().into()),
        Ok(Marker::F64) => NanBox::number(decode::read_f64(cursor).unwrap()),
        Ok(Marker::U8) => NanBox::number(decode::read_u8(cursor).unwrap() as f64),
        Ok(Marker::U16) => NanBox::number(decode::read_u16(cursor).unwrap() as f64),
        Ok(Marker::U32) => NanBox::number(decode::read_u32(cursor).unwrap() as f64),
        Ok(Marker::U64) => NanBox::number(decode::read_u64(cursor).unwrap() as f64),
        Ok(Marker::I8) => NanBox::number(decode::read_i8(cursor).unwrap() as f64),
        Ok(Marker::I16) => NanBox::number(decode::read_i16(cursor).unwrap() as f64),
        Ok(Marker::I32) => NanBox::number(decode::read_i32(cursor).unwrap() as f64),
        Ok(Marker::I64) => NanBox::number(decode::read_i64(cursor).unwrap() as f64),
        Ok(Marker::FixPos(n)) => NanBox::number(n as f64),
        Ok(Marker::FixNeg(n)) => NanBox::number(n as f64),
        Ok(Marker::FixStr(len)) => {
            let len = len as usize;
            NanBox::string(marker_position, len)
        }
        Ok(Marker::Str8) => match cursor.read_byte(1) {
            Ok(len) => NanBox::string(marker_position, len as usize),
            Err(e) => NanBox::error(e),
        },
        Ok(Marker::Str16) => match cursor.read_u16(1) {
            Ok(len) => NanBox::string(marker_position, len as usize),
            Err(e) => NanBox::error(e),
        },
        Ok(Marker::Str32) => match cursor.read_u32(1) {
            Ok(len) => NanBox::string(marker_position, len as usize),
            Err(e) => NanBox::error(e),
        },
        Ok(Marker::FixMap(_) | Marker::Map16 | Marker::Map32) => NanBox::obj(marker_position),
        Ok(Marker::FixArray(len)) => NanBox::array(marker_position, len as usize),
        Ok(Marker::Array16) => match cursor.read_u16(1) {
            Ok(len) => NanBox::array(marker_position, len as usize),
            Err(e) => NanBox::error(e),
        },
        Ok(Marker::Array32) => match cursor.read_u32(1) {
            Ok(len) => NanBox::array(marker_position, len as usize),
            Err(e) => NanBox::error(e),
        },
        Ok(_) => NanBox::error(ErrorCode::ReadError),
        Err(e) => NanBox::error(e),
    }
}

/// Returns the length of the value at the given index.
/// If the value is not a string or array, it returns 0.
#[export_name = "_shopify_function_input_get_val_len"]
extern "C" fn shopify_function_input_get_val_len(scope: u64) -> usize {
    let v = NanBox::from_bits(scope);
    match v.try_decode() {
        Ok(NanBoxValueRef::String { ptr, .. } | NanBoxValueRef::Array { ptr, .. }) => {
            let mut cursor = match cursor(ptr) {
                Ok(cursor) => cursor,
                Err(_) => return 0,
            };
            match cursor.read_marker() {
                Ok(Marker::FixStr(len) | Marker::FixArray(len)) => len as usize,
                Ok(Marker::Str8) => cursor.read_byte(0).unwrap_or(0) as usize,
                Ok(Marker::Str16 | Marker::Array16) => cursor.read_u16(0).unwrap_or(0) as usize,
                Ok(Marker::Str32 | Marker::Array32) => cursor.read_u32(0).unwrap_or(0) as usize,
                _ => 0,
            }
        }
        _ => 0,
    }
}

#[export_name = "_shopify_function_input_get_utf8_str_addr"]
extern "C" fn shopify_function_input_get_utf8_str_addr(idx: usize) -> usize {
    let Some(&byte) = bytes().get(idx) else {
        return 0;
    };
    let offset = match Marker::from_u8(byte) {
        Marker::FixStr(_) => 1,
        Marker::Str8 => 2,
        Marker::Str16 => 3,
        Marker::Str32 => 5,
        _ => 0,
    };
    bytes().as_ptr() as usize + idx + offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::encode::{self, ByteBuf};
    use shopify_function_wasm_api_core::read::ValueRef;
    use std::io::Cursor;

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
            let mut cursor = Cursor::new(bytes.as_slice());
            let nanbox = encode_value(&mut cursor);
            assert_eq!(nanbox, NanBox::bool(b));
        });
    }

    #[test]
    fn test_encode_null_value() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        let mut cursor = Cursor::new(bytes.as_slice());
        let nanbox = encode_value(&mut cursor);
        assert_eq!(nanbox, NanBox::null());
    }

    macro_rules! test_encode_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        let mut cursor = Cursor::new(bytes.as_slice());
                        let nanbox = encode_value(&mut cursor);
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
        ($len:expr, $encode_type:ident) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    let bytes = build_msgpack(|w| encode::write_str(w, "a".repeat($len).as_str())).unwrap();
                    let mut cursor = Cursor::new(bytes.as_slice());
                    let nanbox = encode_value(&mut cursor);
                    let decoded = nanbox.try_decode().unwrap();
                    assert_eq!(
                        decoded,
                        ValueRef::String {
                            ptr: 0,
                            len: $len,
                        }
                    );
                }
            }
        };
    }

    test_encode_str!(31, fixstr);
    test_encode_str!(u8::MAX as usize, str8);

    #[test]
    fn test_encode_str16_value() {
        let bytes = build_msgpack(|w| encode::write_str(w, "a".repeat(u16::MAX as usize).as_str()))
            .unwrap();
        let mut cursor = Cursor::new(bytes.as_slice());
        let nanbox = encode_value(&mut cursor);
        let decoded = nanbox.try_decode().unwrap();

        assert_eq!(
            decoded,
            ValueRef::String {
                ptr: 0,
                len: NanBox::MAX_VALUE_LENGTH as usize
            }
        );
    }

    #[test]
    fn test_encode_str32_value() {
        let bytes =
            build_msgpack(|w| encode::write_str(w, "a".repeat(u16::MAX as usize + 1).as_str()))
                .unwrap();
        let mut cursor = Cursor::new(bytes.as_slice());
        let nanbox = encode_value(&mut cursor);
        let decoded = nanbox.try_decode().unwrap();

        assert_eq!(
            decoded,
            ValueRef::String {
                ptr: 0,
                len: NanBox::MAX_VALUE_LENGTH as usize
            }
        );
    }

    #[test]
    fn test_encode_array_value() {
        let bytes = build_msgpack(|w| {
            encode::write_array_len(w, 3)?;
            encode::write_i32(w, 1)?;
            encode::write_i32(w, 2)?;
            encode::write_i32(w, 3)
        })
        .unwrap();

        let mut cursor = Cursor::new(bytes.as_slice());
        let nanbox = encode_value(&mut cursor);
        let decoded = nanbox.try_decode().unwrap();

        match decoded {
            ValueRef::Array { len, .. } => {
                assert_eq!(len, 3);
            }
            _ => panic!("Expected array, got {:?}", decoded),
        }
    }
}
