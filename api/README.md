# Shopify Function WebAssembly API

The Shopify Wasm API provides functions for your Shopify Function to read and write data. This API facilitates communication between the Shopify platform (host) and the custom Wasm module (guest), primarily through a set of imported functions that the Wasm module can call. The following sections describe these formats, along with status codes, error codes, and the NanBoxed value structure used by the API.

## NanBox Value Structure (64-bit)

Values exchanged with the Wasm module (primarily data read by the module and complex data written by it) are represented as 64-bit NaN-boxed `i64` integers. NaN-boxing provides a performant way to represent multiple value types (such as numbers, strings, booleans, objects, arrays, or errors) within this single `i64` representation, without requiring additional memory allocations for type information. The API uses specific bit patterns within the `i64` to encode this information.

### NaN-box Value Representation:

```
 63  62        52 51 50 49    46 45       32 31                 0
+---+------------+--+--+--------+-----------+--------------------+ 
| 0 | 11111111111| 1 | 1 | TTTT  |   LENGTH  |       VALUE        |
+---+------------+--+--+--------+-----------+--------------------+ 
 ^        ^         ^     ^          ^              ^
Sign  Exponent   Quiet  Tag bits   Length      Value bits
(0)   (all 1s)   NaN    (type)    (14 bits)   (32 bits - data/ptr)
```

- **Sign bit**: 0
- **Exponent**: 11 bits, all 1's.
- **Quiet NaN**: 1 bit set to 1.
- **Tag bits (TTTT)**: 4 bits indicating value type (0-15). See [Value Types](#value-types) below for details.
- **Length field**: 14 bits for string/array length.
- **Value field**: 32 bits for actual data or a pointer to heap-allocated structures.

### 64-bit floating point values:

When a value is a floating-point number (type tag `2`), it is not NanBoxed in the same way as other types. Instead, it directly uses the standard IEEE 754 double-precision binary floating-point format:

```
 63  62        52 51                                         0
+---+------------+--------------------------------------------+
| S |  Exponent  |                  Mantissa                  |
+---+------------+--------------------------------------------+
 ^       ^                             ^
Sign  Exponent                      Mantissa
(variable) (variable)              (variable)
```

Floating point numbers in our API follow the [IEEE-754 specification](https://standards.ieee.org/ieee/754/6210/).

### Value Types

The `Tag bits (TTTT)` in the NanBox structure determine the logical type of the `i64` value. The following type tags are used:

- **0**: `Null` - Null value
- **1**: `Bool` - Boolean value (true/false)
- **2**: `Number` - Numeric value (f64)
- **3**: `String` - UTF-8 encoded string (pointer + length)
- **4**: `Object` - Key-value collection (pointer + length)
- **5**: `Array` - Indexed collection of values (pointer + length)
- **15**: `Error` - Read error codes

## Reading Data

To read input data provided by the Shopify platform, your Wasm module will use a set of imported API functions. These functions allow you to access the root input value and traverse complex data structures like objects and arrays. For a complete list and detailed signatures of these read functions, refer to the C header file ([`api/src/shopify_function.h`](src/shopify_function.h)) or the WebAssembly Text Format definition ([`api/src/shopify_function.wat`](src/shopify_function.wat)).

Each read operation that retrieves data typically returns a 64-bit integer (`i64`). This `i64` value should be interpreted according to the NanBox structure and Value Types detailed above.

### Read Error Codes (i32 type)

When a 64-bit NaN-boxed `i64` value has its type tag bits set to `15` (Error), it signifies that a read error occurred. The lower 32 bits of this `i64` (the "Value field" in the NanBox structure) will then contain one of the following `i32` error codes:

- **0**: `DecodeError` - Value could not be decoded
- **1**: `NotAnObject` - Expected an object but received another type
- **2**: `ByteArrayOutOfBounds` - Byte array index out of bounds
- **3**: `ReadError` - Error occurred during reading
- **4**: `NotAnArray` - Expected an array but received another type
- **5**: `IndexOutOfBounds` - Array index out of bounds
- **6**: `NotIndexable` - Value is not indexable (not an object or array)

## Writing Data

To write output data back to the Shopify platform, your Wasm module will use a corresponding set of imported API functions. These functions allow you to construct complex data structures, such as objects and arrays, and populate them with various value types (strings, numbers, booleans, null) as defined in the [Value Types](#value-types) section. For a complete list and detailed signatures of these write functions, refer to the C header file ([`api/src/shopify_function.h`](src/shopify_function.h)) or the WebAssembly Text Format definition ([`api/src/shopify_function.wat`](src/shopify_function.wat)).

Most write operations return an `i32` status code. A value of `0` (Success) indicates the operation was successful, while other values signify errors.

### Write Error Codes (i32 type)

These are the `i32` status codes returned by write operations:

- **0**: `Success` - The operation was successful
- **1**: `IoError` - Error occurred during writing
- **2**: `ExpectedKey` - Expected a key but received a value
- **3**: `ObjectLengthError` - Object length mismatch
- **4**: `ValueAlreadyWritten` - Value already written
- **5**: `NotAnObject` - Expected an object but received another type
- **6**: `ValueNotFinished` - Value creation not completed
- **7**: `ArrayLengthError` - Array length mismatch
- **8**: `NotAnArray` - Expected an array but received another type
