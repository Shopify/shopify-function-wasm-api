use std::error::Error;

/// A type alias to represent raw NaN-boxed values.
#[cfg(target_pointer_width = "64")]
pub type Val = u128;
#[cfg(target_pointer_width = "32")]
pub type Val = u64;

/// Values are represented as NaN-boxed values.
///
/// As a recap, IEEE floats consist of:
///
/// * 1 bit - sign
/// * 11 bits - exponent
/// * 52 bits - mantissa
///
/// A value is NaN if:
/// * The exponent bits are all 1.
/// * The most significant 2 mantissa bits are 1.
///
/// For example:
/// 1 11111111111 1[0..51]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct NanBox(Val);

impl NanBox {
    /// The number of bits to left shift an f64 to place it in a Nan-boxed value, and similarly right shift a `Val` to get an f64 out of a Nan-boxed value.
    /// For a 32-bit architecture, this is 0 because the Nan-boxed value is 64-bits.
    /// For a 64-bit architecture, this is 64 because the Nan-boxed value is 128-bits and we want the f64 to be in the most significant 64 bits.
    const F64_OFFSET: u8 = Val::BITS as u8 - 64;
    /// The number of bits reserved for the payload.
    /// The payload includes:
    /// * 32 bits for the value encoding
    /// * 14 bits for value encoding metadata. The value encoding length.
    /// * 4 bits for the value tag.
    const PAYLOAD_SIZE: u8 = 50 + Self::F64_OFFSET;
    /// The number of bits reserved for the mantissa.
    const MANTISSA_SIZE: u8 = 52;
    /// The number of bits reserved for the quiet NaN.
    const QUIET_NAN_SIZE: u8 = Self::MANTISSA_SIZE + Self::F64_OFFSET - Self::PAYLOAD_SIZE;
    /// The number of bits reserved for the exponent.
    const EXPONENT_SIZE: u8 = 11;
    /// The NaN-pattern to represent NaN-boxed values.
    /// | 0 - Sign bit | 11 - Exponent (all 1) | 2 - quiet NaN | 50 Payload |
    const NAN_MASK: Val =
        ((1 << (Self::QUIET_NAN_SIZE + Self::EXPONENT_SIZE)) - 1) << Self::PAYLOAD_SIZE;
    /// Mask to retrieve the [`Self::PAYLOAD_SIZE`] bits.
    // We want the LS 50 bits to be 1.
    const PAYLOAD_MASK: Val =
        !(Self::NAN_MASK | (1 << (Self::MANTISSA_SIZE + Self::EXPONENT_SIZE + Self::F64_OFFSET)));
    /// The number of bits reserved for the payload tag.
    const TAG_SIZE: u8 = 4;
    /// The maximum number that can be encoded in the number of bits reserved for
    /// [`TAG_SIZE`].
    const MAX_TAG_VALUE: u8 = (1 << Self::TAG_SIZE) - 1;
    /// Mask to retrieve the [`Self::TAG_SIZE`] bits.
    const TAG_MASK: Val = (Self::MAX_TAG_VALUE as Val) << Self::VALUE_SIZE;
    /// The number of bits reserved for the value encoding.
    /// Effectively 46 bits, which can contain:
    /// * The value encoded in the least significant 32-bits.
    /// * The value length encoded in the most significant 14 bits.
    const VALUE_SIZE: u8 = Self::PAYLOAD_SIZE - Self::TAG_SIZE;
    /// The number of bits reserved for the value encoding.
    /// 32 is the max number of bits given that 32-bit is the Wasm address space,
    /// which represents the pointer size of 32-bit architectures.
    const VALUE_ENCODING_SIZE: u8 = usize::BITS as u8;
    /// The number of bits reserved for the value length metadata of the value
    /// encoding.
    /// If the value is a string, this value represents the length of the string, in
    /// bytes. If the value is an array, this value represents the number of
    /// elements in the array.
    const VALUE_LENGTH_SIZE: u8 = Self::VALUE_SIZE - Self::VALUE_ENCODING_SIZE;
    /// The maximum number that can be encoed in the number of bits reserved for
    /// [`Self::VALUE_LENGTH_SIZE`].
    /// This is (2^14) - 1.
    pub const MAX_VALUE_LENGTH: usize = (1 << Self::VALUE_LENGTH_SIZE) - 1;
    /// Mask to retrive the value from the payload.
    const VALUE_MASK: Val = Self::PAYLOAD_MASK & !Self::TAG_MASK;
    /// Mask to retrive the pointer from the value, in the case that the value is
    /// an array or a string. Assumes that the value has already been masked by
    /// [`Self::VALUE_MASK`].
    const POINTER_MASK: Val = (1 << Self::VALUE_ENCODING_SIZE as Val) - 1;

    /// Retrieves the inner representation of the value.
    pub fn to_bits(&self) -> Val {
        self.0
    }

    /// Creates a NaN-boxed value from a raw `Val`.
    pub fn from_bits(val: Val) -> Self {
        Self(val)
    }

    /// Create a new NaN-boxed boolean.
    pub fn bool(val: bool) -> Self {
        let val = if val { 1 } else { 0 };
        Self::encode(val as _, 0, Tag::Bool)
    }

    /// Create the null representation of `null`.
    pub fn null() -> Self {
        Self::encode(0, 0, Tag::Null)
    }

    /// Create a new NaN-boxed number.
    pub fn number(val: f64) -> Self {
        assert!(!val.is_nan());
        Self((val.to_bits() as Val) << Self::F64_OFFSET)
    }

    /// Create a new NaN-boxed string.
    pub fn string(ptr: usize, len: usize) -> Self {
        Self::encode(ptr as _, len, Tag::String)
    }

    /// Create a new NaN-boxed object.
    pub fn obj(ptr: usize, len: usize) -> Self {
        Self::encode(ptr as _, len, Tag::Object)
    }

    /// Create a new NaN-boxed error.
    pub fn error(code: ErrorCode) -> Self {
        Self::encode(code as _, 0, Tag::Error)
    }

    /// Create a new NaN-boxed array.
    pub fn array(ptr: usize, len: usize) -> Self {
        Self::encode(ptr as _, len, Tag::Array)
    }

    pub fn try_decode(&self) -> Result<ValueRef, Box<dyn Error>> {
        if self.0 & Self::NAN_MASK != Self::NAN_MASK {
            #[cfg(target_pointer_width = "32")]
            let value = self.0;
            #[cfg(target_pointer_width = "64")]
            let value = (self.0 >> Self::F64_OFFSET) as u64;
            return Ok(ValueRef::Number(f64::from_bits(value)));
        }

        let val = self.0 & Self::VALUE_MASK;
        let ptr = val & Self::POINTER_MASK;
        let len = val >> Self::VALUE_ENCODING_SIZE;

        let ptr = ptr as *mut () as usize;
        let len = len as usize;

        let tag = self.tag()?;

        match tag {
            Tag::Bool => Ok(ValueRef::Bool(ptr != 0)),
            Tag::Null => Ok(ValueRef::Null),
            Tag::Number => unreachable!("Number values are not NaN-boxed."),
            Tag::Array => Ok(ValueRef::Array { ptr, len }),
            Tag::String => Ok(ValueRef::String { ptr, len }),
            Tag::Object => Ok(ValueRef::Object { ptr, len }),
            Tag::Error => Ok(ValueRef::Error(
                ErrorCode::from_repr(val as usize).unwrap_or(ErrorCode::Unknown),
            )),
        }
    }

    fn tag(&self) -> Result<Tag, Box<dyn Error>> {
        let tag = (self.0 & Self::PAYLOAD_MASK) >> Self::VALUE_SIZE;
        Tag::from_val(tag)
    }

    fn encode(ptr: usize, len: usize, tag: Tag) -> Self {
        let trimmed_len = len.min(Self::MAX_VALUE_LENGTH) as Val;
        let val = (trimmed_len << Self::VALUE_ENCODING_SIZE) | (ptr as Val & Self::POINTER_MASK);
        Self(Self::NAN_MASK | (tag.as_val() << Self::VALUE_SIZE) | val)
    }
}

/// An unwrapped representation of a NaN-boxed value.
#[derive(Debug, PartialEq)]
pub enum ValueRef {
    Null,
    Bool(bool),
    Number(f64),
    String { ptr: usize, len: usize },
    Object { ptr: usize, len: usize },
    Array { ptr: usize, len: usize },
    Error(ErrorCode),
}

#[derive(Debug, Clone, Copy, strum::EnumIter, strum::FromRepr)]
#[repr(u8)]
enum Tag {
    /// Null type.
    Null = 0,
    /// Boolean type.
    Bool = 1,
    /// Number type, encoded as a 64-bit floating point.
    Number = 2,
    /// String type, encoded as UTF-8.
    String = 3,
    /// An object pointer.
    Object = 4,
    /// An array pointer.
    Array = 5,
    /// An error code.
    Error = NanBox::MAX_TAG_VALUE, // this should be the last tag
}

impl Tag {
    fn as_val(&self) -> Val {
        *self as Val
    }

    fn from_val(v: Val) -> Result<Self, Box<dyn Error>> {
        match u8::try_from(v) {
            Ok(v) => Self::from_repr(v).ok_or_else(|| format!("Unknown tag: {}", v).into()),
            Err(_) => Err(format!("Unknown tag: {}", v).into()),
        }
    }
}

/// An error code.
#[derive(Debug, Clone, Copy, PartialEq, strum::EnumIter, strum::FromRepr)]
#[repr(usize)]
pub enum ErrorCode {
    /// The NanBox could not be decoded.
    DecodeError = 0,
    /// The value is not an object, but an operation expected an object.
    NotAnObject = 1,
    /// Index is out of bounds on the byte array.
    ByteArrayOutOfBounds = 2,
    /// An error occurred while attempting to read a value.
    ReadError = 3,
    /// The value is not an array, but an operation expected an array.
    NotAnArray = 4,
    /// The index is out of bounds for the array.
    IndexOutOfBounds = 5,
    /// The value is not indexable. Indexable values are objects and arrays.
    NotIndexable = 6,
    /// An unknown error code.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_tag_less_than_max_tag_value() {
        Tag::iter().for_each(|tag| {
            assert!((tag as u8) <= NanBox::MAX_TAG_VALUE);
        });
    }

    #[test]
    fn test_nan_mask() {
        for i in 0..NanBox::PAYLOAD_SIZE {
            assert!(NanBox::NAN_MASK & (1 << i) == 0);
        }

        for i in NanBox::PAYLOAD_SIZE..(Val::BITS as u8 - 1) {
            assert!(NanBox::NAN_MASK & (1 << i) != 0);
        }

        const _: () = assert!(NanBox::NAN_MASK & (1 << 63) == 0);
    }

    #[test]
    fn test_nan_mask_and_stdlib_f64_nan_constants_align() {
        let nan_mask = (NanBox::NAN_MASK >> NanBox::F64_OFFSET) as u64;
        for std_nan in &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let std_nan_without_sign = std_nan.to_bits() & !(1 << 63);
            let masked = nan_mask & std_nan.to_bits();
            assert_eq!(masked, std_nan_without_sign);
        }
    }

    #[test]
    fn test_nan_mask_is_f64_nan() {
        assert!(f64::from_bits((NanBox::NAN_MASK >> NanBox::F64_OFFSET) as u64).is_nan());
    }

    #[test]
    fn test_payload_mask() {
        for i in 0..NanBox::PAYLOAD_SIZE {
            assert!(NanBox::PAYLOAD_MASK & (1 << i) != 0);
        }

        for i in NanBox::PAYLOAD_SIZE..(Val::BITS as u8) {
            assert!(NanBox::PAYLOAD_MASK & (1 << i) == 0);
        }
    }

    #[test]
    fn test_tag_mask() {
        for i in 0..NanBox::VALUE_SIZE {
            assert!(NanBox::TAG_MASK & (1 << i) == 0);
        }

        for i in NanBox::VALUE_SIZE..NanBox::PAYLOAD_SIZE {
            assert!(NanBox::TAG_MASK & (1 << i) != 0);
        }

        for i in NanBox::PAYLOAD_SIZE..(Val::BITS as u8) {
            assert!(NanBox::TAG_MASK & (1 << i) == 0);
        }
    }

    #[test]
    fn test_null_roundtrip() {
        let null = NanBox::null();
        let value_ref = null.try_decode().unwrap();
        assert_eq!(value_ref, ValueRef::Null);
    }

    #[test]
    fn test_bool_roundtrip() {
        [true, false].iter().for_each(|&val| {
            let boxed = NanBox::bool(val);
            let value_ref = boxed.try_decode().unwrap();
            assert_eq!(value_ref, ValueRef::Bool(val));
        });
    }

    #[test]
    fn test_number_roundtrip() {
        [0.0, 1.0, -1.0, f64::MAX, f64::MIN]
            .iter()
            .for_each(|&val| {
                let boxed = NanBox::number(val);
                let value_ref = boxed.try_decode().unwrap();
                assert_eq!(value_ref, ValueRef::Number(val));
            });
    }

    #[test]
    fn test_string_roundtrip() {
        let boxed = NanBox::string(1, 2);
        let value_ref = boxed.try_decode().unwrap();
        assert_eq!(value_ref, ValueRef::String { ptr: 1, len: 2 });
    }

    #[test]
    fn test_object_roundtrip() {
        let ptr = 0x12345678;
        let len = 10;
        let boxed = NanBox::obj(ptr, len);
        let value_ref = boxed.try_decode().unwrap();
        assert_eq!(value_ref, ValueRef::Object { ptr, len });
    }

    #[test]
    fn test_error_roundtrip() {
        ErrorCode::iter().for_each(|code| {
            let error = NanBox::error(code);
            let value_ref = error.try_decode().unwrap();
            assert_eq!(value_ref, ValueRef::Error(code));
        });
    }
}
