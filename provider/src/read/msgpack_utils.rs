use rmp::{
    decode::{self, read_marker, Bytes},
    Marker,
};

trait BytesExt {
    fn advance(&mut self, len: usize);
}

impl BytesExt for Bytes<'_> {
    fn advance(&mut self, len: usize) {
        *self = Bytes::new(&self.remaining_slice()[len..]);
    }
}

/// Skips a value in the reader.
///
/// This function will advance the reader by the number of bytes that were read.
///
/// # Errors
///
///  This function will return an error if the reader runs out of bytes before the value is read.
pub(crate) fn skip_value(
    reader: &mut Bytes,
) -> Result<(), decode::MarkerReadError<decode::bytes::BytesReadError>> {
    // `read_marker` will advance the reader by 1 byte if it's a marker
    match read_marker(reader)? {
        Marker::False | Marker::True | Marker::Null | Marker::FixPos(_) | Marker::FixNeg(_) => {
            Ok(())
        }
        Marker::U8 | Marker::I8 => {
            reader.advance(1);
            Ok(())
        }
        Marker::U16 | Marker::I16 => {
            reader.advance(2);
            Ok(())
        }
        Marker::F32 | Marker::U32 | Marker::I32 => {
            reader.advance(4);
            Ok(())
        }
        Marker::F64 | Marker::U64 | Marker::I64 => {
            reader.advance(8);
            Ok(())
        }
        Marker::FixStr(len) => {
            reader.advance(len as usize);
            Ok(())
        }
        Marker::Str8 => {
            let len = reader.remaining_slice()[0];
            reader.advance(len as usize + 1);
            Ok(())
        }
        Marker::Str16 => {
            let remaining = reader.remaining_slice();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]);
            reader.advance(len as usize + 2);
            Ok(())
        }
        Marker::Str32 => {
            let remaining = reader.remaining_slice();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]);
            reader.advance(len as usize + 4);
            Ok(())
        }
        Marker::FixMap(len) => {
            let len = len as usize;
            (0..len * 2).try_for_each(|_| skip_value(reader))
        }
        Marker::Map16 => {
            let remaining = reader.remaining_slice();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]) as usize;
            reader.advance(2);
            (0..len * 2).try_for_each(|_| skip_value(reader))
        }
        Marker::Map32 => {
            let remaining = reader.remaining_slice();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]])
                as usize;
            reader.advance(4);
            (0..len * 2).try_for_each(|_| skip_value(reader))
        }
        Marker::FixArray(len) => {
            let len = len as usize;
            (0..len).try_for_each(|_| skip_value(reader))
        }
        Marker::Array16 => {
            let remaining = reader.remaining_slice();
            let len = u16::from_be_bytes([remaining[0], remaining[1]]) as usize;
            reader.advance(2);
            (0..len).try_for_each(|_| skip_value(reader))
        }
        Marker::Array32 => {
            let remaining = reader.remaining_slice();
            let len = u32::from_be_bytes([remaining[0], remaining[1], remaining[2], remaining[3]])
                as usize;
            reader.advance(4);
            (0..len).try_for_each(|_| skip_value(reader))
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
        let mut reader = Bytes::new(bytes);
        skip_value(&mut reader).unwrap();
        let expected: &[u8] = &[];
        assert_eq!(reader.remaining_slice(), expected);
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
        let mut reader = Bytes::new(&bytes);
        skip_value(&mut reader).unwrap_err();
    }
}
