mod alloc;
pub mod read;
mod string_interner;
pub mod write;

#[cfg(not(target_family = "wasm"))]
use rmp::encode;
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
    input_bytes: Vec<u8>,
    output_bytes: ByteBuf,
    write_state: State,
    write_parent_state_stack: Vec<State>,
    string_interner: StringInterner,
}

#[derive(Debug)]
enum ContextError {
    NullPointer,
}

impl Context {
    fn new(input_bytes: Vec<u8>) -> Self {
        Self {
            input_bytes,
            output_bytes: ByteBuf::new(),
            write_state: State::Start,
            write_parent_state_stack: Vec::new(),
            string_interner: StringInterner::new(),
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

#[cfg(not(target_family = "wasm"))]
pub fn json_to_custom_msgpack(json_value: &serde_json::Value) -> Vec<u8> {
    let mut byte_buf = ByteBuf::new();
    write_json_to_custom_msgpack(&mut byte_buf, json_value);
    byte_buf.into_vec()
}

#[cfg(not(target_family = "wasm"))]
fn write_json_to_custom_msgpack(byte_buf: &mut ByteBuf, json_value: &serde_json::Value) {
    match json_value {
        serde_json::Value::Null => {
            encode::write_nil(byte_buf).unwrap();
        }
        serde_json::Value::Bool(value) => {
            encode::write_bool(byte_buf, *value).unwrap();
        }
        serde_json::Value::Number(value) => {
            if value.is_i64() {
                encode::write_sint(byte_buf, value.as_i64().unwrap()).unwrap();
            } else if value.is_u64() {
                encode::write_u64(byte_buf, value.as_u64().unwrap()).unwrap();
            } else if value.is_f64() {
                encode::write_f64(byte_buf, value.as_f64().unwrap()).unwrap();
            } else {
                panic!("Unsupported number type");
            }
        }
        serde_json::Value::String(value) => {
            encode::write_str(byte_buf, value.as_str()).unwrap();
        }
        serde_json::Value::Array(value) => {
            let byte_len_idx = byte_buf.as_slice().len() + 1;
            encode::write_ext_meta(byte_buf, u32::MAX, 16).unwrap();
            let byte_array_start_idx = byte_buf.as_slice().len();
            encode::write_array_len(byte_buf, value.len() as u32).unwrap();
            for item in value {
                write_json_to_custom_msgpack(byte_buf, item);
            }
            let byte_array_len = (byte_buf.as_slice().len() - byte_array_start_idx) as u32;
            byte_buf.as_mut_vec()[byte_len_idx..byte_len_idx + 4]
                .copy_from_slice(&byte_array_len.to_be_bytes());
        }
        serde_json::Value::Object(value) => {
            let byte_len_idx = byte_buf.as_slice().len() + 1;
            encode::write_ext_meta(byte_buf, u32::MAX, 16).unwrap();
            let byte_map_start_idx = byte_buf.as_slice().len();
            encode::write_map_len(byte_buf, value.len() as u32).unwrap();
            for (key, value) in value {
                encode::write_str(byte_buf, key.as_str()).unwrap();
                write_json_to_custom_msgpack(byte_buf, value);
            }
            let byte_map_len = (byte_buf.as_slice().len() - byte_map_start_idx) as u32;
            byte_buf.as_mut_vec()[byte_len_idx..byte_len_idx + 4]
                .copy_from_slice(&byte_map_len.to_be_bytes());
        }
    }
}
