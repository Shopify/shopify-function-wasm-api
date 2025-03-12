use once_cell::sync::OnceCell;
use rmp::{
    decode::{self, read_marker, Bytes},
    Marker,
};
use shopify_function_wasm_api_core::NanBox;
use std::io::Read;
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
                fn [<test_encode_ $encode_type>]() {
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
}
