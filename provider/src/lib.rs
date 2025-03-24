mod read;
mod string_interner;
mod write;

use once_cell::sync::Lazy;
use string_interner::StringInterner;

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

static STRING_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

#[export_name = "_shopify_function_intern_utf8_str"]
extern "C" fn shopify_function_intern_utf8_str(len: usize) -> u64 {
    let (id, ptr) = STRING_INTERNER.preallocate(len);
    ((id as u64) << 32) | (ptr as u64)
}
