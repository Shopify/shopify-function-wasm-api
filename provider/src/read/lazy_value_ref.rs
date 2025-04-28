use crate::read::{ErrorCode, NanBox};
use bumpalo::{collections::Vec, Bump};
use rmp::Marker;

pub(crate) type LazyValueRefPtr<'a> = *mut LazyValueRef<'a>;

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    length: usize, // Cache the length to avoid recalculating it
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], position: usize) -> Self {
        Self {
            bytes,
            position,
            length: bytes.len(),
        }
    }

    fn read_marker(&mut self) -> Result<Marker, ErrorCode> {
        if self.position >= self.length {
            return Err(ErrorCode::ReadError);
        }
        let marker = Marker::from_u8(self.bytes[self.position]);
        self.position += 1;
        Ok(marker)
    }

    fn read_f32(&mut self) -> Result<f32, ErrorCode> {
        if self.position + 4 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.position += 4;
        Ok(value)
    }

    fn read_f64(&mut self) -> Result<f64, ErrorCode> {
        if self.position + 8 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        self.position += 8;
        Ok(value)
    }

    fn read_i8(&mut self) -> Result<i8, ErrorCode> {
        if self.position + 1 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let value = self.bytes[self.position] as i8;
        self.position += 1;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, ErrorCode> {
        if self.position + 1 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let value = self.bytes[self.position];
        self.position += 1;
        Ok(value)
    }

    fn read_i16(&mut self) -> Result<i16, ErrorCode> {
        if self.position + 2 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = i16::from_be_bytes([bytes[0], bytes[1]]);
        self.position += 2;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, ErrorCode> {
        if self.position + 2 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = u16::from_be_bytes([bytes[0], bytes[1]]);
        self.position += 2;
        Ok(value)
    }

    fn read_i32(&mut self) -> Result<i32, ErrorCode> {
        if self.position + 4 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.position += 4;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, ErrorCode> {
        if self.position + 4 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.position += 4;
        Ok(value)
    }

    fn read_i64(&mut self) -> Result<i64, ErrorCode> {
        if self.position + 8 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        self.position += 8;
        Ok(value)
    }

    fn read_u64(&mut self) -> Result<u64, ErrorCode> {
        if self.position + 8 > self.length {
            return Err(ErrorCode::ReadError);
        }

        let bytes = &self.bytes[self.position..];
        let value = u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        self.position += 8;
        Ok(value)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) struct StringRef {
    ptr: usize,
    len: usize,
}

#[derive(PartialEq, Debug)]
pub(crate) struct ObjectRef<'a> {
    len: usize,
    processed_elements: Vec<'a, (StringRef, LazyValueRef<'a>)>,
    end_position_of_last_processed_element: usize,
}

impl<'a> ObjectRef<'a> {
    fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&(StringRef, LazyValueRef<'a>), ErrorCode> {
        if index >= self.len {
            return Err(ErrorCode::IndexOutOfBounds);
        }

        // Fast path: element already processed
        if index < self.processed_elements.len() {
            return Ok(&self.processed_elements[index]);
        }

        // We need to process more elements
        let count = index + 1 - self.processed_elements.len();

        // Process elements one by one until we reach the desired index
        for _ in 0..count {
            if let Some((_, last)) = self.processed_elements.last_mut() {
                if let Some(end_position) = last.finish_processing(bytes, bump)? {
                    self.end_position_of_last_processed_element = end_position;
                }
            }

            let (LazyValueRef::String(key_string_ref), Some(key_end_position)) =
                LazyValueRef::new(bytes, self.end_position_of_last_processed_element, bump)?
            else {
                return Err(ErrorCode::ReadError);
            };

            let (lazy_value, end_position) = LazyValueRef::new(bytes, key_end_position, bump)?;

            self.end_position_of_last_processed_element = end_position.unwrap_or(key_end_position);

            self.processed_elements.push((key_string_ref, lazy_value));
        }

        Ok(self.processed_elements.last().unwrap())
    }

    fn get_property(
        &mut self,
        key: &[u8],
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<&LazyValueRef<'a>>, ErrorCode> {
        let index_of_value_in_existing =
            self.processed_elements
                .iter()
                .position(|(StringRef { ptr, len }, _)| {
                    let key_bytes = &bytes[*ptr..*ptr + *len];
                    key_bytes == key
                });

        let index_of_value = match index_of_value_in_existing {
            Some(index) => Some(index),
            None => {
                let count = self.len - self.processed_elements.len();
                let mut matched = false;

                for _ in 0..count {
                    if let Some((_, last)) = self.processed_elements.last_mut() {
                        if let Some(end_position) = last.finish_processing(bytes, bump)? {
                            self.end_position_of_last_processed_element = end_position;
                        }
                    }

                    let (LazyValueRef::String(key_string_ref), Some(key_end_position)) =
                        LazyValueRef::new(
                            bytes,
                            self.end_position_of_last_processed_element,
                            bump,
                        )?
                    else {
                        return Err(ErrorCode::ReadError);
                    };

                    matched =
                        &bytes[key_string_ref.ptr..key_string_ref.ptr + key_string_ref.len] == key;

                    let (lazy_value, value_end_position) =
                        LazyValueRef::new(bytes, key_end_position, bump)?;

                    self.end_position_of_last_processed_element =
                        value_end_position.unwrap_or(key_end_position);

                    self.processed_elements.push((key_string_ref, lazy_value));

                    if matched {
                        break;
                    }
                }

                matched.then(|| self.processed_elements.len() - 1)
            }
        };

        Ok(index_of_value.map(|i| &self.processed_elements[i].1))
    }

    fn finish_processing(
        &mut self,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<usize>, ErrorCode> {
        if let Some((_, last)) = self.processed_elements.last_mut() {
            if let Some(end_position) = last.finish_processing(bytes, bump)? {
                self.end_position_of_last_processed_element = end_position;
            }
        }

        let count = self.len - self.processed_elements.len();

        for _ in 0..count {
            let (LazyValueRef::String(key), Some(end_position)) =
                LazyValueRef::new(bytes, self.end_position_of_last_processed_element, bump)?
            else {
                return Err(ErrorCode::ReadError);
            };

            let (mut lazy_value, end_position) = LazyValueRef::new(bytes, end_position, bump)?;

            self.end_position_of_last_processed_element = lazy_value
                .finish_processing(bytes, bump)?
                .or(end_position)
                .expect("`new` or `finish_processing` must return a valid end position`");

            self.processed_elements.push((key, lazy_value));
        }

        Ok(Some(self.end_position_of_last_processed_element))
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct ArrayRef<'a> {
    len: usize,
    processed_elements: Vec<'a, LazyValueRef<'a>>,
    end_position_of_last_processed_element: usize,
}

impl<'a> ArrayRef<'a> {
    fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'a>, ErrorCode> {
        if index >= self.len {
            return Err(ErrorCode::IndexOutOfBounds);
        }

        // Fast path: element already processed
        if index < self.processed_elements.len() {
            return Ok(&self.processed_elements[index]);
        }

        // We need to process more elements
        let count = index + 1 - self.processed_elements.len();

        // Process elements one by one until we reach the desired index
        for _ in 0..count {
            if let Some(last) = self.processed_elements.last_mut() {
                if let Some(end_position) = last.finish_processing(bytes, bump)? {
                    self.end_position_of_last_processed_element = end_position;
                }
            }

            let (lazy_value, end_position) =
                LazyValueRef::new(bytes, self.end_position_of_last_processed_element, bump)?;

            if let Some(end_position) = end_position {
                self.end_position_of_last_processed_element = end_position;
            }

            self.processed_elements.push(lazy_value);
        }

        Ok(self.processed_elements.last().unwrap())
    }

    fn finish_processing(
        &mut self,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<usize>, ErrorCode> {
        if let Some(last) = self.processed_elements.last_mut() {
            if let Some(end_position) = last.finish_processing(bytes, bump)? {
                self.end_position_of_last_processed_element = end_position;
            }
        }

        let count = self.len - self.processed_elements.len();

        for _ in 0..count {
            let (mut lazy_value, end_position) =
                LazyValueRef::new(bytes, self.end_position_of_last_processed_element, bump)?;

            self.end_position_of_last_processed_element = lazy_value
                .finish_processing(bytes, bump)?
                .or(end_position)
                .expect("`new` or `finish_processing` must return a valid end position`");

            self.processed_elements.push(lazy_value);
        }

        Ok(Some(self.end_position_of_last_processed_element))
    }
}

/// A lazy value reference.
///
/// This is a reference to a value that may not be fully processed.
///
/// For example, an array may not have all of its elements processed yet.
///
/// This is used to avoid unnecessary allocations and copying of data.
///
/// The value is processed when it is first accessed.
#[derive(Debug, PartialEq)]
pub(crate) enum LazyValueRef<'a> {
    Null,
    Bool(bool),
    Number(f64),
    String(StringRef),
    Array(ArrayRef<'a>),
    Object(ObjectRef<'a>),
}

impl<'a> LazyValueRef<'a> {
    pub(crate) fn encode(&self) -> NanBox {
        match self {
            LazyValueRef::Null => NanBox::null(),
            LazyValueRef::Bool(b) => NanBox::bool(*b),
            LazyValueRef::Number(n) => NanBox::number(*n),
            LazyValueRef::String(StringRef { len, .. }) => {
                let ptr = self as *const _;
                NanBox::string(ptr as _, *len)
            }
            LazyValueRef::Array(ArrayRef { len, .. }) => {
                let ptr = self as *const _;
                NanBox::array(ptr as _, *len)
            }
            LazyValueRef::Object(ObjectRef { len, .. }) => {
                let ptr = self as *const _;
                NanBox::obj(ptr as _, *len)
            }
        }
    }

    pub(crate) fn mut_from_raw<'b: 'a>(
        raw: LazyValueRefPtr<'b>,
    ) -> Result<&'b mut Self, ErrorCode> {
        if raw.is_null() {
            return Err(ErrorCode::ReadError);
        }
        // Safety: we've verified the pointer is not null
        Ok(unsafe { &mut *raw })
    }

    /// Create a new lazy value reference from a byte slice and a position.
    ///
    /// The 2-tuple in the Ok variant contains the lazy value reference as well
    /// as the position of the end of the value, if it was a non-composite type
    /// and therefore processed immediately.
    pub(crate) fn new(
        bytes: &[u8],
        position: usize,
        bump: &'a Bump,
    ) -> Result<(Self, Option<usize>), ErrorCode> {
        let mut cursor = Cursor::new(bytes, position);
        let marker = cursor.read_marker()?;

        match marker {
            // Simple values - process immediately
            Marker::Null => Ok((Self::Null, Some(cursor.position))),
            Marker::False => Ok((Self::Bool(false), Some(cursor.position))),
            Marker::True => Ok((Self::Bool(true), Some(cursor.position))),

            // Fixed positive and negative integers - no additional reads needed
            Marker::FixPos(n) => Ok((Self::Number(n as f64), Some(cursor.position))),
            Marker::FixNeg(n) => Ok((Self::Number(n as f64), Some(cursor.position))),

            // Numbers requiring additional reads
            Marker::I8 => cursor
                .read_i8()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::U8 => cursor
                .read_u8()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::U16 => cursor
                .read_u16()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::U32 => cursor
                .read_u32()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::U64 => cursor
                .read_u64()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::I16 => cursor
                .read_i16()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::I32 => cursor
                .read_i32()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::I64 => cursor
                .read_i64()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::F32 => cursor
                .read_f32()
                .map(|n| (Self::Number(n as f64), Some(cursor.position))),
            Marker::F64 => cursor
                .read_f64()
                .map(|n| (Self::Number(n), Some(cursor.position))),

            // String types
            Marker::FixStr(len) => {
                let len = len as usize;
                Ok((
                    Self::String(StringRef {
                        ptr: cursor.position,
                        len,
                    }),
                    Some(cursor.position + len),
                ))
            }
            Marker::Str8 => {
                let len = cursor.read_u8().map(|n| n as usize)?;
                Ok((
                    Self::String(StringRef {
                        ptr: cursor.position,
                        len,
                    }),
                    Some(cursor.position + len),
                ))
            }
            Marker::Str16 => {
                let len = cursor.read_u16().map(|n| n as usize)?;
                Ok((
                    Self::String(StringRef {
                        ptr: cursor.position,
                        len,
                    }),
                    Some(cursor.position + len),
                ))
            }
            Marker::Str32 => {
                let len = cursor.read_u32().map(|n| n as usize)?;
                Ok((
                    Self::String(StringRef {
                        ptr: cursor.position,
                        len,
                    }),
                    Some(cursor.position + len),
                ))
            }

            // Map types
            Marker::FixMap(len) => {
                let len = len as usize;
                Ok((
                    Self::Object(ObjectRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }
            Marker::Map16 => {
                let len = cursor.read_u16().map(|n| n as usize)?;
                Ok((
                    Self::Object(ObjectRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }
            Marker::Map32 => {
                let len = cursor.read_u32().map(|n| n as usize)?;
                Ok((
                    Self::Object(ObjectRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }

            // Array types
            Marker::FixArray(len) => {
                let len = len as usize;
                Ok((
                    Self::Array(ArrayRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }
            Marker::Array16 => {
                let len = cursor.read_u16().map(|n| n as usize)?;
                Ok((
                    Self::Array(ArrayRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }
            Marker::Array32 => {
                let len = cursor.read_u32().map(|n| n as usize)?;
                Ok((
                    Self::Array(ArrayRef {
                        len,
                        processed_elements: Vec::with_capacity_in(len, bump),
                        end_position_of_last_processed_element: cursor.position,
                    }),
                    None,
                ))
            }

            // Unknown or unsupported marker
            _ => Err(ErrorCode::ReadError),
        }
    }

    pub(crate) fn get_value_length(&self) -> usize {
        match self {
            Self::String(StringRef { len, .. }) => *len,
            Self::Array(ArrayRef { len, .. }) => *len,
            Self::Object(ObjectRef { len, .. }) => *len,
            _ => 0,
        }
    }

    pub(crate) fn get_utf8_str_addr(&self, bytes: &[u8]) -> usize {
        match self {
            Self::String(StringRef { ptr, .. }) => bytes[*ptr..].as_ptr() as usize,
            _ => 0,
        }
    }

    pub(crate) fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef, ErrorCode> {
        match self {
            Self::Array(array_ref) => array_ref.get_at_index(index, bytes, bump),
            Self::Object(obj_ref) => obj_ref.get_at_index(index, bytes, bump).map(|v| &v.1),
            _ => Err(ErrorCode::NotIndexable),
        }
    }

    pub(crate) fn get_key_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&StringRef, ErrorCode> {
        match self {
            Self::Object(obj_ref) => obj_ref.get_at_index(index, bytes, bump).map(|v| &v.0),
            _ => Err(ErrorCode::NotAnObject),
        }
    }

    pub(crate) fn get_object_property<'b>(
        &'b mut self,
        key: &[u8],
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<&'b Self>, ErrorCode> {
        match self {
            Self::Object(obj_ref) => obj_ref.get_property(key, bytes, bump),
            _ => Err(ErrorCode::NotAnObject),
        }
    }

    /// Returns the end position of the value, if it was a composite type and
    /// therefore was finished during this call. If it was not a composite type,
    /// the end position is not known and None is returned, but the end position
    /// would have been returned in the `new` call to create the value.
    fn finish_processing(
        &mut self,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<usize>, ErrorCode> {
        match self {
            Self::Array(array_ref) => array_ref.finish_processing(bytes, bump),
            Self::Null | Self::Bool(_) | Self::Number(_) | Self::String { .. } => Ok(None),
            Self::Object(obj_ref) => obj_ref.finish_processing(bytes, bump),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::encode::{self, ByteBuf};
    use std::vec::Vec;

    fn build_msgpack<E, F: FnOnce(&mut ByteBuf) -> Result<(), E>>(
        writer_fn: F,
    ) -> Result<Vec<u8>, E> {
        let mut buf = ByteBuf::new();
        writer_fn(&mut buf)?;
        Ok(buf.into_vec())
    }

    fn create_lazy_value<'a>(bytes: &'a [u8], bump: &'a Bump) -> LazyValueRef<'a> {
        let (value, _) = LazyValueRef::new(bytes, 0, bump).unwrap();
        value
    }

    #[test]
    fn test_instantiate_bool_value() {
        [true, false].iter().for_each(|&b| {
            let bytes = build_msgpack(|w| encode::write_bool(w, b)).unwrap();
            let bump = Bump::new();
            let value = create_lazy_value(&bytes, &bump);
            assert_eq!(value, LazyValueRef::Bool(b));
        });
    }

    #[test]
    fn test_encode_bool_value() {
        [true, false].iter().for_each(|&b| {
            let value = LazyValueRef::Bool(b);
            let nanbox = value.encode();
            assert_eq!(nanbox, NanBox::bool(b));
        });
    }

    #[test]
    fn test_instantiate_null_value() {
        let bytes = build_msgpack(encode::write_nil).unwrap();
        let bump = Bump::new();
        let value = create_lazy_value(&bytes, &bump);
        assert_eq!(value, LazyValueRef::Null);
    }

    #[test]
    fn test_encode_null_value() {
        let value = LazyValueRef::Null;
        let nanbox = value.encode();
        assert_eq!(nanbox, NanBox::null());
    }

    macro_rules! test_instantiate_number_type {
        ($type:ty, $encode_type:ident, $values:tt) => {
            paste::paste! {
                #[test]
                fn [<test_instantiate_ $encode_type _value>]() {
                    $values.iter().for_each(|&n| {
                        let bytes = build_msgpack(|w| encode::[<write_ $encode_type>](w, n)).unwrap();
                        let bump = Bump::new();
                        let value = create_lazy_value(&bytes, &bump);
                        assert_eq!(value, LazyValueRef::Number(n as f64));
                    });
                }
            }
        };
        ($type:ty, $encode_type:ident) => {
            test_instantiate_number_type!($type, $encode_type, [$type::MIN, 0 as $type, $type::MAX]);
        };
        ($type:ty) => {
            paste::paste! {
                test_instantiate_number_type!($type, [<$type>]);
            }
        };
    }

    test_instantiate_number_type!(u8, pfix, [0, 1, 127]);
    test_instantiate_number_type!(u8);
    test_instantiate_number_type!(i8);
    test_instantiate_number_type!(i8, nfix, [-32, -1]);
    test_instantiate_number_type!(u16);
    test_instantiate_number_type!(i16);
    test_instantiate_number_type!(u32);
    test_instantiate_number_type!(i32);
    test_instantiate_number_type!(u64);
    test_instantiate_number_type!(i64);
    test_instantiate_number_type!(f32);
    test_instantiate_number_type!(f64);

    #[test]
    fn test_encode_number_value() {
        let value = LazyValueRef::Number(1.0);
        let nanbox = value.encode();
        assert_eq!(nanbox, NanBox::number(1.0));
    }

    macro_rules! test_instantiate_and_encode_str {
        ($len:expr, $encode_type:ident, $offset:expr) => {
            paste::paste! {
                #[test]
                fn [<test_instantiate_ $encode_type _value>]() {
                    let bytes = build_msgpack(|w| encode::write_str(w, "a".repeat($len).as_str())).unwrap();
                    let bump = Bump::new();
                    let value = create_lazy_value(&bytes, &bump);
                    assert_eq!(value, LazyValueRef::String(StringRef { len: $len, ptr: $offset }));
                }

                #[test]
                fn [<test_encode_ $encode_type _value>]() {
                    let value = LazyValueRef::String(StringRef { len: $len, ptr: $offset });
                    let nanbox = value.encode();
                    let ptr = &value as *const _ as usize;
                    // The length is limited by the max value that can be stored in the length portion of the NanBox
                    let expected_length = ($len).min(NanBox::MAX_VALUE_LENGTH as usize);
                    assert_eq!(nanbox, NanBox::string(ptr, expected_length));
                }
            }
        };
    }

    test_instantiate_and_encode_str!(31, fixstr, 1);
    test_instantiate_and_encode_str!(u8::MAX as usize, str8, 2);
    test_instantiate_and_encode_str!(u16::MAX as usize, str16, 3);
    test_instantiate_and_encode_str!(u16::MAX as usize + 1, str32, 5);

    #[test]
    fn test_instantiate_and_traverse_array_value() {
        let bytes = build_msgpack(|w| {
            encode::write_array_len(w, 3)?;
            encode::write_i32(w, 1)?;
            encode::write_i32(w, 2)?;
            encode::write_i32(w, 3)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        // fixarray so the length and marker are in the same byte
        assert_eq!(
            value,
            LazyValueRef::Array(ArrayRef {
                len: 3,
                processed_elements: bumpalo::collections::Vec::new_in(&bump),
                end_position_of_last_processed_element: 1
            })
        );

        [1.0, 2.0, 3.0].iter().enumerate().for_each(|(i, n)| {
            let element = value.get_at_index(i, &bytes, &bump).unwrap();
            assert_eq!(element, &LazyValueRef::Number(*n));
            match &value {
                LazyValueRef::Array(array_ref) => {
                    assert_eq!(array_ref.processed_elements.len(), i + 1);
                }
                _ => panic!("Expected array, got {:?}", value),
            }
        });

        let end_position = value.finish_processing(&bytes, &bump).unwrap();
        assert_eq!(end_position, Some(bytes.len()));
    }

    #[test]
    fn test_encode_array_value() {
        let bump = Bump::new();
        let len = 3;
        let value = LazyValueRef::Array(ArrayRef {
            len,
            processed_elements: bumpalo::collections::Vec::new_in(&bump),
            end_position_of_last_processed_element: 0,
        });
        let nanbox = value.encode();
        let ptr = &value as *const _ as usize;
        assert_eq!(nanbox, NanBox::array(ptr, len));
    }

    #[test]
    fn test_get_at_index_array() {
        let bytes = build_msgpack(|w| {
            encode::write_array_len(w, 3)?;
            encode::write_i32(w, 1)?;
            encode::write_i32(w, 2)?;
            encode::write_i32(w, 3)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);

        let element = value.get_at_index(0, &bytes, &bump).unwrap();
        assert_eq!(element, &LazyValueRef::Number(1.0));

        let element = value.get_at_index(1, &bytes, &bump).unwrap();
        assert_eq!(element, &LazyValueRef::Number(2.0));

        let element = value.get_at_index(2, &bytes, &bump).unwrap();
        assert_eq!(element, &LazyValueRef::Number(3.0));
    }

    #[test]
    fn test_get_at_index_array_out_of_bounds() {
        let bytes = build_msgpack(|w| encode::write_array_len(w, 0).map(|_| ())).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_at_index(0, &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::IndexOutOfBounds);
    }

    #[test]
    fn get_at_index_object() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 2)?;
            encode::write_str(w, "a")?;
            encode::write_i32(w, 1)?;
            encode::write_str(w, "b")?;
            encode::write_i32(w, 2)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);

        let element = value.get_at_index(0, &bytes, &bump).unwrap();
        assert_eq!(element, &LazyValueRef::Number(1.0));

        let element = value.get_at_index(1, &bytes, &bump).unwrap();
        assert_eq!(element, &LazyValueRef::Number(2.0));
    }

    #[test]
    fn test_get_at_index_object_out_of_bounds() {
        let bytes = build_msgpack(|w| encode::write_map_len(w, 0).map(|_| ())).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_at_index(0, &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::IndexOutOfBounds);
    }

    #[test]
    fn test_get_at_index_not_indexable() {
        let bytes = build_msgpack(|w| encode::write_str(w, "")).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_at_index(0, &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::NotIndexable);
    }

    #[test]
    fn test_instantiate_and_traverse_object_value() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 2)?;
            encode::write_str(w, "a")?;
            encode::write_i32(w, 1)?;
            encode::write_str(w, "b")?;
            encode::write_i32(w, 2)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        // fixmap so the length and marker are in the same byte
        assert_eq!(
            value,
            LazyValueRef::Object(ObjectRef {
                len: 2,
                processed_elements: bumpalo::collections::Vec::new_in(&bump),
                end_position_of_last_processed_element: 1
            })
        );

        [("a", 1), ("b", 2)]
            .iter()
            .enumerate()
            .for_each(|(i, (k, v))| {
                let property = value
                    .get_object_property(k.as_bytes(), &bytes, &bump)
                    .unwrap()
                    .unwrap();
                assert_eq!(property, &LazyValueRef::Number(*v as f64));
                match &value {
                    LazyValueRef::Object(obj_ref) => {
                        assert_eq!(obj_ref.processed_elements.len(), i + 1);
                    }
                    _ => panic!("Expected object, got {:?}", value),
                }
            });

        let end_position = value.finish_processing(&bytes, &bump).unwrap();
        assert_eq!(end_position, Some(bytes.len()));
    }

    #[test]
    fn test_encode_object_value() {
        let bump = Bump::new();
        let len = 2;
        let value = LazyValueRef::Object(ObjectRef {
            len,
            processed_elements: bumpalo::collections::Vec::new_in(&bump),
            end_position_of_last_processed_element: 0,
        });
        let nanbox = value.encode();
        let ptr = &value as *const _ as usize;
        assert_eq!(nanbox, NanBox::obj(ptr, len));
    }

    #[test]
    fn test_get_object_property() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 2)?;
            encode::write_str(w, "a")?;
            encode::write_i32(w, 1)?;
            encode::write_str(w, "b")?;
            encode::write_i32(w, 2)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);

        let property = value
            .get_object_property(b"a", &bytes, &bump)
            .unwrap()
            .unwrap();
        assert_eq!(property, &LazyValueRef::Number(1.0));

        let property = value
            .get_object_property(b"b", &bytes, &bump)
            .unwrap()
            .unwrap();
        assert_eq!(property.encode(), NanBox::number(2.0));
    }

    #[test]
    fn test_get_object_property_not_found() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 1)?;
            encode::write_str(w, "a")?;
            encode::write_i32(w, 1)
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);

        let result = value.get_object_property(b"b", &bytes, &bump).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_object_property_not_an_object() {
        let bytes = build_msgpack(|w| encode::write_array_len(w, 0).map(|_| ())).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_object_property(b"a", &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::NotAnObject);
    }

    #[test]
    fn test_get_key_at_index() {
        let bytes = build_msgpack(|w| {
            encode::write_map_len(w, 2)?;
            encode::write_str(w, "a")?;
            encode::write_sint(w, 1)?;
            encode::write_str(w, "b")?;
            encode::write_sint(w, 2).map(|_| ())
        })
        .unwrap();

        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);

        let key = value.get_key_at_index(0, &bytes, &bump).unwrap();
        // 1 byte for the map marker, 1 byte for the fixstr marker/length, so the key is at offset 2
        assert_eq!(key, &StringRef { len: 1, ptr: 2 });

        let key = value.get_key_at_index(1, &bytes, &bump).unwrap();
        // from the start of the previous key (2), we have 1 byte for the contents of the previous key,
        // 1 byte for the fixnum marker/length, and 1 byte for the fixstr marker/length, so the key is at offset 5
        assert_eq!(key, &StringRef { len: 1, ptr: 5 });
    }

    #[test]
    fn test_get_key_at_index_out_of_bounds() {
        let bytes = build_msgpack(|w| encode::write_map_len(w, 0).map(|_| ())).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_key_at_index(0, &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::IndexOutOfBounds);
    }

    #[test]
    fn test_get_key_at_index_not_an_object() {
        let bytes = build_msgpack(|w| encode::write_array_len(w, 0).map(|_| ())).unwrap();
        let bump = Bump::new();
        let mut value = create_lazy_value(&bytes, &bump);
        let error = value.get_key_at_index(0, &bytes, &bump).unwrap_err();
        assert_eq!(error, ErrorCode::NotAnObject);
    }
}
