/// The writer context used for serialization.
pub type WriteContext = *mut std::ffi::c_void;

#[repr(u32)]
#[derive(Debug, strum::FromRepr, PartialEq, Eq)]
pub enum WriteResult {
    /// The write operation was successful.
    Ok = 0,
    /// An error occurred while writing to the output.
    IoError = 1,
    /// Tried to write a value when a key was expected.
    ExpectedKey = 2,
    /// The object length was not honoured.
    ObjectLengthError = 3,
    /// Tried to write a value when a value was already written.
    ValueAlreadyWritten = 4,
    /// The value is not an object, but an operation expected an object.
    NotAnObject = 5,
    /// Value not finished.
    ValueNotFinished = 6,
    /// The array length was not honoured.
    ArrayLengthError = 7,
    /// The value is not an array, but an operation expected an array.
    NotAnArray = 8,
}
