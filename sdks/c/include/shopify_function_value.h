#ifndef SHOPIFY_FUNCTION_VALUE_H
#define SHOPIFY_FUNCTION_VALUE_H

#include "shopify_function.h"

// NaN-box constants for wasm32 (Val = i64)
//
// IEEE 754 double layout:
//   1 bit sign | 11 bits exponent | 52 bits mantissa
//
// NaN-boxed layout (64 bits total):
//   bits 50-62: NaN pattern (all 1s = 0x7FFC000000000000)
//   bits 46-49: tag (4 bits)
//   bits 32-45: value length (14 bits)
//   bits 0-31:  value/pointer (32 bits)
//
// Numbers (f64) are stored as raw IEEE 754 bits, NOT NaN-boxed.

#define SF_NAN_MASK          ((int64_t)0x7FFC000000000000LL)
#define SF_PAYLOAD_MASK      ((int64_t)0x0003FFFFFFFFFFFFLL)
#define SF_TAG_MASK          ((int64_t)0x0003C00000000000LL)
#define SF_VALUE_MASK        ((int64_t)0x00003FFFFFFFFFFFLL)
#define SF_POINTER_MASK      ((int64_t)0x00000000FFFFFFFFLL)
#define SF_TAG_SHIFT         46
#define SF_VALUE_ENCODING_SIZE 32
#define SF_MAX_VALUE_LENGTH  16383

// Tag values
typedef enum {
    SF_TAG_NULL   = 0,
    SF_TAG_BOOL   = 1,
    SF_TAG_NUMBER = 2,
    SF_TAG_STRING = 3,
    SF_TAG_OBJECT = 4,
    SF_TAG_ARRAY  = 5,
    SF_TAG_ERROR  = 15
} SFValueTag;

// Extract the tag from a NaN-boxed value.
// Returns SF_TAG_NUMBER (2) if the value is a raw f64 (not NaN-boxed).
static inline SFValueTag sf_value_get_tag(Val val) {
    if ((val & SF_NAN_MASK) != SF_NAN_MASK) {
        return SF_TAG_NUMBER;
    }
    return (SFValueTag)((val & SF_TAG_MASK) >> SF_TAG_SHIFT);
}

// Check if a value is null.
static inline int sf_value_is_null(Val val) {
    return sf_value_get_tag(val) == SF_TAG_NULL;
}

// Check if a value is a boolean.
static inline int sf_value_is_bool(Val val) {
    return sf_value_get_tag(val) == SF_TAG_BOOL;
}

// Check if a value is a number (f64).
static inline int sf_value_is_number(Val val) {
    return sf_value_get_tag(val) == SF_TAG_NUMBER;
}

// Check if a value is a string.
static inline int sf_value_is_string(Val val) {
    return sf_value_get_tag(val) == SF_TAG_STRING;
}

// Check if a value is an object.
static inline int sf_value_is_object(Val val) {
    return sf_value_get_tag(val) == SF_TAG_OBJECT;
}

// Check if a value is an array.
static inline int sf_value_is_array(Val val) {
    return sf_value_get_tag(val) == SF_TAG_ARRAY;
}

// Extract boolean value. Assumes sf_value_is_bool(val) is true.
static inline int sf_value_as_bool(Val val) {
    return (int)(val & SF_POINTER_MASK);
}

// Extract f64 value. Assumes sf_value_is_number(val) is true.
static inline double sf_value_as_number(Val val) {
    union { int64_t i; double d; } u;
    u.i = val;
    return u.d;
}

// Extract the inline length from a NaN-boxed value (for strings, arrays, objects).
static inline size_t sf_value_inline_len(Val val) {
    return (size_t)((val & SF_VALUE_MASK) >> SF_VALUE_ENCODING_SIZE);
}

// Extract the pointer from a NaN-boxed value.
static inline size_t sf_value_ptr(Val val) {
    return (size_t)(val & SF_POINTER_MASK);
}

// Get the length of a string value. Uses inline length if available,
// otherwise calls the host function.
static inline size_t sf_string_len(Val val) {
    size_t len = sf_value_inline_len(val);
    if (len < SF_MAX_VALUE_LENGTH) {
        return len;
    }
    return shopify_function_input_get_val_len(val);
}

// Get the number of entries in an object.
static inline size_t sf_object_len(Val val) {
    size_t len = sf_value_inline_len(val);
    if (len < SF_MAX_VALUE_LENGTH) {
        return len;
    }
    return shopify_function_input_get_val_len(val);
}

// Get the number of elements in an array.
static inline size_t sf_array_len(Val val) {
    size_t len = sf_value_inline_len(val);
    if (len < SF_MAX_VALUE_LENGTH) {
        return len;
    }
    return shopify_function_input_get_val_len(val);
}

// Read a string value into a buffer. Caller must provide a buffer of at least
// sf_string_len(val) bytes.
static inline void sf_read_string(Val val, uint8_t* buf, size_t len) {
    shopify_function_input_read_utf8_str(sf_value_ptr(val), buf, len);
}

// Simple bump allocator for use in nostdlib WASM environments.
// Provides a static buffer for temporary string allocations.
#define SF_BUMP_ALLOC_SIZE 65536
extern uint8_t sf_bump_buffer[SF_BUMP_ALLOC_SIZE];
extern size_t sf_bump_offset;

static inline uint8_t* sf_bump_alloc(size_t size) {
    if (sf_bump_offset + size > SF_BUMP_ALLOC_SIZE) {
        return (uint8_t*)0;
    }
    uint8_t* ptr = &sf_bump_buffer[sf_bump_offset];
    sf_bump_offset += size;
    return ptr;
}

static inline void sf_bump_reset(void) {
    sf_bump_offset = 0;
}

#endif // SHOPIFY_FUNCTION_VALUE_H
