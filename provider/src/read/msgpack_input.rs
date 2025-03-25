use crate::read::{ErrorCode, NanBox};
use rmp::{decode, Marker};
use std::cell::UnsafeCell;
use std::io::Cursor;

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

pub(crate) struct MsgpackInput<T: AsRef<[u8]>> {
    bytes: T,
    value_jumps: UnsafeCell<Vec<(usize, Option<usize>)>>, // (start, end)
}

impl<T: AsRef<[u8]>> MsgpackInput<T> {
    pub fn new(bytes: T) -> Self {
        Self {
            bytes,
            value_jumps: UnsafeCell::new(Vec::new()),
        }
    }

    pub fn encode_value(&self, offset: usize) -> NanBox {
        let mut cursor = match self.cursor(offset) {
            Ok(cursor) => cursor,
            Err(e) => return NanBox::error(e),
        };
        let marker_position = cursor.position() as usize;
        match cursor.peek_marker() {
            Ok(Marker::False) => NanBox::bool(false),
            Ok(Marker::True) => NanBox::bool(true),
            Ok(Marker::Null) => NanBox::null(),
            Ok(Marker::F32) => NanBox::number(decode::read_f32(&mut cursor).unwrap().into()),
            Ok(Marker::F64) => NanBox::number(decode::read_f64(&mut cursor).unwrap()),
            Ok(Marker::U8) => NanBox::number(decode::read_u8(&mut cursor).unwrap() as f64),
            Ok(Marker::U16) => NanBox::number(decode::read_u16(&mut cursor).unwrap() as f64),
            Ok(Marker::U32) => NanBox::number(decode::read_u32(&mut cursor).unwrap() as f64),
            Ok(Marker::U64) => NanBox::number(decode::read_u64(&mut cursor).unwrap() as f64),
            Ok(Marker::I8) => NanBox::number(decode::read_i8(&mut cursor).unwrap() as f64),
            Ok(Marker::I16) => NanBox::number(decode::read_i16(&mut cursor).unwrap() as f64),
            Ok(Marker::I32) => NanBox::number(decode::read_i32(&mut cursor).unwrap() as f64),
            Ok(Marker::I64) => NanBox::number(decode::read_i64(&mut cursor).unwrap() as f64),
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

    fn cursor(&self, position: usize) -> Result<Cursor<&[u8]>, ErrorCode> {
        if position > self.bytes.as_ref().len() {
            return Err(ErrorCode::ByteArrayOutOfBounds);
        }
        let mut cursor = Cursor::new(self.bytes.as_ref());
        cursor.set_position(position as u64);
        Ok(cursor)
    }

    pub fn get_value_length(&self, offset: usize) -> usize {
        let mut cursor = match self.cursor(offset) {
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

    pub fn get_utf8_str_addr(&self, offset: usize) -> usize {
        let Some(&byte) = self.bytes.as_ref().get(offset) else {
            return 0;
        };
        let marker_and_length_size = match Marker::from_u8(byte) {
            Marker::FixStr(_) => 1,
            Marker::Str8 => 2,
            Marker::Str16 => 3,
            Marker::Str32 => 5,
            _ => 0,
        };
        self.bytes.as_ref().as_ptr() as usize + offset + marker_and_length_size
    }

    pub fn get_object_property(&self, offset: usize, key: &[u8]) -> NanBox {
        let mut cursor = match self.cursor(offset) {
            Ok(cursor) => cursor,
            Err(e) => return NanBox::error(e),
        };
        let Ok(map_len) = decode::read_map_len(&mut cursor) else {
            return NanBox::error(ErrorCode::ReadError);
        };
        for _ in 0..map_len {
            let Ok(len) = decode::read_str_len(&mut cursor) else {
                return NanBox::error(ErrorCode::ReadError);
            };
            let current_key = &cursor.remainder()[..len as usize];
            cursor.advance(len as usize);
            if key == current_key {
                return self.encode_value(cursor.position() as usize);
            }
            let Ok(_) = self.skip_value(&mut cursor, 0) else {
                return NanBox::error(ErrorCode::ReadError);
            };
        }
        NanBox::null()
    }

    pub fn get_at_index(&self, offset: usize, index: usize) -> NanBox {
        let mut cursor = match self.cursor(offset) {
            Ok(cursor) => cursor,
            Err(e) => return NanBox::error(e),
        };

        // Read array marker
        let array_len = match decode::read_array_len(&mut cursor) {
            Ok(array_len) => array_len as usize,
            Err(decode::ValueReadError::TypeMismatch(_)) => {
                return NanBox::error(ErrorCode::NotAnArray)
            }
            Err(_) => return NanBox::error(ErrorCode::ReadError),
        };

        if index >= array_len {
            return NanBox::error(ErrorCode::IndexOutOfBounds);
        }

        let skip_value_result = (0..index).try_fold(0, |value_jumps_start, _i| {
            self.skip_value(&mut cursor, value_jumps_start)
        });

        if let Err(e) = skip_value_result {
            return NanBox::error(e);
        }

        // Return the element at the desired index
        self.encode_value(cursor.position() as usize)
    }

    /// Skip a value in the input, and return the index of the value in the value_jumps vector.
    /// This can be used in subsequent calls to `skip_value` for the `value_jumps_start` parameter.
    fn skip_value(
        &self,
        cursor: &mut Cursor<&[u8]>,
        value_jumps_start: usize,
    ) -> Result<usize, ErrorCode> {
        let value_jumps = unsafe { &mut *self.value_jumps.get() };
        let jump_index = match value_jumps[value_jumps_start..]
            .binary_search_by_key(&(cursor.position() as usize), |(start, _)| *start)
            .map(|index| index + value_jumps_start)
            .map_err(|index| index + value_jumps_start)
        {
            Ok(index) => {
                let (_, end) = value_jumps[index];
                if let Some(end) = end {
                    cursor.set_position(end as u64);
                    return Ok(index);
                }
                index
            }
            Err(index) => {
                assert!(index == value_jumps.len());
                // need to insert so that the vector remains sorted,
                // as recursive calls to `skip_value` will add to the vector
                // in the case of composite types (maps, arrays, etc.)
                value_jumps.push((cursor.position() as usize, None));
                index
            }
        };

        // `read_marker` will advance the reader by 1 byte if it's a marker
        match cursor.read_marker()? {
            Marker::False | Marker::True | Marker::Null | Marker::FixPos(_) | Marker::FixNeg(_) => {
                Ok(jump_index)
            }
            Marker::U8 | Marker::I8 => {
                cursor.advance(1);
                Ok(jump_index)
            }
            Marker::U16 | Marker::I16 => {
                cursor.advance(2);
                Ok(jump_index)
            }
            Marker::F32 | Marker::U32 | Marker::I32 => {
                cursor.advance(4);
                Ok(jump_index)
            }
            Marker::F64 | Marker::U64 | Marker::I64 => {
                cursor.advance(8);
                Ok(jump_index)
            }
            Marker::FixStr(len) => {
                cursor.advance(len as usize);
                Ok(jump_index)
            }
            Marker::Str8 => {
                let len = cursor.read_byte(0)? as usize;
                cursor.advance(len);
                Ok(jump_index)
            }
            Marker::Str16 => {
                let len = cursor.read_u16(0)? as usize;
                cursor.advance(len);
                Ok(jump_index)
            }
            Marker::Str32 => {
                let len = cursor.read_u32(0)? as usize;
                cursor.advance(len);
                Ok(jump_index)
            }
            Marker::FixMap(len) => {
                let len = len as usize;
                (0..len * 2).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            Marker::Map16 => {
                let len = cursor.read_u16(0)? as usize;
                (0..len * 2).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            Marker::Map32 => {
                let len = cursor.read_u32(0)? as usize;
                (0..len * 2).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            Marker::FixArray(len) => {
                let len = len as usize;
                (0..len).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            Marker::Array16 => {
                let len = cursor.read_u16(0)? as usize;
                (0..len).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            Marker::Array32 => {
                let len = cursor.read_u32(0)? as usize;
                (0..len).try_fold(jump_index, |jump_index, _| {
                    self.skip_value(cursor, jump_index)
                })
            }
            _ => Err(ErrorCode::ReadError),
        }
        .inspect(|_| {
            value_jumps[jump_index].1 = Some(cursor.position() as usize);
        })
    }
}

unsafe impl<T: AsRef<[u8]>> Sync for MsgpackInput<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::encode::{self, ByteBuf};
    use shopify_function_wasm_api_core::read::ValueRef;

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
            let input = MsgpackInput::new(bytes.as_slice());
            let nanbox = input.encode_value(0);
            assert_eq!(nanbox, NanBox::bool(b));
        });
    }

    #[test]
    fn test_encode_null_value() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.encode_value(0);
        assert_eq!(nanbox, NanBox::null());
    }

    macro_rules! test_encode_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        let input = MsgpackInput::new(bytes.as_slice());
                        let nanbox = input.encode_value(0);
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
                    let input = MsgpackInput::new(bytes.as_slice());
                    let nanbox = input.encode_value(0);
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
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.encode_value(0);
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
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.encode_value(0);
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

        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.encode_value(0);
        let decoded = nanbox.try_decode().unwrap();

        match decoded {
            ValueRef::Array { len, .. } => {
                assert_eq!(len, 3);
            }
            _ => panic!("Expected array, got {:?}", decoded),
        }
    }

    fn test_skip_value(bytes: &[u8]) {
        let input = MsgpackInput::new(bytes);
        let mut cursor = input.cursor(0).unwrap();
        input.skip_value(&mut cursor, 0).unwrap();
        let expected: &[u8] = &[];
        assert_eq!(cursor.remainder(), expected);
    }

    #[test]
    fn test_skip_value_bool() {
        [true, false].iter().for_each(|&b| {
            let bytes = build_msgpack(|w| encode::write_bool(w, b)).unwrap();
            test_skip_value(&bytes);
        });
    }

    #[test]
    fn test_skip_value_null() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        test_skip_value(&bytes);
    }

    macro_rules! test_skip_value_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_skip_value_ $encode_type>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        test_skip_value(&bytes);
                    });
                }
            }
        };
        ($type:ty, $encode_type:ident) => {
            test_skip_value_number_type!($type, $encode_type, [$type::MIN, 0 as $type, $type::MAX]);
        };
        ($type:ty) => {
            paste::paste! {
                test_skip_value_number_type!($type, [<$type>]);
            }
        }
    }

    test_skip_value_number_type!(u8, pfix, [0, 1, 127]);
    test_skip_value_number_type!(u8);
    test_skip_value_number_type!(i8);
    test_skip_value_number_type!(i8, nfix, [-32, -1]);
    test_skip_value_number_type!(u16);
    test_skip_value_number_type!(i16);
    test_skip_value_number_type!(u32);
    test_skip_value_number_type!(i32);
    test_skip_value_number_type!(u64);
    test_skip_value_number_type!(i64);
    test_skip_value_number_type!(f32);
    test_skip_value_number_type!(f64);

    #[test]
    fn test_skip_value_str() {
        [
            0,
            1,
            31,
            u8::MAX as usize,
            u16::MAX as usize,
            u32::MAX as usize,
        ]
        .iter()
        .for_each(|&len| {
            let bytes = build_msgpack(|w| encode::write_str(w, "a".repeat(len).as_str())).unwrap();
            test_skip_value(&bytes);
        });
    }

    #[test]
    fn test_skip_value_array() {
        [0, 1, 15, u16::MAX as u32, u16::MAX as u32 + 1]
            .iter()
            .for_each(|&len| {
                let bytes = build_msgpack(|w| {
                    encode::write_array_len(w, len)?;
                    (0..len).try_for_each(|i| encode::write_u32(w, i))
                })
                .unwrap();
                test_skip_value(&bytes);
            });
    }

    #[test]
    fn test_skip_value_map() {
        [0, 1, 15, u16::MAX as u32, u16::MAX as u32 + 1]
            .iter()
            .for_each(|&len| {
                let bytes = build_msgpack(|w| {
                    encode::write_map_len(w, len)?;
                    (0..len).try_for_each(|i| {
                        encode::write_str(w, &i.to_string())?;
                        encode::write_u32(w, i)
                    })
                })
                .unwrap();
                test_skip_value(&bytes);
            });
    }

    #[test]
    fn test_skip_value_run_out_of_bytes() {
        let bytes = build_msgpack(|w| encode::write_map_len(w, 1).map(|_| ())).unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let mut cursor = input.cursor(0).unwrap();
        input.skip_value(&mut cursor, 0).unwrap_err();
    }

    #[test]
    fn test_get_at_index() {
        let bytes = build_msgpack(|w| {
            encode::write_array_len(w, 3)?;
            encode::write_i32(w, 1)?;
            encode::write_i32(w, 2)?;
            encode::write_i32(w, 3)
        })
        .unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.get_at_index(0, 0);
        assert_eq!(nanbox, NanBox::number(1.0));
        let nanbox = input.get_at_index(0, 1);
        assert_eq!(nanbox, NanBox::number(2.0));
        let nanbox = input.get_at_index(0, 2);
        assert_eq!(nanbox, NanBox::number(3.0));
    }

    #[test]
    fn test_get_at_index_out_of_bounds() {
        let bytes = build_msgpack(|w| encode::write_array_len(w, 0).map(|_| ())).unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.get_at_index(0, 0);
        assert_eq!(nanbox, NanBox::error(ErrorCode::IndexOutOfBounds));
    }

    #[test]
    fn test_get_at_index_not_an_array() {
        let bytes = build_msgpack(|w| encode::write_map_len(w, 1).map(|_| ())).unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.get_at_index(0, 0);
        assert_eq!(nanbox, NanBox::error(ErrorCode::NotAnArray));
    }

    #[test]
    fn test_get_object_property() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 2)?;
            encode::write_str(w, "a")?;
            encode::write_i32(w, 1)?;
            encode::write_str(w, "b")?;
            encode::write_i32(w, 2)
        })
        .unwrap();
        let input = MsgpackInput::new(bytes.as_slice());
        let nanbox = input.get_object_property(0, b"a");
        assert_eq!(nanbox, NanBox::number(1.0));
        let nanbox = input.get_object_property(0, b"b");
        assert_eq!(nanbox, NanBox::number(2.0));
    }
}
