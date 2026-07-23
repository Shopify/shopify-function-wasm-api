//! Allocation-free navigation over an FBF input payload.

use fbf::format;
use shopify_function_wasm_api_core::read::{ErrorCode, NanBox};
use std::collections::HashMap;

const MAX_DEPTH: u32 = 128;
const CONTAINER_CACHE_LEN: usize = 8;
const CURSOR_CACHE_LEN: usize = 4;
const INVALID_OFFSET: u32 = u32::MAX;

type Result<T> = std::result::Result<T, ErrorCode>;

#[derive(Debug, Default)]
pub(crate) struct InputState {
    pub(crate) root: u32,
    pub(crate) strings: Vec<(u32, u32)>,
    pub(crate) shape_keys: Vec<(u32, u32)>,
    pub(crate) shapes: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    Array,
    Map,
    Shape,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ContainerMeta {
    pub(crate) tag_offset: u32,
    pub(crate) kind: ContainerKind,
    pub(crate) count: u32,
    pub(crate) first_child: u32,
    pub(crate) end: Option<u32>,
    pub(crate) shape_id: u32,
}

const EMPTY_META: ContainerMeta = ContainerMeta {
    tag_offset: INVALID_OFFSET,
    kind: ContainerKind::Array,
    count: 0,
    first_child: 0,
    end: None,
    shape_id: 0,
};

#[derive(Clone, Copy, Debug)]
struct Cursor {
    container: u32,
    next_index: u32,
    next_pos: u32,
}

const EMPTY_CURSOR: Cursor = Cursor {
    container: INVALID_OFFSET,
    next_index: 0,
    next_pos: 0,
};

pub(crate) struct Caches {
    containers: [ContainerMeta; CONTAINER_CACHE_LEN],
    cursors: [Cursor; CURSOR_CACHE_LEN],
    cursor_victim: usize,
    shape_lookup: Vec<u32>,
    long_string_lens: HashMap<u32, u32>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            containers: [EMPTY_META; CONTAINER_CACHE_LEN],
            cursors: [EMPTY_CURSOR; CURSOR_CACHE_LEN],
            cursor_victim: 0,
            shape_lookup: Vec::new(),
            long_string_lens: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TagClass {
    AFixed,
    BLength,
    CSequential,
    String,
    Scalar,
    Reserved,
}

#[derive(Clone, Copy, Debug)]
struct TagInfo {
    class: TagClass,
    arg: u8,
}

const RESERVED_INFO: TagInfo = TagInfo {
    class: TagClass::Reserved,
    arg: 0,
};

const fn build_tag_info() -> [TagInfo; 256] {
    let mut table = [RESERVED_INFO; 256];
    let mut i = 0;
    while i < 256 {
        let tag = i as u8;
        table[i] = classify_tag(tag);
        i += 1;
    }
    table
}

static TAG_INFO: [TagInfo; 256] = build_tag_info();

const fn info(class: TagClass, arg: u8) -> TagInfo {
    TagInfo { class, arg }
}

const fn classify_tag(tag: u8) -> TagInfo {
    match tag {
        0x00..=0x7f | 0xe0..=0xff => info(TagClass::Scalar, 1),
        format::NIL | format::FALSE | format::TRUE => info(TagClass::Scalar, 1),
        format::INT8 | format::UINT8 => info(TagClass::Scalar, 2),
        format::INT16 | format::UINT16 => info(TagClass::Scalar, 3),
        format::INT32 | format::UINT32 | format::FLOAT32 => info(TagClass::Scalar, 5),
        format::INT64 | format::UINT64 | format::FLOAT64 => info(TagClass::Scalar, 9),
        format::STR8 => info(TagClass::String, 0x81),
        format::STR16 => info(TagClass::String, 0x82),
        format::STR32 => info(TagClass::String, 0x84),
        format::STRREF8 => info(TagClass::String, 2),
        format::STRREF16 => info(TagClass::String, 3),
        format::STRREF32 => info(TagClass::String, 5),
        format::FIXSTR_MIN..=format::FIXSTR_MAX => {
            info(TagClass::String, tag - format::FIXSTR_MIN + 1)
        }
        format::ARRAY8 | format::MAP8 | format::SHAPE8 => info(TagClass::BLength, 1),
        format::ARRAY16 | format::MAP16 | format::SHAPE16 => info(TagClass::BLength, 2),
        format::ARRAY32 | format::MAP32 | format::SHAPE32 => info(TagClass::BLength, 4),
        format::SEQARRAY
        | format::SEQMAP
        | format::SEQSHAPE
        | format::SEQFIXARRAY_MIN..=format::SEQFIXMAP_MAX => info(TagClass::CSequential, 0),
        format::FIXARRAY0 | format::FIXMAP0 => info(TagClass::AFixed, 1),
        0xd1..=0xd7 | 0xd9..=0xdf => info(TagClass::BLength, 1),
        _ => RESERVED_INFO,
    }
}

pub(crate) fn reset_caches_for_state(caches: &mut Caches, state: &InputState) {
    *caches = Caches::default();
    caches
        .shape_lookup
        .resize(state.shapes.len(), INVALID_OFFSET);
}

#[inline]
fn read_le(bytes: &[u8], pos: u32, width: u8, end: u32) -> Result<u32> {
    let start = pos as usize;
    let stop = start
        .checked_add(width as usize)
        .ok_or(ErrorCode::ReadError)?;
    if stop > end as usize || stop > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    Ok(match width {
        1 => bytes[start] as u32,
        2 => u16::from_le_bytes([bytes[start], bytes[start + 1]]) as u32,
        4 => u32::from_le_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
        ]),
        _ => return Err(ErrorCode::ReadError),
    })
}

#[inline]
fn checked_add(a: u32, b: u32) -> Result<u32> {
    a.checked_add(b).ok_or(ErrorCode::ReadError)
}

#[inline]
fn read_var_u32(bytes: &[u8], pos: &mut u32, end: u32) -> Result<u32> {
    if *pos > end || end as usize > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    let mut cursor = *pos as usize;
    let value = fbf::varint::read(&bytes[..end as usize], &mut cursor, false)
        .map_err(|_| ErrorCode::ReadError)?;
    let value = u32::try_from(value).map_err(|_| ErrorCode::ReadError)?;
    *pos = u32::try_from(cursor).map_err(|_| ErrorCode::ReadError)?;
    Ok(value)
}

#[inline]
fn bounded_span(bytes: &[u8], start: u32, len: u32, end: u32) -> Result<(u32, u32)> {
    let stop = checked_add(start, len)?;
    if stop > end || stop as usize > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    Ok((start, stop))
}

fn string_span_at(
    bytes: &[u8],
    state: &InputState,
    pos: u32,
    end: u32,
) -> Result<((u32, u32), u32)> {
    let tag = *bytes.get(pos as usize).ok_or(ErrorCode::ReadError)?;
    if pos >= end {
        return Err(ErrorCode::ReadError);
    }
    match tag {
        format::FIXSTR_MIN..=format::FIXSTR_MAX => {
            let len = (tag - format::FIXSTR_MIN) as u32;
            let content = checked_add(pos, 1)?;
            let (_, next) = bounded_span(bytes, content, len, end)?;
            Ok(((content, len), next))
        }
        format::STR8 | format::STR16 | format::STR32 => {
            let width = TAG_INFO[tag as usize].arg & 0x7f;
            let len_pos = checked_add(pos, 1)?;
            let len = read_le(bytes, len_pos, width, end)?;
            let content = checked_add(len_pos, width as u32)?;
            let (_, next) = bounded_span(bytes, content, len, end)?;
            Ok(((content, len), next))
        }
        format::STRREF8 | format::STRREF16 | format::STRREF32 => {
            let width = TAG_INFO[tag as usize].arg - 1;
            let id_pos = checked_add(pos, 1)?;
            let id = read_le(bytes, id_pos, width, end)?;
            let next = checked_add(id_pos, width as u32)?;
            let span = state
                .strings
                .get(id as usize)
                .copied()
                .ok_or(ErrorCode::ReadError)?;
            Ok((span, next))
        }
        _ => Err(ErrorCode::ReadError),
    }
}

#[inline]
fn read_number(bytes: &[u8], pos: u32, tag: u8, end: u32) -> Result<f64> {
    let width = match tag {
        format::INT8 | format::UINT8 => 1,
        format::INT16 | format::UINT16 => 2,
        format::INT32 | format::UINT32 | format::FLOAT32 => 4,
        format::INT64 | format::UINT64 | format::FLOAT64 => 8,
        _ => return Err(ErrorCode::ReadError),
    };
    let start = checked_add(pos, 1)?;
    bounded_span(bytes, start, width, end)?;
    let start = start as usize;
    let value = match tag {
        format::INT8 => (bytes[start] as i8) as f64,
        format::INT16 => i16::from_le_bytes([bytes[start], bytes[start + 1]]) as f64,
        format::INT32 => i32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as f64,
        format::INT64 => i64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()) as f64,
        format::UINT8 => bytes[start] as f64,
        format::UINT16 => u16::from_le_bytes([bytes[start], bytes[start + 1]]) as f64,
        format::UINT32 => u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as f64,
        format::UINT64 => u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()) as f64,
        format::FLOAT32 => f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as f64,
        format::FLOAT64 => f64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()),
        _ => unreachable!(),
    };
    if value.is_nan() {
        Err(ErrorCode::ReadError)
    } else {
        Ok(value)
    }
}

#[inline]
pub(crate) fn decode_value(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    pos: u32,
) -> Result<NanBox> {
    let end = u32::try_from(bytes.len()).map_err(|_| ErrorCode::ReadError)?;
    let tag = *bytes.get(pos as usize).ok_or(ErrorCode::ReadError)?;
    let value = match tag {
        0x00..=0x7f => NanBox::number(tag as f64),
        0xe0..=0xff => NanBox::number((tag as i8) as f64),
        format::NIL => NanBox::null(),
        format::FALSE => NanBox::bool(false),
        format::TRUE => NanBox::bool(true),
        format::INT8..=format::FLOAT64 => NanBox::number(read_number(bytes, pos, tag, end)?),
        format::FIXSTR_MIN..=format::FIXSTR_MAX
        | format::STR8
        | format::STR16
        | format::STR32
        | format::STRREF8
        | format::STRREF16
        | format::STRREF32 => {
            let ((content, len), _) = string_span_at(bytes, state, pos, end)?;
            if len >= NanBox::MAX_VALUE_LENGTH as u32 {
                caches.long_string_lens.insert(content, len);
            }
            NanBox::string(content as usize, len as usize)
        }
        _ => {
            let meta = container_meta(bytes, state, caches, pos)?;
            match meta.kind {
                ContainerKind::Array => NanBox::array(pos as usize, meta.count as usize),
                ContainerKind::Map | ContainerKind::Shape => {
                    NanBox::obj(pos as usize, meta.count as usize)
                }
            }
        }
    };
    Ok(value)
}

pub(crate) fn parse_input(bytes: &[u8]) -> Result<InputState> {
    let input_len = u32::try_from(bytes.len()).map_err(|_| ErrorCode::ReadError)?;
    if bytes.len() < 5
        || bytes[..3] != format::MAGIC
        || bytes[3] != format::VERSION
        || bytes[4] & format::FLAG_RESERVED_MASK != 0
    {
        return Err(ErrorCode::ReadError);
    }

    let flags = bytes[4];
    let mut pos = 5u32;
    let mut state = InputState::default();

    if flags & format::FLAG_STRING_TABLE != 0 {
        let count = read_var_u32(bytes, &mut pos, input_len)? as usize;
        let remaining = (input_len - pos) as usize;
        state.strings = Vec::with_capacity(count.min(remaining));
        if count > remaining {
            return Err(ErrorCode::ReadError);
        }
        for _ in 0..count {
            let len = read_var_u32(bytes, &mut pos, input_len)?;
            let content = pos;
            let (_, next) = bounded_span(bytes, content, len, input_len)?;
            state.strings.push((content, len));
            pos = next;
        }
    }

    if flags & format::FLAG_SHAPE_TABLE != 0 {
        parse_shapes(bytes, input_len, &mut pos, &mut state)?;
    }
    if pos >= input_len {
        return Err(ErrorCode::ReadError);
    }
    state.root = pos;
    Ok(state)
}

fn parse_shapes(bytes: &[u8], input_len: u32, pos: &mut u32, state: &mut InputState) -> Result<()> {
    let count = read_var_u32(bytes, pos, input_len)? as usize;
    let remaining = (input_len - *pos) as usize;
    state.shapes = Vec::with_capacity(count.min(remaining));
    if count > remaining {
        return Err(ErrorCode::ReadError);
    }
    for _ in 0..count {
        let key_count = read_var_u32(bytes, pos, input_len)?;
        if key_count > input_len - *pos {
            return Err(ErrorCode::ReadError);
        }
        let start = u32::try_from(state.shape_keys.len()).map_err(|_| ErrorCode::ReadError)?;
        state
            .shape_keys
            .reserve((key_count as usize).min((input_len - *pos) as usize));
        for _ in 0..key_count {
            let (span, next) = string_span_at(bytes, state, *pos, input_len)?;
            state.shape_keys.push(span);
            *pos = next;
        }
        state.shapes.push((start, key_count));
    }
    Ok(())
}

#[inline]
fn framed_payload(bytes: &[u8], pos: u32, width: u8, limit: u32) -> Result<(u32, u32)> {
    let len_pos = checked_add(pos, 1)?;
    let len = read_le(bytes, len_pos, width, limit)?;
    let payload = checked_add(len_pos, width as u32)?;
    let (_, end) = bounded_span(bytes, payload, len, limit)?;
    Ok((payload, end))
}

#[inline]
fn validate_child_minimum(meta: &ContainerMeta, limit: u32) -> Result<()> {
    let end = meta.end.unwrap_or(limit);
    let slots = if meta.kind == ContainerKind::Map {
        meta.count.checked_mul(2).ok_or(ErrorCode::ReadError)?
    } else {
        meta.count
    };
    if meta.first_child > end || slots > end - meta.first_child {
        return Err(ErrorCode::ReadError);
    }
    Ok(())
}

#[inline]
pub(crate) fn container_meta(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    pos: u32,
) -> Result<ContainerMeta> {
    let slot = pos as usize & (CONTAINER_CACHE_LEN - 1);
    if caches.containers[slot].tag_offset == pos {
        return Ok(caches.containers[slot]);
    }
    let limit = u32::try_from(bytes.len()).map_err(|_| ErrorCode::ReadError)?;
    let tag = *bytes.get(pos as usize).ok_or(ErrorCode::ReadError)?;
    let mut meta = ContainerMeta {
        tag_offset: pos,
        kind: ContainerKind::Array,
        count: 0,
        first_child: checked_add(pos, 1)?,
        end: None,
        shape_id: 0,
    };
    match tag {
        format::FIXARRAY0 => meta.end = Some(meta.first_child),
        format::FIXMAP0 => {
            meta.kind = ContainerKind::Map;
            meta.end = Some(meta.first_child);
        }
        0xd1..=0xd7 | 0xd9..=0xdf => {
            let (first, end) = framed_payload(bytes, pos, 1, limit)?;
            meta.first_child = first;
            meta.end = Some(end);
            if tag <= format::FIXARRAY_MAX {
                meta.count = (tag - format::FIXARRAY0) as u32;
            } else {
                meta.kind = ContainerKind::Map;
                meta.count = (tag - format::FIXMAP0) as u32;
            }
        }
        format::SEQFIXARRAY_MIN..=format::SEQFIXARRAY_MAX => {
            meta.count = (tag - format::SEQFIXARRAY_MIN + 1) as u32;
        }
        format::SEQFIXMAP_MIN..=format::SEQFIXMAP_MAX => {
            meta.kind = ContainerKind::Map;
            meta.count = (tag - format::SEQFIXMAP_MIN + 1) as u32;
        }
        format::ARRAY8..=format::ARRAY32 | format::MAP8..=format::MAP32 => {
            let width = TAG_INFO[tag as usize].arg;
            let (mut first, end) = framed_payload(bytes, pos, width, limit)?;
            meta.count = read_var_u32(bytes, &mut first, end)?;
            meta.first_child = first;
            meta.end = Some(end);
            if matches!(tag, format::MAP8..=format::MAP32) {
                meta.kind = ContainerKind::Map;
            }
        }
        format::SEQARRAY | format::SEQMAP => {
            let mut first = meta.first_child;
            meta.count = read_var_u32(bytes, &mut first, limit)?;
            meta.first_child = first;
            if tag == format::SEQMAP {
                meta.kind = ContainerKind::Map;
            }
        }
        format::SHAPE8..=format::SHAPE32 | format::SEQSHAPE => {
            let (mut first, end) = if tag == format::SEQSHAPE {
                (meta.first_child, None)
            } else {
                let width = TAG_INFO[tag as usize].arg;
                let (first, end) = framed_payload(bytes, pos, width, limit)?;
                (first, Some(end))
            };
            let shape_end = end.unwrap_or(limit);
            meta.shape_id = read_var_u32(bytes, &mut first, shape_end)?;
            meta.count = state
                .shapes
                .get(meta.shape_id as usize)
                .map(|shape| shape.1)
                .ok_or(ErrorCode::ReadError)?;
            meta.kind = ContainerKind::Shape;
            meta.first_child = first;
            meta.end = end;
        }
        _ => return Err(ErrorCode::ReadError),
    }
    validate_child_minimum(&meta, limit)?;
    caches.containers[slot] = meta;
    Ok(meta)
}

#[inline]
fn cursor_start(caches: &Caches, container: u32, index: u32, first: u32) -> (u32, u32) {
    caches
        .cursors
        .iter()
        .find(|cursor| cursor.container == container && index >= cursor.next_index)
        .map_or((0, first), |cursor| (cursor.next_index, cursor.next_pos))
}

#[inline]
fn update_cursor(caches: &mut Caches, container: u32, next_index: u32, next_pos: u32) {
    if let Some(cursor) = caches
        .cursors
        .iter_mut()
        .find(|cursor| cursor.container == container)
    {
        *cursor = Cursor {
            container,
            next_index,
            next_pos,
        };
        return;
    }
    let slot = caches.cursor_victim;
    caches.cursors[slot] = Cursor {
        container,
        next_index,
        next_pos,
    };
    caches.cursor_victim = (slot + 1) % CURSOR_CACHE_LEN;
}

#[inline]
fn container_end(meta: ContainerMeta, bytes: &[u8]) -> Result<u32> {
    meta.end.map_or_else(
        || u32::try_from(bytes.len()).map_err(|_| ErrorCode::ReadError),
        Ok,
    )
}

#[inline]
fn skip_map_pair(bytes: &[u8], state: &InputState, key_pos: u32, end: u32) -> Result<(u32, u32)> {
    let (_, value_pos) = string_span_at(bytes, state, key_pos, end)?;
    let next_pair = skip_value(bytes, state, value_pos, end)?;
    Ok((value_pos, next_pair))
}

#[inline]
pub(crate) fn element_at(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    container: u32,
    index: u32,
) -> Result<NanBox> {
    let meta = container_meta(bytes, state, caches, container)?;
    if index >= meta.count {
        return Err(ErrorCode::IndexOutOfBounds);
    }
    let end = container_end(meta, bytes)?;
    let (mut current, mut pos) = cursor_start(caches, container, index, meta.first_child);
    match meta.kind {
        ContainerKind::Array | ContainerKind::Shape => {
            while current < index {
                pos = skip_value(bytes, state, pos, end)?;
                current += 1;
            }
            let next = skip_value(bytes, state, pos, end)?;
            let value = decode_value(bytes, state, caches, pos)?;
            update_cursor(caches, container, index + 1, next);
            Ok(value)
        }
        ContainerKind::Map => {
            while current < index {
                pos = skip_map_pair(bytes, state, pos, end)?.1;
                current += 1;
            }
            let (value_pos, next) = skip_map_pair(bytes, state, pos, end)?;
            let value = decode_value(bytes, state, caches, value_pos)?;
            update_cursor(caches, container, index + 1, next);
            Ok(value)
        }
    }
}

#[inline]
fn span_matches(bytes: &[u8], span: (u32, u32), query: &[u8]) -> bool {
    if span.1 as usize != query.len() {
        return false;
    }
    let start = span.0 as usize;
    let Some(end) = start.checked_add(span.1 as usize) else {
        return false;
    };
    bytes.get(start..end) == Some(query)
}

fn map_find(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    meta: ContainerMeta,
    query: &[u8],
) -> Result<Option<NanBox>> {
    let end = container_end(meta, bytes)?;
    let (mut index, mut pos) = caches
        .cursors
        .iter()
        .find(|cursor| cursor.container == meta.tag_offset && cursor.next_index < meta.count)
        .map_or((0, meta.first_child), |cursor| {
            (cursor.next_index, cursor.next_pos)
        });

    for _ in 0..meta.count {
        if index == meta.count {
            index = 0;
            pos = meta.first_child;
        }
        let (span, value_pos) = string_span_at(bytes, state, pos, end)?;
        let next = skip_value(bytes, state, value_pos, end)?;
        if span_matches(bytes, span, query) {
            let value = decode_value(bytes, state, caches, value_pos)?;
            update_cursor(caches, meta.tag_offset, index + 1, next);
            return Ok(Some(value));
        }
        index += 1;
        pos = next;
    }
    Ok(None)
}

fn shape_find(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    meta: ContainerMeta,
    query: &[u8],
) -> Result<Option<NanBox>> {
    if meta.count == 0 {
        return Ok(None);
    }
    let (keys_start, keys_len) = state.shapes[meta.shape_id as usize];
    if keys_len != meta.count {
        return Err(ErrorCode::ReadError);
    }
    let recent = caches.shape_lookup[meta.shape_id as usize];
    let start = if recent < meta.count {
        (recent + 1) % meta.count
    } else {
        0
    };
    let mut matched = None;
    for step in 0..meta.count {
        let index = (start + step) % meta.count;
        let span = state.shape_keys[(keys_start + index) as usize];
        if span_matches(bytes, span, query) {
            matched = Some(index);
            break;
        }
    }
    let Some(mut index) = matched else {
        return Ok(None);
    };
    // Duplicate shape keys are legal. Always return the first matching value.
    for earlier in 0..index {
        let span = state.shape_keys[(keys_start + earlier) as usize];
        if span_matches(bytes, span, query) {
            index = earlier;
            break;
        }
    }
    caches.shape_lookup[meta.shape_id as usize] = index;
    element_at(bytes, state, caches, meta.tag_offset, index).map(Some)
}

#[inline]
pub(crate) fn find_property(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    container: u32,
    query: &[u8],
) -> Result<Option<NanBox>> {
    let meta = container_meta(bytes, state, caches, container)?;
    match meta.kind {
        ContainerKind::Array => Err(ErrorCode::NotAnObject),
        ContainerKind::Map => map_find(bytes, state, caches, meta, query),
        ContainerKind::Shape => shape_find(bytes, state, caches, meta, query),
    }
}

#[inline]
pub(crate) fn key_at(
    bytes: &[u8],
    state: &InputState,
    caches: &mut Caches,
    container: u32,
    index: u32,
) -> Result<(u32, u32)> {
    let meta = container_meta(bytes, state, caches, container)?;
    if meta.kind == ContainerKind::Array {
        return Err(ErrorCode::NotAnObject);
    }
    if index >= meta.count {
        return Err(ErrorCode::IndexOutOfBounds);
    }
    if meta.kind == ContainerKind::Shape {
        let (start, _) = state.shapes[meta.shape_id as usize];
        let span = state.shape_keys[(start + index) as usize];
        if span.1 >= NanBox::MAX_VALUE_LENGTH as u32 {
            caches.long_string_lens.insert(span.0, span.1);
        }
        return Ok(span);
    }

    let end = container_end(meta, bytes)?;
    let (mut current, mut pos) = cursor_start(caches, container, index, meta.first_child);
    while current < index {
        pos = skip_map_pair(bytes, state, pos, end)?.1;
        current += 1;
    }
    let (span, value_pos) = string_span_at(bytes, state, pos, end)?;
    let next = skip_value(bytes, state, value_pos, end)?;
    if span.1 >= NanBox::MAX_VALUE_LENGTH as u32 {
        caches.long_string_lens.insert(span.0, span.1);
    }
    update_cursor(caches, container, index + 1, next);
    Ok(span)
}

#[inline]
pub(crate) fn long_string_len(caches: &Caches, content: u32) -> Option<u32> {
    caches.long_string_lens.get(&content).copied()
}

#[inline]
pub(crate) fn skip_value(bytes: &[u8], state: &InputState, pos: u32, end: u32) -> Result<u32> {
    checked_add(pos, extent_at(bytes, state, pos, end, 0)?)
}

fn extent_at(bytes: &[u8], state: &InputState, pos: u32, end: u32, depth: u32) -> Result<u32> {
    if pos >= end || end as usize > bytes.len() {
        return Err(ErrorCode::ReadError);
    }
    let tag = bytes[pos as usize];
    let tag_info = TAG_INFO[tag as usize];
    match tag_info.class {
        TagClass::AFixed | TagClass::Scalar => {
            let size = tag_info.arg as u32;
            bounded_span(bytes, pos, size, end)?;
            Ok(size)
        }
        TagClass::String if tag_info.arg & 0x80 == 0 => {
            let size = tag_info.arg as u32;
            bounded_span(bytes, pos, size, end)?;
            Ok(size)
        }
        TagClass::String | TagClass::BLength => {
            let width = (tag_info.arg & 0x7f) as u32;
            let length_pos = checked_add(pos, 1)?;
            let payload_len = read_le(bytes, length_pos, width as u8, end)?;
            let header = checked_add(1, width)?;
            let extent = checked_add(header, payload_len)?;
            bounded_span(bytes, pos, extent, end)?;
            Ok(extent)
        }
        TagClass::CSequential => sequential_extent(bytes, state, pos, end, depth),
        TagClass::Reserved => Err(ErrorCode::ReadError),
    }
}

fn sequential_extent(
    bytes: &[u8],
    state: &InputState,
    start: u32,
    end: u32,
    depth: u32,
) -> Result<u32> {
    if depth >= MAX_DEPTH {
        return Err(ErrorCode::ReadError);
    }
    let tag = bytes[start as usize];
    let mut pos = checked_add(start, 1)?;
    let child_count = match tag {
        format::SEQFIXARRAY_MIN..=format::SEQFIXARRAY_MAX => {
            (tag - format::SEQFIXARRAY_MIN + 1) as u32
        }
        format::SEQFIXMAP_MIN..=format::SEQFIXMAP_MAX => {
            ((tag - format::SEQFIXMAP_MIN + 1) as u32) * 2
        }
        format::SEQARRAY => read_var_u32(bytes, &mut pos, end)?,
        format::SEQMAP => read_var_u32(bytes, &mut pos, end)?
            .checked_mul(2)
            .ok_or(ErrorCode::ReadError)?,
        format::SEQSHAPE => {
            let id = read_var_u32(bytes, &mut pos, end)?;
            state
                .shapes
                .get(id as usize)
                .map(|shape| shape.1)
                .ok_or(ErrorCode::ReadError)?
        }
        _ => return Err(ErrorCode::ReadError),
    };
    if child_count > end.saturating_sub(pos) {
        return Err(ErrorCode::ReadError);
    }
    for _ in 0..child_count {
        let extent = extent_at(bytes, state, pos, end, depth + 1)?;
        pos = checked_add(pos, extent)?;
    }
    pos.checked_sub(start).ok_or(ErrorCode::ReadError)
}

#[cfg(test)]
mod tests;
