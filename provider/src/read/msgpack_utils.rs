use super::CursorExt;
use rmp::{
    decode::{self, read_marker},
    Marker,
};
use std::io::Cursor;

/// Skips a value in the reader.
///
/// This function will advance the reader by the number of bytes that were read.
///
/// # Errors
///
///  This function will return an error if the reader runs out of bytes before the value is read.
pub(crate) fn skip_value(
    cursor: &mut Cursor<&[u8]>,
) -> Result<(), decode::MarkerReadError<std::io::Error>> {
    // `read_marker` will advance the reader by 1 byte if it's a marker
    match read_marker(cursor)? {
        Marker::False | Marker::True | Marker::Null | Marker::FixPos(_) | Marker::FixNeg(_) => {
            Ok(())
        }
        Marker::U8 | Marker::I8 => {
            cursor.advance(1);
            Ok(())
        }
        Marker::U16 | Marker::I16 => {
            cursor.advance(2);
            Ok(())
        }
        Marker::F32 | Marker::U32 | Marker::I32 => {
            cursor.advance(4);
            Ok(())
        }
        Marker::F64 | Marker::U64 | Marker::I64 => {
            cursor.advance(8);
            Ok(())
        }
        Marker::FixStr(len) => {
            cursor.advance(len as usize);
            Ok(())
        }
        Marker::Str8 => {
            let len = cursor.remainder()[0] as usize;
            cursor.advance(len + 1);
            Ok(())
        }
        Marker::Str16 => {
            let remaining = cursor.remainder();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]);
            cursor.advance(len as usize + 2);
            Ok(())
        }
        Marker::Str32 => {
            let remaining = cursor.remainder();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]);
            cursor.advance(len as usize + 4);
            Ok(())
        }
        Marker::FixMap(len) => {
            let len = len as usize;
            (0..len * 2).try_for_each(|_| skip_value(cursor))
        }
        Marker::Map16 => {
            let remaining = cursor.remainder();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]) as usize;
            cursor.advance(2);
            (0..len * 2).try_for_each(|_| skip_value(cursor))
        }
        Marker::Map32 => {
            let remaining = cursor.remainder();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]])
                as usize;
            cursor.advance(4);
            (0..len * 2).try_for_each(|_| skip_value(cursor))
        }
        Marker::FixArray(len) => {
            let len = len as usize;
            (0..len).try_for_each(|_| skip_value(cursor))
        }
        Marker::Array16 => {
            let remaining = cursor.remainder();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]) as usize;
            cursor.advance(2);
            (0..len).try_for_each(|_| skip_value(cursor))
        }
        Marker::Array32 => {
            let remaining = cursor.remainder();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]])
                as usize;
            cursor.advance(4);
            (0..len).try_for_each(|_| skip_value(cursor))
        }
        marker => todo!("marker not yet supported: {:?}", marker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::encode::{self, ByteBuf};

    fn build_msgpack<E, F: FnOnce(&mut ByteBuf) -> Result<(), E>>(
        writer_fn: F,
    ) -> Result<Vec<u8>, E> {
        let mut buf = ByteBuf::new();
        writer_fn(&mut buf)?;
        Ok(buf.into_vec())
    }

    fn test_skip_value(bytes: &[u8]) {
        let mut cursor = Cursor::new(bytes);
        skip_value(&mut cursor).unwrap();
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
        let mut cursor = Cursor::new(bytes.as_slice());
        skip_value(&mut cursor).unwrap_err();
    }
}
