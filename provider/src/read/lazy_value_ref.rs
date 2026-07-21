use crate::read::{ErrorCode, NanBox};
use bumpalo::{collections::Vec, Bump};

pub(crate) type LazyValueRefPtr<'a> = *mut LazyValueRef<'a>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct StringRef {
    /// Offset into the payload bytes where the UTF-8 string data begins.
    offset: usize,
    len: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct ShapeRef<'a> {
    keys: &'a [StringRef],
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Tables<'a> {
    strings: &'a [StringRef],
    shapes: &'a [ShapeRef<'a>],
}

#[derive(PartialEq, Debug)]
pub(crate) struct ArrayRef<'a> {
    len: usize,
    processed_elements: Vec<'a, LazyValueRef<'a>>,
    next_position: usize,
    payload_end: usize,
    tables: Tables<'a>,
}

#[derive(PartialEq, Debug)]
pub(crate) struct ObjectRef<'a> {
    len: usize,
    kind: ObjectKind<'a>,
}

#[derive(PartialEq, Debug)]
enum ObjectKind<'a> {
    Map {
        processed_elements: Vec<'a, (LazyValueRef<'a>, LazyValueRef<'a>)>,
        next_position: usize,
        payload_end: usize,
        tables: Tables<'a>,
    },
    Shape {
        keys: &'a [StringRef],
        processed_values: Vec<'a, LazyValueRef<'a>>,
        next_position: usize,
        payload_end: usize,
        tables: Tables<'a>,
    },
}

/// A lazy value reference backed by an FBF payload.
#[derive(Debug, PartialEq)]
pub(crate) enum LazyValueRef<'a> {
    Null,
    Bool(bool),
    Number(f64),
    String(StringRef),
    Array(ArrayRef<'a>),
    Object(ObjectRef<'a>),
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    fn read_u8(&mut self, end: usize) -> Result<u8, ErrorCode> {
        if self.position >= end {
            return Err(ErrorCode::ReadError);
        }
        let value = self.bytes[self.position];
        self.position += 1;
        Ok(value)
    }

    fn read_varint(&mut self, end: usize) -> Result<u64, ErrorCode> {
        let mut result = 0_u64;
        for shift in (0..70).step_by(7) {
            let byte = self.read_u8(end)?;
            if shift == 63 && byte > 1 {
                return Err(ErrorCode::ReadError);
            }
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(ErrorCode::ReadError)
    }
}

fn checked_end(position: usize, size: usize, limit: usize) -> Result<usize, ErrorCode> {
    let end = position.checked_add(size).ok_or(ErrorCode::ReadError)?;
    if end > limit {
        return Err(ErrorCode::ReadError);
    }
    Ok(end)
}

fn read_len(bytes: &[u8], position: usize, width: usize, limit: usize) -> Result<usize, ErrorCode> {
    let end = checked_end(position, width, limit)?;
    let value = match width {
        1 => bytes[position] as usize,
        2 => u16::from_le_bytes(bytes[position..end].try_into().unwrap()) as usize,
        4 => u32::from_le_bytes(bytes[position..end].try_into().unwrap()) as usize,
        _ => return Err(ErrorCode::ReadError),
    };
    Ok(value)
}

fn length_width(tag: u8) -> Option<usize> {
    match tag {
        0x8d | 0x93 | 0x96 | 0x9c => Some(1),
        0x8e | 0x94 | 0x97 | 0x9d => Some(2),
        0x8f | 0x95 | 0x98 | 0x9e => Some(4),
        0xd1..=0xd7 | 0xd9..=0xdf => Some(1),
        _ => None,
    }
}

fn payload_range(
    bytes: &[u8],
    position: usize,
    tag: u8,
    limit: usize,
) -> Result<(usize, usize), ErrorCode> {
    let width = length_width(tag).ok_or(ErrorCode::ReadError)?;
    let length_position = checked_end(position, 1, limit)?;
    let payload_start = checked_end(length_position, width, limit)?;
    let len = read_len(bytes, length_position, width, limit)?;
    let payload_end = checked_end(payload_start, len, limit)?;
    Ok((payload_start, payload_end))
}

fn string_eq(candidate: StringRef, key: &[u8], bytes: &[u8]) -> bool {
    if candidate.len != key.len() {
        return false;
    }
    bytes
        .get(candidate.offset..candidate.offset + candidate.len)
        .is_some_and(|candidate| candidate == key)
}

impl<'a> Tables<'a> {
    fn get_string(&self, id: usize) -> Result<StringRef, ErrorCode> {
        self.strings.get(id).copied().ok_or(ErrorCode::ReadError)
    }

    fn get_shape(&self, id: usize) -> Result<ShapeRef<'a>, ErrorCode> {
        self.shapes.get(id).copied().ok_or(ErrorCode::ReadError)
    }
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
        // Safety: the API only stores pointers to `LazyValueRef`s allocated in the context bump arena.
        Ok(unsafe { &mut *raw })
    }

    /// Create a new lazy value reference from a complete FBF payload.
    pub(crate) fn new(
        bytes: &[u8],
        position: usize,
        bump: &'a Bump,
    ) -> Result<(Self, Option<usize>), ErrorCode> {
        if position != 0 || bytes.len() < 5 || &bytes[0..3] != b"FBF" || bytes[3] != 1 {
            return Err(ErrorCode::ReadError);
        }
        let flags = bytes[4];
        if flags & 0xfc != 0 {
            return Err(ErrorCode::ReadError);
        }

        let mut cursor = Cursor::new(bytes, 5);
        let strings: &[StringRef] = if flags & 0x01 != 0 {
            let count = usize::try_from(cursor.read_varint(bytes.len())?)
                .map_err(|_| ErrorCode::ReadError)?;
            let mut strings = std::vec::Vec::with_capacity(count);
            for _ in 0..count {
                let len = usize::try_from(cursor.read_varint(bytes.len())?)
                    .map_err(|_| ErrorCode::ReadError)?;
                let end = checked_end(cursor.position, len, bytes.len())?;
                strings.push(StringRef {
                    offset: cursor.position,
                    len,
                });
                cursor.position = end;
            }
            bump.alloc_slice_copy(&strings)
        } else {
            &[]
        };
        let mut tables = Tables {
            strings,
            shapes: &[],
        };

        let shapes: &[ShapeRef] = if flags & 0x02 != 0 {
            let count = usize::try_from(cursor.read_varint(bytes.len())?)
                .map_err(|_| ErrorCode::ReadError)?;
            let mut shapes = std::vec::Vec::with_capacity(count);
            for _ in 0..count {
                let key_count = usize::try_from(cursor.read_varint(bytes.len())?)
                    .map_err(|_| ErrorCode::ReadError)?;
                let mut keys = std::vec::Vec::with_capacity(key_count);
                for _ in 0..key_count {
                    let (key, end) = parse_string_ref(bytes, cursor.position, bytes.len(), tables)?;
                    keys.push(key);
                    cursor.position = end;
                }
                shapes.push(ShapeRef {
                    keys: bump.alloc_slice_copy(&keys),
                });
            }
            bump.alloc_slice_copy(&shapes)
        } else {
            &[]
        };
        tables.shapes = shapes;

        let (value, end) = Self::new_at(bytes, cursor.position, bytes.len(), bump, tables)?;
        Ok((value, Some(end)))
    }

    fn new_at(
        bytes: &[u8],
        position: usize,
        limit: usize,
        bump: &'a Bump,
        tables: Tables<'a>,
    ) -> Result<(Self, usize), ErrorCode> {
        if position >= limit {
            return Err(ErrorCode::ReadError);
        }
        let tag = bytes[position];
        let payload = position + 1;
        match tag {
            0x00..=0x7f => Ok((Self::Number(f64::from(tag)), payload)),
            0xe0..=0xff => Ok((Self::Number(f64::from(tag as i8)), payload)),
            0x80 => Ok((Self::Null, payload)),
            0x81 => Ok((Self::Bool(false), payload)),
            0x82 => Ok((Self::Bool(true), payload)),
            0x83 => {
                let end = checked_end(payload, 1, limit)?;
                Ok((Self::Number(f64::from(bytes[payload] as i8)), end))
            }
            0x84 => {
                let end = checked_end(payload, 2, limit)?;
                let value = i16::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(f64::from(value)), end))
            }
            0x85 => {
                let end = checked_end(payload, 4, limit)?;
                let value = i32::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(f64::from(value)), end))
            }
            0x86 => {
                let end = checked_end(payload, 8, limit)?;
                let value = i64::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(value as f64), end))
            }
            0x87 => {
                let end = checked_end(payload, 1, limit)?;
                Ok((Self::Number(f64::from(bytes[payload])), end))
            }
            0x88 => {
                let end = checked_end(payload, 2, limit)?;
                let value = u16::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(f64::from(value)), end))
            }
            0x89 => {
                let end = checked_end(payload, 4, limit)?;
                let value = u32::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(f64::from(value)), end))
            }
            0x8a => {
                let end = checked_end(payload, 8, limit)?;
                let value = u64::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(value as f64), end))
            }
            0x8b => {
                let end = checked_end(payload, 4, limit)?;
                let value = f32::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(f64::from(value)), end))
            }
            0x8c => {
                let end = checked_end(payload, 8, limit)?;
                let value = f64::from_le_bytes(bytes[payload..end].try_into().unwrap());
                Ok((Self::Number(value), end))
            }
            0x8d..=0x8f | 0xa2..=0xc1 | 0x99..=0x9b => {
                let (string, end) = parse_string_ref(bytes, position, limit, tables)?;
                Ok((Self::String(string), end))
            }
            0xd0 => Ok((
                Self::Array(ArrayRef {
                    len: 0,
                    processed_elements: Vec::new_in(bump),
                    next_position: payload,
                    payload_end: payload,
                    tables,
                }),
                payload,
            )),
            0xd1..=0xd7 | 0x93..=0x95 => {
                let (count, next_position, payload_end, end) =
                    parse_array_header(bytes, position, limit)?;
                Ok((
                    Self::Array(ArrayRef {
                        len: count,
                        processed_elements: Vec::with_capacity_in(count, bump),
                        next_position,
                        payload_end,
                        tables,
                    }),
                    end,
                ))
            }
            0xd8 => Ok((
                Self::Object(ObjectRef {
                    len: 0,
                    kind: ObjectKind::Map {
                        processed_elements: Vec::new_in(bump),
                        next_position: payload,
                        payload_end: payload,
                        tables,
                    },
                }),
                payload,
            )),
            0xd9..=0xdf | 0x96..=0x98 => {
                let (count, next_position, payload_end, end) =
                    parse_map_header(bytes, position, limit)?;
                Ok((
                    Self::Object(ObjectRef {
                        len: count,
                        kind: ObjectKind::Map {
                            processed_elements: Vec::with_capacity_in(count, bump),
                            next_position,
                            payload_end,
                            tables,
                        },
                    }),
                    end,
                ))
            }
            0x9c..=0x9e => {
                let (payload_start, payload_end) = payload_range(bytes, position, tag, limit)?;
                let mut cursor = Cursor::new(bytes, payload_start);
                let shape_id = usize::try_from(cursor.read_varint(payload_end)?)
                    .map_err(|_| ErrorCode::ReadError)?;
                let shape = tables.get_shape(shape_id)?;
                let count = shape.keys.len();
                Ok((
                    Self::Object(ObjectRef {
                        len: count,
                        kind: ObjectKind::Shape {
                            keys: shape.keys,
                            processed_values: Vec::with_capacity_in(count, bump),
                            next_position: cursor.position,
                            payload_end,
                            tables,
                        },
                    }),
                    payload_end,
                ))
            }
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
            Self::String(StringRef { offset, .. }) => bytes[*offset..].as_ptr() as usize,
            _ => 0,
        }
    }

    pub(crate) fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'_>, ErrorCode> {
        match self {
            Self::Array(array_ref) => array_ref.get_at_index(index, bytes, bump),
            Self::Object(obj_ref) => obj_ref.get_at_index(index, bytes, bump),
            _ => Err(ErrorCode::NotIndexable),
        }
    }

    pub(crate) fn get_key_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'_>, ErrorCode> {
        match self {
            Self::Object(obj_ref) => obj_ref.get_key_at_index(index, bytes, bump),
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
}

fn parse_string_ref(
    bytes: &[u8],
    position: usize,
    limit: usize,
    tables: Tables<'_>,
) -> Result<(StringRef, usize), ErrorCode> {
    if position >= limit {
        return Err(ErrorCode::ReadError);
    }
    let tag = bytes[position];
    let payload = position + 1;
    match tag {
        0xa2..=0xc1 => {
            let len = (tag - 0xa2) as usize;
            let end = checked_end(payload, len, limit)?;
            Ok((
                StringRef {
                    offset: payload,
                    len,
                },
                end,
            ))
        }
        0x8d..=0x8f => {
            let (payload_start, payload_end) = payload_range(bytes, position, tag, limit)?;
            Ok((
                StringRef {
                    offset: payload_start,
                    len: payload_end - payload_start,
                },
                payload_end,
            ))
        }
        0x99 => {
            let end = checked_end(payload, 1, limit)?;
            Ok((tables.get_string(bytes[payload] as usize)?, end))
        }
        0x9a => {
            let end = checked_end(payload, 2, limit)?;
            let id = u16::from_le_bytes(bytes[payload..end].try_into().unwrap()) as usize;
            Ok((tables.get_string(id)?, end))
        }
        0x9b => {
            let end = checked_end(payload, 4, limit)?;
            let id = u32::from_le_bytes(bytes[payload..end].try_into().unwrap()) as usize;
            Ok((tables.get_string(id)?, end))
        }
        _ => Err(ErrorCode::ReadError),
    }
}

fn parse_array_header(
    bytes: &[u8],
    position: usize,
    limit: usize,
) -> Result<(usize, usize, usize, usize), ErrorCode> {
    let tag = bytes[position];
    let (payload_start, payload_end) = payload_range(bytes, position, tag, limit)?;
    if (0xd1..=0xd7).contains(&tag) {
        Ok((
            (tag - 0xd0) as usize,
            payload_start,
            payload_end,
            payload_end,
        ))
    } else {
        let mut cursor = Cursor::new(bytes, payload_start);
        let count =
            usize::try_from(cursor.read_varint(payload_end)?).map_err(|_| ErrorCode::ReadError)?;
        Ok((count, cursor.position, payload_end, payload_end))
    }
}

fn parse_map_header(
    bytes: &[u8],
    position: usize,
    limit: usize,
) -> Result<(usize, usize, usize, usize), ErrorCode> {
    let tag = bytes[position];
    let (payload_start, payload_end) = payload_range(bytes, position, tag, limit)?;
    if (0xd9..=0xdf).contains(&tag) {
        Ok((
            (tag - 0xd8) as usize,
            payload_start,
            payload_end,
            payload_end,
        ))
    } else {
        let mut cursor = Cursor::new(bytes, payload_start);
        let count =
            usize::try_from(cursor.read_varint(payload_end)?).map_err(|_| ErrorCode::ReadError)?;
        Ok((count, cursor.position, payload_end, payload_end))
    }
}

impl<'a> ArrayRef<'a> {
    fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'_>, ErrorCode> {
        if index >= self.len {
            return Err(ErrorCode::IndexOutOfBounds);
        }
        while self.processed_elements.len() <= index {
            let (value, end) = LazyValueRef::new_at(
                bytes,
                self.next_position,
                self.payload_end,
                bump,
                self.tables,
            )?;
            self.next_position = end;
            self.processed_elements.push(value);
        }
        self.processed_elements
            .get(index)
            .ok_or(ErrorCode::IndexOutOfBounds)
    }
}

impl<'a> ObjectRef<'a> {
    fn processed_len(&self) -> usize {
        match &self.kind {
            ObjectKind::Map {
                processed_elements, ..
            } => processed_elements.len(),
            ObjectKind::Shape {
                processed_values, ..
            } => processed_values.len(),
        }
    }

    fn process_next(&mut self, bytes: &[u8], bump: &'a Bump) -> Result<(), ErrorCode> {
        match &mut self.kind {
            ObjectKind::Map {
                processed_elements,
                next_position,
                payload_end,
                tables,
            } => {
                let (key, value_start) =
                    LazyValueRef::new_at(bytes, *next_position, *payload_end, bump, *tables)?;
                let (value, end) =
                    LazyValueRef::new_at(bytes, value_start, *payload_end, bump, *tables)?;
                *next_position = end;
                processed_elements.push((key, value));
            }
            ObjectKind::Shape {
                processed_values,
                next_position,
                payload_end,
                tables,
                ..
            } => {
                let (value, end) =
                    LazyValueRef::new_at(bytes, *next_position, *payload_end, bump, *tables)?;
                *next_position = end;
                processed_values.push(value);
            }
        }
        Ok(())
    }

    fn get_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'_>, ErrorCode> {
        if index >= self.len {
            return Err(ErrorCode::IndexOutOfBounds);
        }
        while self.processed_len() <= index {
            self.process_next(bytes, bump)?;
        }
        match &self.kind {
            ObjectKind::Map {
                processed_elements, ..
            } => processed_elements
                .get(index)
                .map(|(_, value)| value)
                .ok_or(ErrorCode::IndexOutOfBounds),
            ObjectKind::Shape {
                processed_values, ..
            } => processed_values
                .get(index)
                .ok_or(ErrorCode::IndexOutOfBounds),
        }
    }

    fn get_key_at_index(
        &mut self,
        index: usize,
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<&LazyValueRef<'_>, ErrorCode> {
        if index >= self.len {
            return Err(ErrorCode::IndexOutOfBounds);
        }
        match &self.kind {
            ObjectKind::Shape { keys, .. } => Ok(bump.alloc(LazyValueRef::String(keys[index]))),
            ObjectKind::Map { .. } => {
                while self.processed_len() <= index {
                    self.process_next(bytes, bump)?;
                }
                match &self.kind {
                    ObjectKind::Map {
                        processed_elements, ..
                    } => processed_elements
                        .get(index)
                        .map(|(key, _)| key)
                        .ok_or(ErrorCode::IndexOutOfBounds),
                    _ => unreachable!(),
                }
            }
        }
    }

    fn get_property<'b>(
        &'b mut self,
        key: &[u8],
        bytes: &[u8],
        bump: &'a Bump,
    ) -> Result<Option<&'b LazyValueRef<'a>>, ErrorCode> {
        match &self.kind {
            ObjectKind::Shape { keys, .. } => {
                let Some(index) = keys
                    .iter()
                    .position(|candidate| string_eq(*candidate, key, bytes))
                else {
                    return Ok(None);
                };
                while self.processed_len() <= index {
                    self.process_next(bytes, bump)?;
                }
                match &self.kind {
                    ObjectKind::Shape {
                        processed_values, ..
                    } => Ok(processed_values.get(index)),
                    _ => unreachable!(),
                }
            }
            ObjectKind::Map { .. } => {
                for index in 0..self.len {
                    while self.processed_len() <= index {
                        self.process_next(bytes, bump)?;
                    }
                    let found = match &self.kind {
                        ObjectKind::Map {
                            processed_elements, ..
                        } => match &processed_elements[index].0 {
                            LazyValueRef::String(candidate) => string_eq(*candidate, key, bytes),
                            _ => false,
                        },
                        _ => unreachable!(),
                    };
                    if found {
                        return match &self.kind {
                            ObjectKind::Map {
                                processed_elements, ..
                            } => Ok(Some(&processed_elements[index].1)),
                            _ => unreachable!(),
                        };
                    }
                }
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fbf::Value as FbfValue;

    fn encode(value: &FbfValue) -> std::vec::Vec<u8> {
        fbf::to_vec(value).unwrap()
    }

    #[test]
    fn test_instantiate_scalars() {
        let bump = Bump::new();
        for (value, expected) in [
            (FbfValue::Nil, LazyValueRef::Null),
            (FbfValue::Bool(true), LazyValueRef::Bool(true)),
            (FbfValue::Int(42), LazyValueRef::Number(42.0)),
        ] {
            let bytes = encode(&value);
            let (parsed, _) = LazyValueRef::new(&bytes, 0, &bump).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn test_string() {
        let bump = Bump::new();
        let bytes = encode(&FbfValue::Str("hello".to_string()));
        let (value, _) = LazyValueRef::new(&bytes, 0, &bump).unwrap();
        assert_eq!(value.get_value_length(), 5);
        let ptr = value.get_utf8_str_addr(&bytes);
        let str_bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, 5) };
        assert_eq!(str_bytes, b"hello");
    }

    #[test]
    fn test_get_at_index_array() {
        let bump = Bump::new();
        let bytes = encode(&FbfValue::Array(vec![
            FbfValue::Int(1),
            FbfValue::Bool(false),
        ]));
        let (mut value, _) = LazyValueRef::new(&bytes, 0, &bump).unwrap();
        assert_eq!(value.get_value_length(), 2);
        assert_eq!(
            value.get_at_index(0, &bytes, &bump).unwrap(),
            &LazyValueRef::Number(1.0)
        );
        assert_eq!(
            value.get_at_index(1, &bytes, &bump).unwrap(),
            &LazyValueRef::Bool(false)
        );
        assert_eq!(
            value.get_at_index(2, &bytes, &bump).unwrap_err(),
            ErrorCode::IndexOutOfBounds
        );
    }

    #[test]
    fn test_object_lookup_and_keys() {
        let bump = Bump::new();
        let bytes = encode(&FbfValue::Map(vec![
            (FbfValue::Str("a".to_string()), FbfValue::Int(1)),
            (FbfValue::Str("b".to_string()), FbfValue::Bool(true)),
        ]));
        let (mut value, _) = LazyValueRef::new(&bytes, 0, &bump).unwrap();
        assert_eq!(value.get_value_length(), 2);
        assert_eq!(
            value.get_at_index(1, &bytes, &bump).unwrap(),
            &LazyValueRef::Bool(true)
        );
        assert_eq!(
            value.get_object_property(b"a", &bytes, &bump).unwrap(),
            Some(&LazyValueRef::Number(1.0))
        );
        let key = value.get_key_at_index(0, &bytes, &bump).unwrap();
        assert_eq!(key.get_value_length(), 1);
    }
}
