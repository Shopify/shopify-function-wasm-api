pub mod write;

#[derive(Clone, Copy)]
pub struct InternedStringId(shopify_function_wasm_api_core::InternedStringId);

impl InternedStringId {
    pub fn from_usize(id: usize) -> Self {
        Self(id)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum ContextError {
    NullPointer,
}

impl std::error::Error for ContextError {}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextError::NullPointer => write!(f, "Null pointer encountered"),
        }
    }
}

#[cfg(target_family = "wasm")]
mod wasm;
#[cfg(target_family = "wasm")]
pub use wasm::{Context, Value};

#[cfg(not(target_family = "wasm"))]
mod local;
#[cfg(not(target_family = "wasm"))]
pub use local::{Context, Value};
