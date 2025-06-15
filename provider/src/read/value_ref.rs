use crate::read::{ErrorCode, NanBox};
use rmp::Marker;

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

pub(crate) enum ValueRef {
    Null,
    Bool(bool),
    Number(f64),
    String {
        ptr: usize,
        len: usize,
        marker_and_len_size: usize,
    },
    Array {
        ptr: usize,
        len: usize,
        marker_and_len_size: usize,
    },
    Object {
        ptr: usize,
        len: usize,
        marker_and_len_size: usize,
    },
}

impl ValueRef {
    pub(crate) fn new(bytes: &[u8], position: usize) -> Result<Self, ErrorCode> {
        let mut cursor = Cursor::new(bytes, position);
        let start_position = cursor.position;
        let marker = cursor.read_marker()?;
        match marker {
            Marker::Null => Ok(Self::Null),
            Marker::True => Ok(Self::Bool(true)),
            Marker::False => Ok(Self::Bool(false)),
            Marker::FixPos(value) => Ok(Self::Number(value as f64)),
            Marker::FixNeg(value) => Ok(Self::Number(value as f64)),
            Marker::I8 => cursor.read_i8().map(|n| Self::Number(n as f64)),
            Marker::U8 => cursor.read_u8().map(|n| Self::Number(n as f64)),
            Marker::I16 => cursor.read_i16().map(|n| Self::Number(n as f64)),
            Marker::U16 => cursor.read_u16().map(|n| Self::Number(n as f64)),
            Marker::I32 => cursor.read_i32().map(|n| Self::Number(n as f64)),
            Marker::U32 => cursor.read_u32().map(|n| Self::Number(n as f64)),
            Marker::I64 => cursor.read_i64().map(|n| Self::Number(n as f64)),
            Marker::U64 => cursor.read_u64().map(|n| Self::Number(n as f64)),
            Marker::F32 => cursor.read_f32().map(|n| Self::Number(n as f64)),
            Marker::F64 => cursor.read_f64().map(Self::Number),
            Marker::FixStr(len) => Ok(Self::String {
                ptr: start_position,
                len: len as usize,
                marker_and_len_size: 1,
            }),
            Marker::Str8 => {
                let len = cursor.read_u8()? as usize;
                Ok(Self::String {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 2,
                })
            }
            Marker::Str16 => {
                let len = cursor.read_u16()? as usize;
                Ok(Self::String {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 3,
                })
            }
            Marker::Str32 => {
                let len = cursor.read_u32()? as usize;
                Ok(Self::String {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 5,
                })
            }
            Marker::Ext32 => {
                cursor.read_u32()?;
                let type_id = cursor.read_u8()?;
                if type_id == 16 {
                    let position = cursor.position;
                    Self::new(bytes, position)
                } else {
                    Err(ErrorCode::ReadError)
                }
            }
            Marker::FixMap(len) => Ok(Self::Object {
                ptr: start_position,
                len: len as usize,
                marker_and_len_size: 1,
            }),
            Marker::Map16 => {
                let len = cursor.read_u16()? as usize;
                Ok(Self::Object {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 3,
                })
            }
            Marker::Map32 => {
                let len = cursor.read_u32()? as usize;
                Ok(Self::Object {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 5,
                })
            }
            Marker::FixArray(len) => Ok(Self::Array {
                ptr: start_position,
                len: len as usize,
                marker_and_len_size: 1,
            }),
            Marker::Array16 => {
                let len = cursor.read_u16()? as usize;
                Ok(Self::Array {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 3,
                })
            }
            Marker::Array32 => {
                let len = cursor.read_u32()? as usize;
                Ok(Self::Array {
                    ptr: start_position,
                    len,
                    marker_and_len_size: 5,
                })
            }
            _ => Err(ErrorCode::ReadError),
        }
    }

    pub(crate) fn encode(&self) -> NanBox {
        match self {
            Self::Null => NanBox::null(),
            Self::Bool(value) => NanBox::bool(*value),
            Self::Number(value) => NanBox::number(*value),
            Self::String { ptr, len, .. } => NanBox::string(*ptr, *len),
            Self::Array { ptr, len, .. } => NanBox::array(*ptr, *len),
            Self::Object { ptr, len, .. } => NanBox::obj(*ptr, *len),
        }
    }

    pub(crate) fn get_value_length(&self) -> usize {
        match self {
            Self::String { len, .. } => *len,
            Self::Array { len, .. } => *len,
            Self::Object { len, .. } => *len,
            _ => usize::MAX,
        }
    }

    pub(crate) fn get_utf8_str_addr(&self, bytes: &[u8]) -> usize {
        match self {
            Self::String {
                ptr,
                marker_and_len_size,
                ..
            } => bytes.as_ptr() as usize + ptr + marker_and_len_size,
            _ => 0,
        }
    }

    pub(crate) fn get_at_index(&self, index: usize, bytes: &[u8]) -> Result<Self, ErrorCode> {
        match self {
            Self::Array {
                ptr,
                len,
                marker_and_len_size,
            } => {
                if index >= *len {
                    return Err(ErrorCode::IndexOutOfBounds);
                }
                let position = ptr + marker_and_len_size;
                let mut cursor = Cursor::new(bytes, position);
                for _ in 0..index {
                    Self::skip_value(&mut cursor)?;
                }
                Self::new(bytes, cursor.position)
            }
            Self::Object {
                ptr,
                len,
                marker_and_len_size,
            } => {
                if index >= *len {
                    return Err(ErrorCode::IndexOutOfBounds);
                }
                let position = ptr + marker_and_len_size;
                let mut cursor = Cursor::new(bytes, position);
                for _ in 0..(index * 2) {
                    Self::skip_value(&mut cursor)?;
                }
                Self::skip_value(&mut cursor)?; // skip key
                Self::new(bytes, cursor.position)
            }
            _ => Err(ErrorCode::NotIndexable),
        }
    }

    pub(crate) fn get_key_at_index(&self, index: usize, bytes: &[u8]) -> Result<Self, ErrorCode> {
        match self {
            Self::Object {
                ptr,
                len,
                marker_and_len_size,
            } => {
                if index >= *len {
                    return Err(ErrorCode::IndexOutOfBounds);
                }
                let position = ptr + marker_and_len_size;
                let mut cursor = Cursor::new(bytes, position);
                for _ in 0..(index * 2) {
                    Self::skip_value(&mut cursor)?;
                }
                Self::new(bytes, cursor.position)
            }
            _ => Err(ErrorCode::NotAnObject),
        }
    }

    pub(crate) fn get_object_property(
        &self,
        key: &[u8],
        bytes: &[u8],
    ) -> Result<Option<Self>, ErrorCode> {
        match self {
            Self::Object {
                ptr,
                len,
                marker_and_len_size,
            } => {
                let mut cursor = Cursor::new(bytes, ptr + marker_and_len_size);
                for _ in 0..*len {
                    let marker = cursor.read_marker()?;
                    let str_len = match marker {
                        Marker::FixStr(len) => len as usize,
                        Marker::Str8 => cursor.read_u8()? as usize,
                        Marker::Str16 => cursor.read_u16()? as usize,
                        Marker::Str32 => cursor.read_u32()? as usize,
                        _ => return Err(ErrorCode::ReadError),
                    };
                    let current_key = &bytes[cursor.position..cursor.position + str_len];
                    cursor.position += str_len;
                    if key == current_key {
                        return Self::new(bytes, cursor.position).map(Some);
                    }
                    Self::skip_value(&mut cursor)?;
                }
                Ok(None)
            }
            _ => Err(ErrorCode::NotAnObject),
        }
    }

    fn skip_value(cursor: &mut Cursor) -> Result<(), ErrorCode> {
        let marker = cursor.read_marker()?;
        match marker {
            Marker::Null | Marker::True | Marker::False | Marker::FixPos(_) | Marker::FixNeg(_) => {
                Ok(())
            }
            Marker::I8 | Marker::U8 => {
                cursor.position += 1;
                Ok(())
            }
            Marker::I16 | Marker::U16 => {
                cursor.position += 2;
                Ok(())
            }
            Marker::I32 | Marker::U32 | Marker::F32 => {
                cursor.position += 4;
                Ok(())
            }
            Marker::I64 | Marker::U64 | Marker::F64 => {
                cursor.position += 8;
                Ok(())
            }
            Marker::FixStr(len) => {
                cursor.position += len as usize;
                Ok(())
            }
            Marker::Str8 => {
                let len = cursor.read_u8()? as usize;
                cursor.position += len;
                Ok(())
            }
            Marker::Str16 => {
                let len = cursor.read_u16()? as usize;
                cursor.position += len;
                Ok(())
            }
            Marker::Str32 => {
                let len = cursor.read_u32()? as usize;
                cursor.position += len;
                Ok(())
            }
            Marker::Ext32 => {
                let len = cursor.read_u32()? as usize;
                cursor.read_i8()?; // type id
                cursor.position += len;
                Ok(())
            }
            _ => Err(ErrorCode::ReadError),
        }
    }
}
