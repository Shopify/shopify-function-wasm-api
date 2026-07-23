# Plan: Provider read-path implementation (FBF input navigation)

`provider/src/read.rs` currently stubs every exported function with `unimplemented!()`.
This document specifies a complete, optimal implementation. It encodes everything
learned from two prior implementations and a fuel-measurement campaign; follow it
closely. The prior (working, optimized) implementation is available for reference at
`git show HEAD:provider/src/read/nav.rs` and `git show HEAD:provider/src/read.rs` —
use it to check behavior and to port unit tests, but implement fresh from this spec.

## 0. Hard requirements

1. **ABI**: the eight exported functions in `provider/src/read.rs` keep their exact
   names/signatures (they are wired through the trampoline and `api` crate).
2. **Behavioral compatibility**: all existing tests must pass unchanged — provider
   unit tests are gone (you write new ones), but `api` crate tests/doctests and
   `integration_tests` (including wasmtime e2e + fuel thresholds) are the contract.
3. **Performance**: fuel integration tests enforce ±2% of these targets — meet or
   beat them (lower the targets if you beat them by >2%):
   - benchmark 11,652 · early-exit 12,458 · obj-prop echo 670 · interned echo 965
   - shapes example 4,611 · shapes benchmark 12,483 / early-exit 12,997
   - echo-null 465 · log tests unchanged (write-path only)
   - Reference micro numbers from the prior implementation (10k-call loop slope):
     `get_at_index` ≈ 384 fuel/call, `get_obj_prop` ≈ 850 fuel/call,
     per-line marginal ≈ 2,114 (plain input) / 2,497 (table-optimized input).
4. **No per-value heap allocation** during navigation. Allocation happens only when
   parsing the prelude (once) and in the rare long-string side table.

## 1. Format essentials (FBF v1)

Payload = 5-byte header + optional string table + optional shape table + root value.
Reference: `/Users/adampetro/src/github.com/shopify-playground/functions-binary-format/SPEC.md`
(§3–§8) and `src/tag.rs`. The `fbf` crate is already a path dependency; use
`fbf::format` constants/helpers and `fbf::varint`. You MAY use `fbf::read::{parse_header,
parse_prelude, Tables}` for the prelude, or hand-roll it against the flat structures
in §3 (preferred — avoids `Vec<Vec<Span>>`).

- Header: magic `"FBF"`, version `0x01`, flags: bit0 string table, bit1 shape table.
- String table: `varint count`, entries `varint len + bytes` (raw, no tags). ID = index.
- Shape table: `varint count`; per shape `varint keyCount` + keys as FBF string
  values (fixstr / str8/16/32 inline, or strref8/16/32). ID = index. Duplicate keys
  are legal (validation removed); first match wins on lookup.
- Value tags (all multibyte fields little-endian):
  - `0x00–0x7f` pos fixint, `0xe0–0xff` neg fixint
  - `0x80` nil, `0x81` false, `0x82` true
  - `0x83–0x86` int8/16/32/64 · `0x87–0x8a` uint8/16/32/64 · `0x8b/0x8c` f32/f64
  - `0x8d–0x8f` str8/16/32 (LE length, then bytes) · `0xa2–0xc1` fixstr (len in tag)
  - `0x99–0x9b` strref8/16/32 (LE id; self-sized: total 2/3/5 bytes)
  - Length-framed containers (byte length L follows tag; O(1) skip = 1+W+L):
    `0x93–0x95` array8/16/32 and `0x96–0x98` map8/16/32 — payload starts with a
    **varint count** then children; `0x9c–0x9e` shape8/16/32 — payload starts with a
    **varint shape id** then values; `0xd1–0xd7` fixarrayN / `0xd9–0xdf` fixmapN —
    count in tag, **u8 length field**; `0xd0`/`0xd8` fixarray0/fixmap0 — self-sized.
  - Sequential containers (no byte length; skip = walk children):
    `0x90` seqarray / `0x91` seqmap (varint count), `0x92` seqshape (varint shape id),
    `0xc2–0xc8` seqfixarray1–7, `0xc9–0xcf` seqfixmap1–7 (count in tag).
  - `0x9f–0xa1` reserved → error.
- Inputs are predominantly **length-framed** (optimize for O(1) skips); sequential
  tags must be handled correctly (recursive walk, depth cap 128, checked arithmetic)
  but need not be fast.

## 2. NanBox conventions (proven optimal — do not deviate)

All NanBoxes are produced here; `ptr` payloads are provider-interpreted offsets into
`Context.input_bytes` (`core::read::NanBox` is unchanged; len saturates at
`NanBox::MAX_VALUE_LENGTH` = 16383):

| Logical type | NanBox | ptr meaning | len meaning |
|---|---|---|---|
| null/bool/number | immediate | — | — |
| string | `NanBox::string` | **content** byte offset | byte length (saturating) |
| array | `NanBox::array` | **tag** byte offset | element count (saturating) |
| map | `NanBox::obj` | **tag** byte offset | pair count (saturating) |
| shaped object | `NanBox::obj` | **tag** byte offset | key count (saturating) |
| error | `NanBox::error(code)` | — | — |

- Strings: content offset makes `get_utf8_str_addr` a bounds-checked base+offset add,
  and works uniformly for inline strings, strref-resolved strings (offset points into
  the string-table region), and shape keys. Because a content offset cannot recover
  the header, record `offset → true len` in a side map **only when len ≥ 16383**;
  `get_val_len` consults it for saturated strings.
- Containers: tag offset is re-parsed on access; §4's metadata cache makes that O(1)
  amortized. Shaped objects are indistinguishable from maps to the guest.

## 3. Data structures (all in `Context`, reset by `initialize`/`initialize_from_fbf_bytes`)

```rust
struct InputState {
    root: u32,                       // offset of root value
    strings: Vec<(u32, u32)>,        // string table: (content offset, len); pre-sized
    shape_keys: Vec<(u32, u32)>,     // flat arena of resolved key content spans
    shapes: Vec<(u32, u32)>,         // per shape: (start, len) range into shape_keys
}
```
- Built lazily on first `shopify_function_input_get`; parse failure → `ReadError`.
- Pre-size vectors with `capacity = min(declared count, remaining bytes)`.
- No nested `Vec<Vec<_>>` (measured allocation cost); resolve strref keys to content
  spans at parse time so shape-key comparisons and `get_obj_key_at_index` need no
  further resolution.

Caches (cleared with the input):
- **Container metadata cache**: small direct-mapped array (8–16 slots, key
  `tag_offset`, replace on collision): `{ tag_offset, kind (array/map/shape),
  count: u32, first_child: u32, end: Option<u32>, shape_id: u32 }`. Every accessor
  goes through `container_meta(pos)`; this removes per-call tag/varint re-parsing.
- **Cursor cache**: 4 slots `{ container_tag_offset, next_index: u32, next_pos: u32 }`,
  round-robin replacement. In-order iteration and map scans resume from the cursor;
  out-of-order access falls back to a fresh walk from `first_child`. For maps, index
  units are *pairs* (next_pos points at a key).
- **Shape lookup memo**: per-shape last-matched key index (start the next scan there,
  wrap around once) — generated SDK code reads properties in declaration order, so
  this makes property→index resolution O(1) amortized. Optionally also memo
  `interned_string_id → (shape_id, key_index)` for `get_interned_obj_prop`.
- **Long-string lens**: `HashMap<u32, u32>` (content offset → len), strings ≥ 16383 only.
- **Property scratch buffer**: `input_obj_prop_buffer: Vec<u8>` +
  `shopify_function_input_get_obj_prop_buffer(len) -> usize` (returns pointer to a
  buffer of `len` bytes, reused across calls, `Vec::with_capacity(64)` initial). The
  trampoline glue copies the guest's query string here before calling
  `shopify_function_input_get_obj_prop` — the glue already exists and expects this
  export; match the prior implementation's contract exactly
  (`git show HEAD:provider/src/read.rs`, first export).

All offsets are u32 internally (format limits payloads to u32 lengths); cast once at
the boundary.

## 4. Core algorithms

Implement in a `nav` submodule as pure functions over `(&[u8], &InputState, &mut caches)`;
keep the exported fns thin.

- **TAG_INFO dispatch table**: a `static [TagInfo; 256]` classifying every tag byte:
  `{ class: A_fixed(size) | B_len(width) | C_seq(kind) | Str(kind) | Scalar(kind) | Reserved }`.
  One indexed load replaces long match chains in `skip`/`extent`/`decode` hot paths.
  (The prior implementation used match dispatch; the LUT is the main new idea worth
  trying. If it doesn't measure better, matching is acceptable — measure, don't guess.)
- `extent(pos) -> Result<u32>`: class A → constant; class B → `1 + W + L` via widened
  LE load with bounds check; class C → recursive walk (depth cap 128; every add checked).
- `skip(pos) -> Result<u32>` (= pos + extent, walking only when sequential).
- `decode_value(pos) -> Result<NanBox>`: scalars immediate (LE loads; ints as f64);
  strings → content span (strref: `strings[id]`); containers → `container_meta(pos)`
  for the count, NanBox with tag offset.
- `container_meta(pos)`: cache hit or parse per §1 tag rules (`shape` arity =
  `shapes[id].1`; error if id out of range).
- `element_at(pos, i)`: meta → resume from cursor if `i ≥ cursor.next_index`, else
  `first_child`; skip `Δ` values (arrays) / `2Δ` values (maps, then +1 to reach the
  value); decode; update cursor. Shapes: values only (`Δ` skips).
- `map_find(pos, query)`: iterate pairs starting at cursor (wrap once): key length
  check first (fixstr len from tag / str len field / strref → table span len), then
  byte compare; miss → skip value, continue. Hit → decode value, advance cursor past
  the pair. Missing key → `Ok(None)` (caller returns `NanBox::null()`).
- `shape_find(pos, query)`: meta → scan `shape_keys[shapes[id]]` starting at the
  per-shape memo (wrap once) with len-first compare → key index → `element_at`-style
  value skip. Duplicate keys: first match wins.
- Prelude parse: single forward pass, bounds-checked, no semantic validation.

## 5. Exported functions — exact semantics

| Function | Behavior | Errors (as `NanBox::error` unless noted) |
|---|---|---|
| `input_get()` | build `InputState` if absent; `decode_value(root)` | parse/decode failure → `ReadError` |
| `input_get_obj_prop(scope, ptr, len)` | scope must decode to Object; query = bytes at `ptr/len` (provider memory — the scratch buffer); map_find or shape_find | scope not object → `NotAnObject`; scope undecodable → `DecodeError`; missing key → `NanBox::null()`; malformed data → `ReadError` |
| `input_get_interned_obj_prop(scope, id)` | same, query = `string_interner.get(id)` | same |
| `input_get_at_index(scope, i)` | array element / map **value** at pair i / shape value i | `i ≥ count` → `IndexOutOfBounds`; scalar/string scope → `NotIndexable`; undecodable → `ReadError` |
| `input_get_obj_key_at_index(scope, i)` | map key at pair i / shape key i (string NanBox from content span) | non-object → `NotAnObject`; `i ≥ count` → `IndexOutOfBounds` |
| `input_get_val_len(scope)` | string byte len (side map if saturated) / array count / map pair count / shape key count — re-derive via `container_meta` when saturated | non-measurable type → `usize::MAX` (plain return, not a NanBox) |
| `input_get_utf8_str_addr(ptr)` | `input_bytes.as_ptr() + ptr`, bounds-checked | out of bounds → `0` |
| `input_get_obj_prop_buffer(len)` | grow/reuse scratch buffer, return its pointer | — |

Per-call discipline: one `Context::with`/`with_mut` borrow, decode the scope NanBox
once, no redundant re-encoding, `#[inline]` on hot nav functions.

## 6. Testing

Unit tests (new, in the nav module / read.rs):
- Port the committed corpus (`git show HEAD:provider/src/read/nav.rs`, `#[cfg(test)]`
  section) or write equivalent coverage:
  - scalars: every int width, both fixint ranges, f32/f64, bounds values
  - strings: fixstr/str8/str16/str32, strref, empty, ≥16383-byte string (val_len via
    side map), `get_utf8_str_addr` bounds behavior
  - arrays/maps: fixed, length-framed, sequential, empty; indexing incl. descending
    after ascending (cursor fallback correctness), out-of-bounds, not-indexable
  - shaped objects: property hit/miss, key-at-index, val_len, duplicate-key
    first-wins, out-of-range shape id
  - mixed framing (sequential children inside length-framed parents)
  - malformed: truncation at every structural boundary, reserved tags, bad strref id
- Fixtures: `fbf::to_vec` (plain), `fbf::to_vec_optimized` (tables+shapes),
  `fbf::to_vec_with` (sequential), plus handcrafted bytes (SPEC.md §9 / Appendix A).

Integration: the full existing suite must pass (`cargo test` workspace-wide),
including fuel thresholds (§0.3) — measure and adjust targets only downward.

## 7. Implementation order

1. `TAG_INFO` + `extent`/`skip` + prelude parse into `InputState` (+ unit tests)
2. `decode_value` + `input_get` + string/`val_len`/`utf8_str_addr` paths
3. `container_meta` + `element_at`/`key_at` + cursor cache
4. `map_find`/`shape_find` + interned path + scratch-buffer export
5. Long-string side map; error-path polish
6. Wasm build + full test suite + fuel runs; tune caches only if targets miss
