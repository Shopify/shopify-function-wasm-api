//! Pure offset-based FBF navigation for the provider read path.
//!
//! This module implements lazy, cursor-based navigation over FBF payloads
//! without heap allocation. Values are represented as offsets into the input
//! bytes, and NanBox encodings carry those offsets directly.

use fbf::{
    format,
    read::{Reader, Span, Tables},
};
use shopify_function_wasm_api_core::read::{ErrorCode, NanBox};

const MAX_DEPTH: usize = 128;

/// Holds lengths that do not fit in a NanBox. String NanBoxes carry content
/// offsets, so the encoded length prefix cannot be recovered from that offset.
#[derive(Default)]
pub(crate) struct LongStringLens {
    entries: Vec<(usize, usize)>, // (content offset, length)
}

impl LongStringLens {
    pub fn get(&self, offset: usize) -> Option<usize> {
        self.entries
            .iter()
            .find(|(entry_offset, _)| *entry_offset == offset)
            .map(|(_, len)| *len)
    }

    pub fn insert(&mut self, offset: usize, len: usize) {
        if len < NanBox::MAX_VALUE_LENGTH {
            return;
        }
        if let Some((_, existing_len)) = self
            .entries
            .iter_mut()
            .find(|(entry_offset, _)| *entry_offset == offset)
        {
            *existing_len = len;
        } else {
            self.entries.push((offset, len));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Array,
    Map,
    Shape(usize),
}

#[derive(Clone, Copy)]
struct ContainerMeta {
    offset: usize,
    first_child: usize,
    count: usize,
    end: usize,
    kind: ContainerKind,
}

/// Small caches for decoded container metadata, cursor positions, and recent
/// shaped-object key matches. Input access tends to alternate between one
/// parent array and its child objects, so four slots cover the hot working set.
#[derive(Default)]
pub(crate) struct CursorMemo {
    entries: [Option<(u32, u32, u32)>; 4], // (container offset, index, child offset)
    containers: [Option<ContainerMeta>; 4],
    shape_keys: [Option<(u32, u32)>; 4], // (shape id, most recently matched key index)
}

impl CursorMemo {
    pub fn get(&self, container: usize, index: usize) -> Option<usize> {
        let container = u32::try_from(container).ok()?;
        let index = u32::try_from(index).ok()?;
        self.entries
            .iter()
            .flatten()
            .find(|(entry_container, entry_index, _)| {
                *entry_container == container && *entry_index == index
            })
            .map(|(_, _, child_offset)| *child_offset as usize)
    }

    pub fn insert(&mut self, container: usize, index: usize, child_offset: usize) {
        let (Ok(container), Ok(index), Ok(child_offset)) = (
            u32::try_from(container),
            u32::try_from(index),
            u32::try_from(child_offset),
        ) else {
            return;
        };
        self.entries.rotate_right(1);
        self.entries[0] = Some((container, index, child_offset));
    }

    fn container(&self, offset: usize) -> Option<ContainerMeta> {
        self.containers
            .iter()
            .flatten()
            .find(|meta| meta.offset == offset)
            .copied()
    }

    fn insert_container(&mut self, meta: ContainerMeta) {
        if let Some(existing) = self
            .containers
            .iter_mut()
            .find(|entry| entry.is_some_and(|entry| entry.offset == meta.offset))
        {
            *existing = Some(meta);
            return;
        }
        self.containers.rotate_right(1);
        self.containers[0] = Some(meta);
    }

    fn shape_key(&self, shape_id: usize) -> Option<usize> {
        let shape_id = u32::try_from(shape_id).ok()?;
        self.shape_keys
            .iter()
            .flatten()
            .find(|(entry_shape_id, _)| *entry_shape_id == shape_id)
            .map(|(_, key_index)| *key_index as usize)
    }

    fn insert_shape_key(&mut self, shape_id: usize, key_index: usize) {
        let (Ok(shape_id), Ok(key_index)) = (u32::try_from(shape_id), u32::try_from(key_index))
        else {
            return;
        };
        self.shape_keys.rotate_right(1);
        self.shape_keys[0] = Some((shape_id, key_index));
    }

    /// If the immediately preceding child is cached, resume from it.
    pub fn find_preceding(&self, container: usize, index: usize) -> Option<(usize, usize)> {
        if index == 0 {
            return None;
        }
        let container = u32::try_from(container).ok()?;
        let target = u32::try_from(index - 1).ok()?;
        self.entries
            .iter()
            .flatten()
            .find(|(entry_container, entry_index, _)| {
                *entry_container == container && *entry_index == target
            })
            .map(|(_, cached_index, child_offset)| (*cached_index as usize, *child_offset as usize))
    }
}

/// Decode a value at the given offset and return its type, content span, and
/// encoded end offset. Does NOT allocate or recursively parse containers.
pub(crate) fn decode_value(
    bytes: &[u8],
    tables: &Tables,
    offset: usize,
    limit: usize,
    long_strs: &mut LongStringLens,
) -> Result<(ValueType, usize), ErrorCode> {
    if offset >= limit || limit > bytes.len() {
        return Err(ErrorCode::ReadError);
    }

    let mut reader = Reader::new(bytes);
    reader.pos = offset;
    reader.limit = limit;

    let tag = reader.read_u8().map_err(|_| ErrorCode::ReadError)?;

    // Positive fixint
    if format::is_pos_fixint(tag) {
        return Ok((ValueType::Number(tag as f64), reader.pos));
    }
    // Negative fixint
    if format::is_neg_fixint(tag) {
        return Ok((ValueType::Number((tag as i8) as f64), reader.pos));
    }
    // Fixstr
    if format::is_fixstr(tag) {
        let len = format::fixstr_len(tag);
        let ptr = reader.pos;
        reader.read_slice(len).map_err(|_| ErrorCode::ReadError)?;
        long_strs.insert(ptr, len);
        return Ok((ValueType::String { ptr, len }, reader.pos));
    }

    match tag {
        format::NIL => Ok((ValueType::Null, reader.pos)),
        format::FALSE => Ok((ValueType::Bool(false), reader.pos)),
        format::TRUE => Ok((ValueType::Bool(true), reader.pos)),

        format::INT8 | format::INT16 | format::INT32 | format::INT64 => {
            let width = match tag {
                format::INT8 => 1,
                format::INT16 => 2,
                format::INT32 => 4,
                _ => 8,
            };
            let value = reader
                .read_int_le(width)
                .map_err(|_| ErrorCode::ReadError)?;
            Ok((ValueType::Number(value as f64), reader.pos))
        }

        format::UINT8 | format::UINT16 | format::UINT32 | format::UINT64 => {
            let width = match tag {
                format::UINT8 => 1,
                format::UINT16 => 2,
                format::UINT32 => 4,
                _ => 8,
            };
            let value = reader
                .read_uint_le(width)
                .map_err(|_| ErrorCode::ReadError)?;
            Ok((ValueType::Number(value as f64), reader.pos))
        }

        format::FLOAT32 => {
            let bits = reader.read_uint_le(4).map_err(|_| ErrorCode::ReadError)? as u32;
            Ok((ValueType::Number(f32::from_bits(bits) as f64), reader.pos))
        }

        format::FLOAT64 => {
            let bits = reader.read_uint_le(8).map_err(|_| ErrorCode::ReadError)?;
            Ok((ValueType::Number(f64::from_bits(bits)), reader.pos))
        }

        format::STR8 | format::STR16 | format::STR32 => {
            let width = format::length_width(tag).ok_or(ErrorCode::ReadError)?;
            let len = read_usize(&mut reader, width)?;
            let ptr = reader.pos;
            reader.read_slice(len).map_err(|_| ErrorCode::ReadError)?;
            long_strs.insert(ptr, len);
            Ok((ValueType::String { ptr, len }, reader.pos))
        }

        format::STRREF8 => {
            if !tables.has_string_table || reader.pos >= limit {
                return Err(ErrorCode::ReadError);
            }
            let id = bytes[reader.pos] as usize;
            reader.pos += 1;
            let span = tables
                .strings
                .get(id)
                .copied()
                .ok_or(ErrorCode::ReadError)?;
            long_strs.insert(span.start, span.len);
            Ok((
                ValueType::String {
                    ptr: span.start,
                    len: span.len,
                },
                reader.pos,
            ))
        }

        format::STRREF16 | format::STRREF32 => {
            if !tables.has_string_table {
                return Err(ErrorCode::ReadError);
            }
            let width = format::length_width_strref(tag);
            let id = reader
                .read_uint_le(width)
                .map_err(|_| ErrorCode::ReadError)?;
            let encoded_end = reader.pos;
            let span = tables.string_span(id).map_err(|_| ErrorCode::ReadError)?;
            long_strs.insert(span.start, span.len);
            Ok((
                ValueType::String {
                    ptr: span.start,
                    len: span.len,
                },
                encoded_end,
            ))
        }

        // Empty fixed containers
        tag if tag == format::FIXARRAY0 => Ok((
            ValueType::Array {
                offset,
                first_child: reader.pos,
                count: 0,
                frame_end: Some(reader.pos),
            },
            reader.pos,
        )),
        tag if tag == format::FIXMAP0 => Ok((
            ValueType::Map {
                offset,
                first_child: reader.pos,
                count: 0,
                frame_end: Some(reader.pos),
            },
            reader.pos,
        )),

        // Length-framed fixarray
        tag if format::is_fixarray(tag) => {
            let count = format::fixarray_count(tag);
            let frame_end = read_frame_end(&mut reader, 1)?;
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, frame_end)?;
            Ok((
                ValueType::Array {
                    offset,
                    first_child,
                    count,
                    frame_end: Some(frame_end),
                },
                frame_end,
            ))
        }

        // Sequential fixarray
        tag if format::is_seqfixarray(tag) => {
            let count = format::seqfixarray_count(tag);
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, limit)?;
            Ok((
                ValueType::Array {
                    offset,
                    first_child,
                    count,
                    frame_end: None,
                },
                0, // must be computed by walking children
            ))
        }

        // Length-framed array
        format::ARRAY8 | format::ARRAY16 | format::ARRAY32 => {
            let width = format::length_width(tag).ok_or(ErrorCode::ReadError)?;
            let frame_end = read_frame_end(&mut reader, width)?;
            reader.limit = frame_end;
            let count = read_container_index(&mut reader)?;
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, frame_end)?;
            Ok((
                ValueType::Array {
                    offset,
                    first_child,
                    count,
                    frame_end: Some(frame_end),
                },
                frame_end,
            ))
        }

        // Sequential array
        format::SEQARRAY => {
            let count = read_container_index(&mut reader)?;
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, limit)?;
            Ok((
                ValueType::Array {
                    offset,
                    first_child,
                    count,
                    frame_end: None,
                },
                0,
            ))
        }

        // Length-framed fixmap
        tag if format::is_fixmap(tag) => {
            let count = format::fixmap_count(tag);
            let frame_end = read_frame_end(&mut reader, 1)?;
            let first_child = reader.pos;
            validate_children_fit(count, 2, first_child, frame_end)?;
            Ok((
                ValueType::Map {
                    offset,
                    first_child,
                    count,
                    frame_end: Some(frame_end),
                },
                frame_end,
            ))
        }

        // Sequential fixmap
        tag if format::is_seqfixmap(tag) => {
            let count = format::seqfixmap_count(tag);
            let first_child = reader.pos;
            validate_children_fit(count, 2, first_child, limit)?;
            Ok((
                ValueType::Map {
                    offset,
                    first_child,
                    count,
                    frame_end: None,
                },
                0,
            ))
        }

        // Length-framed map
        format::MAP8 | format::MAP16 | format::MAP32 => {
            let width = format::length_width(tag).ok_or(ErrorCode::ReadError)?;
            let frame_end = read_frame_end(&mut reader, width)?;
            reader.limit = frame_end;
            let count = read_container_index(&mut reader)?;
            let first_child = reader.pos;
            validate_children_fit(count, 2, first_child, frame_end)?;
            Ok((
                ValueType::Map {
                    offset,
                    first_child,
                    count,
                    frame_end: Some(frame_end),
                },
                frame_end,
            ))
        }

        // Sequential map
        format::SEQMAP => {
            let count = read_container_index(&mut reader)?;
            let first_child = reader.pos;
            validate_children_fit(count, 2, first_child, limit)?;
            Ok((
                ValueType::Map {
                    offset,
                    first_child,
                    count,
                    frame_end: None,
                },
                0,
            ))
        }

        // Length-framed shape
        format::SHAPE8 | format::SHAPE16 | format::SHAPE32 => {
            if !tables.has_shape_table {
                return Err(ErrorCode::ReadError);
            }
            let width = format::length_width(tag).ok_or(ErrorCode::ReadError)?;
            let frame_end = read_frame_end(&mut reader, width)?;
            reader.limit = frame_end;
            let shape_id = read_container_index(&mut reader)?;
            let keys = tables
                .shape(shape_id as u64)
                .map_err(|_| ErrorCode::ReadError)?;
            let count = keys.len();
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, frame_end)?;
            Ok((
                ValueType::Shape {
                    offset,
                    first_child,
                    shape_id,
                    count,
                    frame_end: Some(frame_end),
                },
                frame_end,
            ))
        }

        // Sequential shape
        format::SEQSHAPE => {
            if !tables.has_shape_table {
                return Err(ErrorCode::ReadError);
            }
            let shape_id = read_container_index(&mut reader)?;
            let keys = tables
                .shape(shape_id as u64)
                .map_err(|_| ErrorCode::ReadError)?;
            let count = keys.len();
            let first_child = reader.pos;
            validate_children_fit(count, 1, first_child, limit)?;
            Ok((
                ValueType::Shape {
                    offset,
                    first_child,
                    shape_id,
                    count,
                    frame_end: None,
                },
                0,
            ))
        }

        _ => Err(ErrorCode::ReadError),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ValueType {
    Null,
    Bool(bool),
    Number(f64),
    String {
        ptr: usize,
        len: usize,
    },
    Array {
        offset: usize,
        first_child: usize,
        count: usize,
        frame_end: Option<usize>,
    },
    Map {
        offset: usize,
        first_child: usize,
        count: usize,
        frame_end: Option<usize>,
    },
    Shape {
        offset: usize,
        first_child: usize,
        shape_id: usize,
        count: usize,
        frame_end: Option<usize>,
    },
}

/// Skip a value at `position` within `end`, returning the offset immediately
/// after the value. Implements the universal skip rule (SPEC §8).
pub(crate) fn skip_value(
    bytes: &[u8],
    tables: &Tables,
    position: usize,
    end: usize,
    depth: usize,
) -> Result<usize, ErrorCode> {
    if depth > MAX_DEPTH {
        return Err(ErrorCode::ReadError);
    }
    if position >= end || end > bytes.len() {
        return Err(ErrorCode::ReadError);
    }

    // Length-framed values (including the common non-empty map/array forms)
    // can be skipped directly. Avoid constructing a Reader and then checking
    // all sequential-container tags only to have value_extent parse the tag a
    // second time.
    let tag = bytes[position];
    if let Some(width) = format::length_width(tag) {
        let field_start = position.checked_add(1).ok_or(ErrorCode::ReadError)?;
        let field_end = field_start.checked_add(width).ok_or(ErrorCode::ReadError)?;
        if field_end > end {
            return Err(ErrorCode::ReadError);
        }
        let payload_len = match width {
            1 => bytes[field_start] as usize,
            2 => u16::from_le_bytes([bytes[field_start], bytes[field_start + 1]]) as usize,
            4 => u32::from_le_bytes([
                bytes[field_start],
                bytes[field_start + 1],
                bytes[field_start + 2],
                bytes[field_start + 3],
            ]) as usize,
            _ => return Err(ErrorCode::ReadError),
        };
        let value_end = field_end
            .checked_add(payload_len)
            .ok_or(ErrorCode::ReadError)?;
        return (value_end <= end)
            .then_some(value_end)
            .ok_or(ErrorCode::ReadError);
    }

    let mut reader = Reader::new(bytes);
    reader.pos = position;
    reader.limit = end;

    let tag = reader.peek_u8().map_err(|_| ErrorCode::ReadError)?;

    // Check for sequential containers that must be walked
    let child_count = if format::is_seqfixarray(tag) {
        reader.pos += 1;
        Some(format::seqfixarray_count(tag))
    } else if format::is_seqfixmap(tag) {
        reader.pos += 1;
        format::seqfixmap_count(tag)
            .checked_mul(2)
            .ok_or(ErrorCode::ReadError)?
            .into()
    } else {
        match tag {
            format::SEQARRAY => {
                reader.pos += 1;
                Some(read_container_index(&mut reader)?)
            }
            format::SEQMAP => {
                reader.pos += 1;
                let pairs = read_container_index(&mut reader)?;
                Some(pairs.checked_mul(2).ok_or(ErrorCode::ReadError)?)
            }
            format::SEQSHAPE => {
                reader.pos += 1;
                let shape_id = read_container_index(&mut reader)?;
                let keys = tables
                    .shape(shape_id as u64)
                    .map_err(|_| ErrorCode::ReadError)?;
                Some(keys.len())
            }
            _ => None,
        }
    };

    if let Some(count) = child_count {
        let child_depth = depth.checked_add(1).ok_or(ErrorCode::ReadError)?;
        let mut pos = reader.pos;
        for _ in 0..count {
            pos = skip_value(bytes, tables, pos, end, child_depth)?;
        }
        Ok(pos)
    } else {
        // Use Reader's value_extent for class-A and class-B values
        let extent = reader
            .value_extent(position, end)
            .map_err(|_| ErrorCode::ReadError)?;
        position.checked_add(extent).ok_or(ErrorCode::ReadError)
    }
}

fn container_meta_from_value(vtype: ValueType, limit: usize) -> Option<ContainerMeta> {
    match vtype {
        ValueType::Array {
            offset,
            first_child,
            count,
            frame_end,
        } => Some(ContainerMeta {
            offset,
            first_child,
            count,
            end: frame_end.unwrap_or(limit),
            kind: ContainerKind::Array,
        }),
        ValueType::Map {
            offset,
            first_child,
            count,
            frame_end,
        } => Some(ContainerMeta {
            offset,
            first_child,
            count,
            end: frame_end.unwrap_or(limit),
            kind: ContainerKind::Map,
        }),
        ValueType::Shape {
            offset,
            first_child,
            shape_id,
            count,
            frame_end,
        } => Some(ContainerMeta {
            offset,
            first_child,
            count,
            end: frame_end.unwrap_or(limit),
            kind: ContainerKind::Shape(shape_id),
        }),
        _ => None,
    }
}

/// Retain metadata obtained while encoding a returned container so its next
/// accessor does not have to decode the same container again.
pub(crate) fn memoize_container(vtype: ValueType, limit: usize, cursor_memo: &mut CursorMemo) {
    if let Some(meta) = container_meta_from_value(vtype, limit) {
        cursor_memo.insert_container(meta);
    }
}

fn container_meta(
    bytes: &[u8],
    tables: &Tables,
    offset: usize,
    limit: usize,
    cursor_memo: &mut CursorMemo,
    long_strs: &mut LongStringLens,
) -> Result<ContainerMeta, ErrorCode> {
    if let Some(meta) = cursor_memo.container(offset) {
        return Ok(meta);
    }
    let (vtype, _) = decode_value(bytes, tables, offset, limit, long_strs)?;
    let meta = container_meta_from_value(vtype, limit).ok_or(ErrorCode::NotIndexable)?;
    cursor_memo.insert_container(meta);
    Ok(meta)
}

/// Navigate to the child at `index` within a container starting at `offset`.
/// Returns the child's offset within `limit`.
pub(crate) fn navigate_to_child(
    bytes: &[u8],
    tables: &Tables,
    offset: usize,
    index: usize,
    limit: usize,
    cursor_memo: &mut CursorMemo,
    long_strs: &mut LongStringLens,
) -> Result<usize, ErrorCode> {
    let meta = container_meta(bytes, tables, offset, limit, cursor_memo, long_strs)?;
    if index >= meta.count {
        return Err(ErrorCode::IndexOutOfBounds);
    }
    let map = meta.kind == ContainerKind::Map;

    if let Some(cached) = cursor_memo.get(offset, index) {
        return Ok(cached);
    }

    let (mut current_index, mut pos) =
        if let Some(cached) = cursor_memo.find_preceding(offset, index) {
            cached
        } else {
            let first_value = if map {
                map_value_after_key(bytes, tables, meta.first_child, meta.end, long_strs)?
            } else {
                meta.first_child
            };
            (0, first_value)
        };

    while current_index < index {
        pos = skip_value(bytes, tables, pos, meta.end, 0)?;
        if map {
            // Map children exposed to the guest are values, not key/value slots.
            pos = map_value_after_key(bytes, tables, pos, meta.end, long_strs)?;
        }
        current_index += 1;
        if current_index % 4 == 0 {
            cursor_memo.insert(offset, current_index, pos);
        }
    }

    cursor_memo.insert(offset, index, pos);
    Ok(pos)
}

/// Find the offset of the first child within a container at `offset`.
fn find_first_child(bytes: &[u8], offset: usize, limit: usize) -> Result<usize, ErrorCode> {
    if offset >= limit || limit > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    let mut reader = Reader::new(bytes);
    reader.pos = offset;
    reader.limit = limit;

    let tag = reader.read_u8().map_err(|_| ErrorCode::ReadError)?;

    // Empty containers have no children
    if tag == format::FIXARRAY0 || tag == format::FIXMAP0 {
        return Err(ErrorCode::IndexOutOfBounds);
    }

    // Fixed sequential forms: first child immediately follows tag
    if format::is_seqfixarray(tag) || format::is_seqfixmap(tag) {
        return Ok(reader.pos);
    }

    // Fixed length-framed forms: skip 1-byte length
    if format::is_fixarray(tag) || format::is_fixmap(tag) {
        reader.read_u8().map_err(|_| ErrorCode::ReadError)?; // skip length byte
        return Ok(reader.pos);
    }

    // Sequential non-fixed: skip count/shape-id varint
    if matches!(tag, format::SEQARRAY | format::SEQMAP | format::SEQSHAPE) {
        reader.read_varint().map_err(|_| ErrorCode::ReadError)?;
        return Ok(reader.pos);
    }

    // Length-framed non-fixed: skip length field, then count/shape-id varint
    if let Some(width) = format::length_width(tag) {
        reader
            .read_uint_le(width)
            .map_err(|_| ErrorCode::ReadError)?;
        reader.read_varint().map_err(|_| ErrorCode::ReadError)?;
        return Ok(reader.pos);
    }

    Err(ErrorCode::ReadError)
}

fn decode_map_key(
    bytes: &[u8],
    tables: &Tables,
    key_offset: usize,
    end: usize,
    long_strs: &mut LongStringLens,
) -> Result<(Span, usize), ErrorCode> {
    if key_offset >= end || end > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    let tag = bytes[key_offset];
    if format::is_fixstr(tag) {
        let len = format::fixstr_len(tag);
        let start = key_offset + 1;
        let encoded_end = start.checked_add(len).ok_or(ErrorCode::ReadError)?;
        if encoded_end > end {
            return Err(ErrorCode::ReadError);
        }
        return Ok((Span { start, len }, encoded_end));
    }

    let (key_type, encoded_end) = decode_value(bytes, tables, key_offset, end, long_strs)?;
    match key_type {
        ValueType::String { ptr, len } => Ok((Span { start: ptr, len }, encoded_end)),
        _ => Err(ErrorCode::ReadError),
    }
}

/// Validate a map key and return the offset of the value that follows it.
fn map_value_after_key(
    bytes: &[u8],
    tables: &Tables,
    key_offset: usize,
    end: usize,
    long_strs: &mut LongStringLens,
) -> Result<usize, ErrorCode> {
    decode_map_key(bytes, tables, key_offset, end, long_strs).map(|(_, value_offset)| value_offset)
}

/// Look up a map/shape property by key name. Returns the value offset if found.
pub(crate) fn lookup_property(
    bytes: &[u8],
    tables: &Tables,
    offset: usize,
    key: &[u8],
    limit: usize,
    cursor_memo: &mut CursorMemo,
    long_strs: &mut LongStringLens,
) -> Result<Option<usize>, ErrorCode> {
    let meta = container_meta(bytes, tables, offset, limit, cursor_memo, long_strs)?;

    match meta.kind {
        ContainerKind::Map => {
            let mut pos = meta.first_child;

            for i in 0..meta.count {
                let (key_span, value_pos) =
                    decode_map_key(bytes, tables, pos, meta.end, long_strs)?;
                let key_bytes = span_bytes(bytes, key_span).ok_or(ErrorCode::ReadError)?;

                if key_bytes == key {
                    cursor_memo.insert(offset, i, value_pos);
                    return Ok(Some(value_pos));
                }

                pos = skip_value(bytes, tables, value_pos, meta.end, 0)?;
            }
            Ok(None)
        }

        ContainerKind::Shape(shape_id) => {
            let keys = tables
                .shape(shape_id as u64)
                .map_err(|_| ErrorCode::ReadError)?;
            let recent = cursor_memo
                .shape_key(shape_id)
                .filter(|&idx| idx < keys.len());
            let mut key_idx = recent.filter(|&idx| span_bytes(bytes, keys[idx]) == Some(key));

            if key_idx.is_none() && !keys.is_empty() {
                let start = recent.map_or(0, |idx| (idx + 1) % keys.len());
                for step in 0..keys.len() {
                    let idx = (start + step) % keys.len();
                    if Some(idx) != recent && span_bytes(bytes, keys[idx]) == Some(key) {
                        key_idx = Some(idx);
                        break;
                    }
                }
            }

            match key_idx {
                Some(idx) => {
                    cursor_memo.insert_shape_key(shape_id, idx);
                    let value_pos = navigate_to_child(
                        bytes,
                        tables,
                        offset,
                        idx,
                        limit,
                        cursor_memo,
                        long_strs,
                    )?;
                    Ok(Some(value_pos))
                }
                None => Ok(None),
            }
        }

        ContainerKind::Array => Err(ErrorCode::NotAnObject),
    }
}

/// Retrieve the key at `index` within a map or shape.
pub(crate) fn get_key_at_index(
    bytes: &[u8],
    tables: &Tables,
    offset: usize,
    index: usize,
    limit: usize,
    long_strs: &mut LongStringLens,
) -> Result<(usize, usize), ErrorCode> {
    let (vtype, _) = decode_value(bytes, tables, offset, limit, long_strs)?;

    match vtype {
        ValueType::Map {
            count, frame_end, ..
        } => {
            if index >= count {
                return Err(ErrorCode::IndexOutOfBounds);
            }
            let first_child = find_first_child(bytes, offset, limit)?;
            let end = frame_end.unwrap_or(limit);
            let mut pos = first_child;

            for i in 0..=index {
                let (key_type, key_end) = decode_value(bytes, tables, pos, end, long_strs)?;
                let key_span = match key_type {
                    ValueType::String { ptr, len } => (ptr, len),
                    _ => return Err(ErrorCode::ReadError),
                };
                if i == index {
                    return Ok(key_span);
                }
                let value_pos = if key_end == 0 {
                    skip_value(bytes, tables, pos, end, 0)?
                } else {
                    key_end
                };
                pos = skip_value(bytes, tables, value_pos, end, 0)?;
            }
            Err(ErrorCode::IndexOutOfBounds)
        }

        ValueType::Shape {
            shape_id, count, ..
        } => {
            if index >= count {
                return Err(ErrorCode::IndexOutOfBounds);
            }
            let keys = tables
                .shape(shape_id as u64)
                .map_err(|_| ErrorCode::ReadError)?;
            let span = keys[index];
            long_strs.insert(span.start, span.len);
            Ok((span.start, span.len))
        }

        _ => Err(ErrorCode::NotAnObject),
    }
}

/// Helper to read a usize from a Reader with checked conversion
fn read_usize(reader: &mut Reader<'_>, width: usize) -> Result<usize, ErrorCode> {
    let value = reader
        .read_uint_le(width)
        .map_err(|_| ErrorCode::ReadError)?;
    usize::try_from(value).map_err(|_| ErrorCode::ReadError)
}

/// Helper to read a container count/id with the 2^32-1 limit
fn read_container_index(reader: &mut Reader<'_>) -> Result<usize, ErrorCode> {
    let value = reader
        .read_container_index()
        .map_err(|_| ErrorCode::ReadError)?;
    usize::try_from(value).map_err(|_| ErrorCode::ReadError)
}

/// Helper to read a frame end offset
fn read_frame_end(reader: &mut Reader<'_>, width: usize) -> Result<usize, ErrorCode> {
    let payload_len = read_usize(reader, width)?;
    let end = reader
        .pos
        .checked_add(payload_len)
        .ok_or(ErrorCode::ReadError)?;
    if end > reader.limit || end > reader.data.len() {
        return Err(ErrorCode::ReadError);
    }
    Ok(end)
}

/// Validate that a container has space for at least `count` children
fn validate_children_fit(
    count: usize,
    values_per_item: usize,
    position: usize,
    end: usize,
) -> Result<(), ErrorCode> {
    let minimum = count
        .checked_mul(values_per_item)
        .ok_or(ErrorCode::ReadError)?;
    if minimum > end.saturating_sub(position) {
        return Err(ErrorCode::ReadError);
    }
    Ok(())
}

/// Helper to safely extract bytes from a span
fn span_bytes(data: &[u8], span: Span) -> Option<&[u8]> {
    let end = span.start.checked_add(span.len)?;
    data.get(span.start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fbf::read::{parse_header, parse_prelude};

    fn parse_root(bytes: &[u8]) -> (Tables, usize) {
        let mut reader = Reader::new(bytes);
        let header = parse_header(&mut reader).unwrap();
        let tables = parse_prelude(&mut reader, header).unwrap();
        (tables, reader.pos)
    }

    fn payload(root: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::from(format::MAGIC);
        bytes.push(format::VERSION);
        bytes.push(0);
        bytes.extend_from_slice(root);
        bytes
    }

    #[test]
    fn decodes_scalars() {
        let mut long_strs = LongStringLens::default();

        // Null
        let bytes = payload(&[format::NIL]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, end) =
            decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        assert_eq!(vtype, ValueType::Null);
        assert_eq!(end, bytes.len());

        // Bool
        let bytes = payload(&[format::TRUE]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, _) = decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        assert_eq!(vtype, ValueType::Bool(true));

        // Fixint
        let bytes = payload(&[42]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, _) = decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        assert_eq!(vtype, ValueType::Number(42.0));

        // Negative fixint
        let bytes = payload(&[0xff]); // -1
        let (tables, root) = parse_root(&bytes);
        let (vtype, _) = decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        assert_eq!(vtype, ValueType::Number(-1.0));
    }

    #[test]
    fn decodes_strings_and_strrefs() {
        let mut long_strs = LongStringLens::default();

        // Fixstr
        let bytes = payload(&[format::FIXSTR_MIN + 4, b'h', b'i', b'!', b'x']);
        let (tables, root) = parse_root(&bytes);
        let (vtype, end) =
            decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        match vtype {
            ValueType::String { ptr, len } => {
                assert_eq!(len, 4);
                assert_eq!(&bytes[ptr..ptr + len], b"hi!x");
            }
            _ => panic!("expected string"),
        }
        assert_eq!(end, bytes.len());

        // str8
        let bytes = payload(&[format::STR8, 2, b'h', b'i']);
        let (tables, root) = parse_root(&bytes);
        let (vtype, _) = decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        match vtype {
            ValueType::String { ptr, len } => {
                assert_eq!(len, 2);
                assert_eq!(&bytes[ptr..ptr + len], b"hi");
            }
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn decodes_arrays() {
        let mut long_strs = LongStringLens::default();

        // Empty array
        let bytes = payload(&[format::FIXARRAY0]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, end) =
            decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        assert!(matches!(vtype, ValueType::Array { count: 0, .. }));
        assert_eq!(end, bytes.len());

        // fixarray3 length-framed
        let bytes = payload(&[0xd3, 3, 1, 2, 3]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, end) =
            decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        match vtype {
            ValueType::Array {
                count, frame_end, ..
            } => {
                assert_eq!(count, 3);
                assert_eq!(frame_end, Some(bytes.len()));
            }
            _ => panic!("expected array"),
        }
        assert_eq!(end, bytes.len());

        // seqfixarray3
        let bytes = payload(&[0xc4, 1, 2, 3]);
        let (tables, root) = parse_root(&bytes);
        let (vtype, end) =
            decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).unwrap();
        match vtype {
            ValueType::Array {
                count, frame_end, ..
            } => {
                assert_eq!(count, 3);
                assert_eq!(frame_end, None);
            }
            _ => panic!("expected array"),
        }
        assert_eq!(end, 0); // sequential must be computed
    }

    #[test]
    fn skips_values_correctly() {
        // Skip scalar
        let bytes = payload(&[1, 2, 3]);
        let (tables, root) = parse_root(&bytes);
        let next = skip_value(&bytes, &tables, root, bytes.len(), 0).unwrap();
        assert_eq!(next, root + 1);

        // Skip fixarray
        let bytes = payload(&[0xd3, 3, 1, 2, 3]);
        let (tables, root) = parse_root(&bytes);
        let next = skip_value(&bytes, &tables, root, bytes.len(), 0).unwrap();
        assert_eq!(next, bytes.len());

        // Skip seqarray
        let bytes = payload(&[format::SEQFIXARRAY_MIN + 2, 1, 2, 3]);
        let (tables, root) = parse_root(&bytes);
        let next = skip_value(&bytes, &tables, root, bytes.len(), 0).unwrap();
        assert_eq!(next, bytes.len());
    }

    #[test]
    fn navigates_to_array_children() {
        let mut cursor = CursorMemo::default();
        let mut long_strs = LongStringLens::default();

        let bytes = payload(&[0xd3, 3, 1, 2, 3]);
        let (tables, root) = parse_root(&bytes);

        let child0 = navigate_to_child(
            &bytes,
            &tables,
            root,
            0,
            bytes.len(),
            &mut cursor,
            &mut long_strs,
        )
        .unwrap();
        let child1 = navigate_to_child(
            &bytes,
            &tables,
            root,
            1,
            bytes.len(),
            &mut cursor,
            &mut long_strs,
        )
        .unwrap();
        let child2 = navigate_to_child(
            &bytes,
            &tables,
            root,
            2,
            bytes.len(),
            &mut cursor,
            &mut long_strs,
        )
        .unwrap();

        assert_eq!(bytes[child0], 1);
        assert_eq!(bytes[child1], 2);
        assert_eq!(bytes[child2], 3);

        // Out of bounds
        assert!(navigate_to_child(
            &bytes,
            &tables,
            root,
            3,
            bytes.len(),
            &mut cursor,
            &mut long_strs
        )
        .is_err());
    }

    #[test]
    fn cursor_memo_caches_positions() {
        let mut cursor = CursorMemo::default();
        cursor.insert(100, 5, 200);

        assert_eq!(cursor.get(100, 5), Some(200));
        assert_eq!(cursor.get(100, 4), None);
        assert_eq!(cursor.find_preceding(100, 6), Some((5, 200)));
    }

    #[test]
    fn long_string_lens_cache() {
        let mut cache = LongStringLens::default();

        // Lengths represented directly in the NanBox need no side entry.
        cache.insert(100, NanBox::MAX_VALUE_LENGTH - 1);
        assert_eq!(cache.get(100), None);

        // Lengths truncated in the NanBox are retained for the input lifetime.
        cache.insert(100, NanBox::MAX_VALUE_LENGTH);
        assert_eq!(cache.get(100), Some(NanBox::MAX_VALUE_LENGTH));
        for i in 0..5 {
            cache.insert(1000 + i * 10, NanBox::MAX_VALUE_LENGTH + 100 + i);
        }
        assert_eq!(
            cache.get(100),
            Some(NanBox::MAX_VALUE_LENGTH),
            "older long-string lengths must not be evicted",
        );
    }

    #[test]
    fn validates_container_boundaries() {
        let mut long_strs = LongStringLens::default();

        // fixarray2 with only 1 byte of payload
        let bytes = payload(&[0xd2, 1, 1]);
        let (tables, root) = parse_root(&bytes);
        assert!(decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).is_err());

        // array with frame end beyond input
        let bytes = payload(&[format::ARRAY8, 100, 5, 1, 2]);
        let (tables, root) = parse_root(&bytes);
        assert!(decode_value(&bytes, &tables, root, bytes.len(), &mut long_strs).is_err());
    }

    #[test]
    fn rejects_non_string_map_keys() {
        let mut cursor = CursorMemo::default();
        let mut long_strs = LongStringLens::default();

        // Map with integer key
        let bytes = payload(&[0xd9, 2, 1, 2]);
        let (tables, root) = parse_root(&bytes);

        // Lookup should fail when it encounters the non-string key
        let result = lookup_property(
            &bytes,
            &tables,
            root,
            b"x",
            bytes.len(),
            &mut cursor,
            &mut long_strs,
        );
        assert!(result.is_err());
    }

    #[test]
    fn enforces_max_depth() {
        // Create deeply nested array: [[[[...]]]]]
        let mut bytes = Vec::from(format::MAGIC);
        bytes.push(format::VERSION);
        bytes.push(0);

        bytes.extend_from_slice(&[0xc2; 130]); // seqfixarray1
        bytes.push(1); // innermost value

        let (tables, root) = parse_root(&bytes);
        assert!(skip_value(&bytes, &tables, root, bytes.len(), 0).is_err());
    }
}
