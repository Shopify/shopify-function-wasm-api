pub const WriteResult = enum(usize) {
    Ok = 0,
    IoError = 1,
    ExpectedKey = 2,
    ObjectLengthError = 3,
    ValueAlreadyWritten = 4,
    NotAnObject = 5,
    ValueNotFinished = 6,
    ArrayLengthError = 7,
    NotAnArray = 8,
};