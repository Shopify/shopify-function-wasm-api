#[repr(usize)]
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
    /// The value is not a shaped object, but an operation expected one.
    NotAShape = 9,
    /// The number of values written for a shaped object did not match the shape's key count.
    ShapeLengthError = 10,
    /// The shape ID does not refer to a defined shape.
    InvalidShapeId = 11,
    /// A shape definition operation was invalid (e.g. no definition in progress, key count mismatch, or a definition was already in progress).
    ShapeDefinitionError = 12,
}
