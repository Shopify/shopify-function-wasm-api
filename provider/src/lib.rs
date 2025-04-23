mod alloc;
mod read;
mod string_interner;
mod write;

use rmp::encode::ByteBuf;
use shopify_function_wasm_api_core::ContextPtr;
use std::{io::Read, ptr::NonNull};
use string_interner::StringInterner;
use write::State;

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

struct Context {
    bump_allocator: bumpalo::Bump,
    input_bytes: Vec<u8>,
    output_bytes: ByteBuf,
    write_state: State,
    write_parent_state_stack: Vec<State>,
    string_interner: StringInterner,
}

#[derive(Debug)]
pub enum ContextError {
    NullPointer,
}

impl Context {
    fn new(input_bytes: Vec<u8>) -> Self {
        let bump_allocator = bumpalo::Bump::new();
        Self {
            bump_allocator,
            input_bytes,
            output_bytes: ByteBuf::new(),
            write_state: State::Start,
            write_parent_state_stack: Vec::new(),
            string_interner: StringInterner::new(),
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

    fn ref_from_raw<'a>(raw: ContextPtr) -> Result<&'a Self, ContextError> {
        NonNull::new(raw as _)
            .ok_or(ContextError::NullPointer)
            .map(|ptr| unsafe { ptr.as_ref() })
    }

    fn mut_from_raw<'a>(raw: ContextPtr) -> Result<&'a mut Self, ContextError> {
        NonNull::new(raw as _)
            .ok_or(ContextError::NullPointer)
            .map(|mut ptr| unsafe { ptr.as_mut() })
    }
}

#[export_name = "_shopify_function_context_new"]
extern "C" fn shopify_function_context_new() -> ContextPtr {
    Box::into_raw(Box::new(Context::new_from_stdin())) as _
}

#[export_name = "_shopify_function_intern_utf8_str"]
extern "C" fn shopify_function_intern_utf8_str(context: ContextPtr, len: usize) -> u64 {
    match Context::mut_from_raw(context) {
        Ok(context) => {
            let (id, ptr) = context.string_interner.preallocate(len);
            ((id as u64) << 32) | (ptr as u64)
        }
        Err(_) => 0,
    }
}
