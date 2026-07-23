use super::*;
use serde_json::json;

fn payload(body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::from(format::MAGIC);
    bytes.extend_from_slice(&[format::VERSION, 0]);
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn parses_plain_and_optimized_preludes() {
    let plain = fbf::to_vec(&json!({"x": [1, 2]})).unwrap();
    let state = parse_input(&plain).unwrap();
    assert_eq!(state.root, 5);
    assert!(state.strings.is_empty());

    let optimized = fbf::to_vec_optimized(&json!([
        {"x": 1, "y": "same"},
        {"x": 2, "y": "same"}
    ]))
    .unwrap();
    let state = parse_input(&optimized).unwrap();
    assert!(state.root > 5);
    assert!(!state.strings.is_empty());
    assert!(!state.shapes.is_empty());
}

#[test]
fn skips_all_framing_classes() {
    for body in [
        vec![0x2a],
        vec![format::FIXSTR_MIN + 2, b'h', b'i'],
        vec![format::STR8, 2, b'h', b'i'],
        vec![format::FIXARRAY0],
        vec![format::FIXARRAY0 + 3, 3, 1, 2, 3],
        vec![format::SEQFIXARRAY_MIN + 2, 1, 2, 3],
        vec![format::SEQARRAY, 3, 1, 2, 3],
    ] {
        let bytes = payload(&body);
        let state = parse_input(&bytes).unwrap();
        assert_eq!(
            skip_value(&bytes, &state, state.root, bytes.len() as u32),
            Ok(bytes.len() as u32)
        );
    }
}

#[test]
fn skips_mixed_sequential_children() {
    let body = [
        format::FIXARRAY0 + 2,
        5,
        format::SEQFIXARRAY_MIN,
        1,
        format::STR8,
        1,
        b'x',
    ];
    let bytes = payload(&body);
    let state = parse_input(&bytes).unwrap();
    assert_eq!(
        skip_value(&bytes, &state, state.root, bytes.len() as u32),
        Ok(bytes.len() as u32)
    );
}

#[test]
fn rejects_bad_headers_and_structural_truncation() {
    assert!(parse_input(b"FBF\x01\0").is_err());
    assert!(parse_input(b"BAD\x01\0\x80").is_err());
    let bytes = payload(&[format::STR16, 3, 0, b'x']);
    let state = parse_input(&bytes).unwrap();
    assert!(skip_value(&bytes, &state, state.root, bytes.len() as u32).is_err());
}

#[test]
fn rejects_reserved_tags_and_excess_depth() {
    let bytes = payload(&[0x9f]);
    let state = parse_input(&bytes).unwrap();
    assert!(skip_value(&bytes, &state, state.root, bytes.len() as u32).is_err());

    let mut body = vec![format::SEQFIXARRAY_MIN; (MAX_DEPTH + 1) as usize];
    body.push(format::NIL);
    let bytes = payload(&body);
    let state = parse_input(&bytes).unwrap();
    assert!(skip_value(&bytes, &state, state.root, bytes.len() as u32).is_err());
}
