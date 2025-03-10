use once_cell::sync::OnceCell;
use rmp::{
    decode::{read_marker, Bytes},
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
#[export_name = "shopify_function_input_get"]
extern "C" fn shopify_function_input_get() -> u64 {
    encode_value(0).to_bits()
}

fn encode_value(offset: u64) -> NanBox {
    let mut reader = Bytes::new(&bytes()[offset as usize..]);
    match read_marker(&mut reader) {
        Ok(Marker::False) => NanBox::bool(false),
        Ok(Marker::True) => NanBox::bool(true),
        Ok(Marker::Null) => NanBox::null(),
        _ => todo!(),
    }
}
