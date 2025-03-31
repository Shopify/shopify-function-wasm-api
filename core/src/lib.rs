pub mod read;
pub mod write;

/// The context used for serialization.
pub type ContextPtr = *mut std::ffi::c_void;
