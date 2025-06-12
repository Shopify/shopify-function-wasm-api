mod alloc;
pub mod log;
pub mod read;
mod string_interner;
pub mod write;

use rmp::encode::ByteBuf;
use shopify_function_wasm_api_core::ContextPtr;
use std::ptr::NonNull;
use string_interner::StringInterner;
use write::State;

pub const PROVIDER_MODULE_NAME: &str =
    concat!("shopify_function_v", env!("CARGO_PKG_VERSION_MAJOR"));

#[cfg(target_pointer_width = "64")]
type DoubleUsize = u128;
#[cfg(target_pointer_width = "32")]
type DoubleUsize = u64;

struct Context {
    bump_allocator: bumpalo::Bump,
    input_bytes: Vec<u8>,
    output_bytes: ByteBuf,
    write_state: State,
    write_parent_state_stack: Vec<State>,
    string_interner: StringInterner,
    logs: LogBuffer,
}

#[derive(Debug)]
enum ContextError {
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
            logs: LogBuffer::new(),
        }
    }

    #[cfg(target_family = "wasm")]
    fn new_from_stdin() -> Self {
        use std::io::Read;
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

macro_rules! decorate_for_target {
    ($(#[doc = $docs:tt])? fn $fn_name:ident($($args:tt)*) -> $ret:ty {
        $($body:tt)*
    }) => {
        #[cfg(target_family = "wasm")]
        $(#[doc = $docs])?
        #[export_name = concat!("_", stringify!($fn_name))]
        extern "C" fn $fn_name($($args)*) -> $ret {
            $($body)*
        }
        #[cfg(not(target_family = "wasm"))]
        $(#[doc = $docs])?
        pub fn $fn_name($($args)*) -> $ret {
            $($body)*
        }
    }
}

pub(crate) use decorate_for_target;

use crate::log::LogBuffer;

#[cfg(target_family = "wasm")]
#[export_name = "_shopify_function_context_new"]
extern "C" fn shopify_function_context_new() -> ContextPtr {
    Box::into_raw(Box::new(Context::new_from_stdin())) as _
}

#[cfg(not(target_family = "wasm"))]
pub fn shopify_function_context_new_from_msgpack_bytes(bytes: Vec<u8>) -> ContextPtr {
    Box::into_raw(Box::new(Context::new(bytes))) as _
}

decorate_for_target! {
    fn shopify_function_intern_utf8_str(context: ContextPtr, len: usize) -> DoubleUsize {
        match Context::mut_from_raw(context) {
            Ok(context) => {
            let (id, ptr) = context.string_interner.preallocate(len);
                ((id as DoubleUsize) << usize::BITS) | (ptr as DoubleUsize)
            }
            Err(_) => 0,
        }
    }
}
