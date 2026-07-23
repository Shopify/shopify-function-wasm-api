mod alloc;
pub mod log;
pub mod read;
mod string_interner;
pub mod write;

use fbf::format::{
    FLAG_SHAPE_TABLE, FLAG_STRING_TABLE, MAGIC, STRREF16, STRREF32, STRREF8, VERSION,
};
#[cfg(target_family = "wasm")]
use std::cell::UnsafeCell;
use std::{cell::RefCell, ops::Range};
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
    /// Parsed input prelude and root offset, initialized on first read.
    input_state: Option<read::nav::InputState>,
    /// Small read-side navigation caches and rare long-string lengths.
    input_caches: read::nav::Caches,
    /// Inline destination for normal property names copied by the trampoline.
    input_obj_prop_buffer: [u8; 64],
    /// Reused fallback for property names larger than the inline buffer.
    input_obj_prop_overflow: Vec<u8>,
    /// The encoded root value, without the FBF header or definition prelude.
    output_bytes: Vec<u8>,
    /// The fully assembled output. This remains owned by the context so Wasm
    /// `finalize` can return a stable pointer into it.
    #[cfg(target_family = "wasm")]
    assembled_output_bytes: Vec<u8>,
    logs: Logs,
    write_state: State,
    write_parent_state_stack: Vec<State>,
    string_interner: StringInterner,
    /// Interned string IDs in output string-table order.
    string_table: Vec<shopify_function_wasm_api_core::InternedStringId>,
    /// Output string-table ID by interned string ID.
    interned_string_table_ids: Vec<Option<u32>>,
    /// Flat output shape keys, stored as string-table IDs.
    shape_keys: Vec<u32>,
    /// Ranges into `shape_keys`, in output shape-table order.
    shapes: Vec<Range<usize>>,
    /// The expected key count and starting offset of the active definition.
    open_shape_definition: Option<(usize, usize)>,
}

#[cfg(target_family = "wasm")]
thread_local! {
    static CONTEXT: UnsafeCell<Context> = UnsafeCell::new(Context::default())
}

#[cfg(not(target_family = "wasm"))]
thread_local! {
    static CONTEXT: RefCell<Context> = RefCell::new(Context::default())
}

#[cfg(target_family = "wasm")]
thread_local! {
    static OUTPUT_AND_LOG_PTRS: RefCell<[usize; 6]> = const { RefCell::new([0; 6]) };
}

impl Default for Context {
    fn default() -> Self {
        Self {
            input_bytes: Vec::new(),
            input_state: None,
            input_caches: read::nav::Caches::default(),
            input_obj_prop_buffer: [0; 64],
            input_obj_prop_overflow: Vec::new(),
            output_bytes: Vec::with_capacity(1024),
            #[cfg(target_family = "wasm")]
            assembled_output_bytes: Vec::new(),
            logs: Logs::default(),
            write_state: State::Start,
            write_parent_state_stack: Vec::new(),
            string_interner: StringInterner::new(),
            string_table: Vec::with_capacity(8),
            interned_string_table_ids: Vec::with_capacity(8),
            shape_keys: Vec::with_capacity(16),
            shapes: Vec::with_capacity(4),
            open_shape_definition: None,
        }
    }
}

impl Context {
    #[cfg(not(target_family = "wasm"))]
    fn new(input_bytes: Vec<u8>) -> Self {
        Context {
            input_bytes,
            ..Default::default()
        }
    }

    fn with<F, T>(f: F) -> T
    where
        F: FnOnce(&Context) -> T,
    {
        #[cfg(target_family = "wasm")]
        return CONTEXT.with(|context| unsafe { f(&*context.get()) });

        #[cfg(not(target_family = "wasm"))]
        CONTEXT.with_borrow(f)
    }

    fn with_mut<F, T>(f: F) -> T
    where
        F: FnOnce(&mut Context) -> T,
    {
        #[cfg(target_family = "wasm")]
        return CONTEXT.with(|context| unsafe { f(&mut *context.get()) });

        #[cfg(not(target_family = "wasm"))]
        CONTEXT.with_borrow_mut(f)
    }

    fn assemble_output_payload(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(5 + self.output_bytes.len());
        payload.extend_from_slice(&MAGIC);
        payload.push(VERSION);

        let mut flags = 0;
        if !self.string_table.is_empty() {
            flags |= FLAG_STRING_TABLE;
        }
        if !self.shapes.is_empty() {
            flags |= FLAG_SHAPE_TABLE;
        }
        payload.push(flags);

        if !self.string_table.is_empty() {
            fbf::varint::write(&mut payload, self.string_table.len() as u64);
            for &interned_id in &self.string_table {
                let entry = self.string_interner.get(interned_id);
                fbf::varint::write(&mut payload, entry.len() as u64);
                payload.extend_from_slice(entry);
            }
        }

        if !self.shapes.is_empty() {
            fbf::varint::write(&mut payload, self.shapes.len() as u64);
            for shape in &self.shapes {
                let keys = &self.shape_keys[shape.clone()];
                fbf::varint::write(&mut payload, keys.len() as u64);
                for &key_id in keys {
                    append_string_reference(&mut payload, key_id);
                }
            }
        }

        payload.extend_from_slice(&self.output_bytes);
        payload
    }
}

pub(crate) fn append_string_reference(output: &mut Vec<u8>, id: u32) {
    if let Ok(id) = u8::try_from(id) {
        output.push(STRREF8);
        output.push(id);
    } else if let Ok(id) = u16::try_from(id) {
        output.push(STRREF16);
        output.extend_from_slice(&id.to_le_bytes());
    } else {
        output.push(STRREF32);
        output.extend_from_slice(&id.to_le_bytes());
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

use crate::log::Logs;

#[cfg(target_family = "wasm")]
#[export_name = "initialize"]
extern "C" fn initialize(input_len: usize) -> *const u8 {
    Context::with_mut(|context| {
        *context = Context::default();
        context.input_bytes = vec![0; input_len];
        context.input_bytes.as_ptr()
    })
}

#[cfg(not(target_family = "wasm"))]
pub fn initialize_from_fbf_bytes(bytes: Vec<u8>) {
    CONTEXT.with_borrow_mut(|context| {
        use std::mem;

        let string_interner = mem::take(&mut context.string_interner);
        *context = Context::new(bytes);
        context.string_interner = string_interner;
    })
}

#[cfg(target_family = "wasm")]
#[export_name = "finalize"]
extern "C" fn finalize() -> *const usize {
    Context::with_mut(|context| {
        context.assembled_output_bytes = context.assemble_output_payload();
        OUTPUT_AND_LOG_PTRS.with_borrow_mut(|output_and_log_ptrs| {
            let output = &context.assembled_output_bytes;
            output_and_log_ptrs[0] = output.as_ptr() as usize;
            output_and_log_ptrs[1] = output.len();
            let (log_offset1, log_len1, log_offset2, log_len2) = context.logs.read_ptrs();
            output_and_log_ptrs[2] = log_offset1 as _;
            output_and_log_ptrs[3] = log_len1;
            output_and_log_ptrs[4] = log_offset2 as _;
            output_and_log_ptrs[5] = log_len2;
            output_and_log_ptrs.as_ptr()
        })
    })
}

decorate_for_target! {
    fn shopify_function_intern_utf8_str(len: usize) -> DoubleUsize {
        Context::with_mut(|context| {
            let (id, ptr) = context.string_interner.preallocate(len);
            ((id as DoubleUsize) << usize::BITS) | (ptr as DoubleUsize)
        })
    }
}
