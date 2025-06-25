mod alloc;
pub mod log;
pub mod read;
mod string_interner;
pub mod write;

use bumpalo::Bump;
use rmp::encode::ByteBuf;
use std::cell::RefCell;
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
    logs: Vec<u8>,
    write_state: State,
    write_parent_state_stack: Vec<State>,
    string_interner: StringInterner,
}

thread_local! {
    static CONTEXT: RefCell<Context> = RefCell::new(Context::default())
}

#[cfg(target_family = "wasm")]
thread_local! {
    static OUTPUT_AND_LOG_PTRS: RefCell<[usize; 4]> = RefCell::new([0; 4]);
}

impl Default for Context {
    fn default() -> Self {
        Self {
            bump_allocator: Bump::new(),
            input_bytes: Vec::new(),
            output_bytes: ByteBuf::with_capacity(1024),
            logs: Vec::with_capacity(1024),
            write_state: State::Start,
            write_parent_state_stack: Vec::new(),
            string_interner: StringInterner::new(),
        }
    }
}

impl Context {
    #[cfg(not(target_family = "wasm"))]
    fn new(input_bytes: Vec<u8>) -> Self {
        let mut context = Self::default();
        context.input_bytes = input_bytes;
        context
    }

    fn with<F, T>(f: F) -> T
    where
        F: FnOnce(&Context) -> T,
    {
        CONTEXT.with_borrow(f)
    }

    fn with_mut<F, T>(f: F) -> T
    where
        F: FnOnce(&mut Context) -> T,
    {
        CONTEXT.with_borrow_mut(f)
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
#[export_name = "initialize"]
extern "C" fn initialize(
    input_len: usize,
    output_initial_capacity: usize,
    log_initial_capacity: usize,
) -> *const u8 {
    CONTEXT.with_borrow_mut(|context| {
        *context = Context::default();
        context.input_bytes = vec![0; input_len];
        context.output_bytes = ByteBuf::with_capacity(output_initial_capacity);
        context.logs = Vec::with_capacity(log_initial_capacity);
        context.input_bytes.as_ptr()
    })
}

#[cfg(not(target_family = "wasm"))]
pub fn initialize_from_msgpack_bytes(bytes: Vec<u8>) {
    CONTEXT.with_borrow_mut(|context| *context = Context::new(bytes))
}

#[cfg(target_family = "wasm")]
#[export_name = "finalize"]
extern "C" fn finalize() -> *const usize {
    Context::with(|context| {
        OUTPUT_AND_LOG_PTRS.with_borrow_mut(|output_and_log_ptrs| {
            let output = context.output_bytes.as_vec();
            output_and_log_ptrs[0] = output.as_ptr() as usize;
            output_and_log_ptrs[1] = output.len();
            output_and_log_ptrs[2] = context.logs.as_ptr() as usize;
            output_and_log_ptrs[3] = context.logs.len();
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
