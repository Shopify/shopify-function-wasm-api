/// The writer context used for serialization.
pub type WriteContext = *mut std::ffi::c_void;

#[repr(u32)]
#[derive(Debug, strum::FromRepr)]
pub enum WriteResult {
    /// The write operation was successful.
    Ok = 0,
    /// An error occurred while writing to the output.
    IoError = 1,
}
