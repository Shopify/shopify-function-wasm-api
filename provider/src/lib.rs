mod alloc;
mod read;
mod write;

use read::MsgpackInput;
use rmp::encode::ByteBuf;
use shopify_function_wasm_api_core::ContextPtr;
use std::{io::Read, ptr::NonNull};
use write::State;

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

struct Context {
    msgpack_input: MsgpackInput<Vec<u8>>,
    output_bytes: ByteBuf,
    write_state: State,
    write_parent_state_stack: Vec<State>,
}

impl Context {
    fn new(input_bytes: Vec<u8>) -> Self {
        Self {
            msgpack_input: MsgpackInput::new(input_bytes),
            output_bytes: ByteBuf::new(),
            write_state: State::Start,
            write_parent_state_stack: Vec::new(),
        }
    }

    fn new_from_stdin() -> Self {
        let mut input_bytes: Vec<u8> = vec![];
        let mut stdin = std::io::stdin();
        // Temporary use of stdin, to copy data into the Wasm linear memory.
        // Initial benchmarking doesn't seem to suggest that this represents
        // a source of performance overhead.
        stdin.read_to_end(&mut input_bytes).unwrap();

        Self::new(input_bytes)
    }
}

#[export_name = "_shopify_function_context_new"]
extern "C" fn shopify_function_context_new() -> ContextPtr {
    Box::into_raw(Box::new(Context::new_from_stdin())) as _
}

fn context_from_raw(context: ContextPtr) -> NonNull<Context> {
    unsafe { NonNull::new_unchecked(context as _) }
}
