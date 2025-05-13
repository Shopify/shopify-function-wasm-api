# Shopify Function WebAssembly API

This document provides details on the low-level aspects of the Shopify Function Wasm API, including status codes, error codes, type codes, and the NanBox value structure.

## Write Error Codes (i32 type):

- **0**: `Success` - The operation was successful
- **1**: `IoError` - Error occurred during writing
- **2**: `ExpectedKey` - Expected a key but received a value
- **3**: `ObjectLengthError` - Object length mismatch
- **4**: `ValueAlreadyWritten` - Value already written
- **5**: `NotAnObject` - Expected an object but received another type
- **6**: `ValueNotFinished` - Value creation not completed
- **7**: `ArrayLengthError` - Array length mismatch
- **8**: `NotAnArray` - Expected an array but received another type

## Read Error Codes (i32 type):

- **0**: `DecodeError` - Value could not be decoded
- **1**: `NotAnObject` - Expected an object but received another type
- **2**: `ByteArrayOutOfBounds` - Byte array index out of bounds
- **3**: `ReadError` - Error occurred during reading
- **4**: `NotAnArray` - Expected an array but received another type
- **5**: `IndexOutOfBounds` - Array index out of bounds
- **6**: `NotIndexable` - Value is not indexable (not an object or array)

## Value types:

Values in the Wasm API are represented as 64-bit NaN-boxed values. NaN-box provides a performant way to represent multiple value types without requiring additional memory allocations for type information. The API uses specific bit patterns to encode type information, length fields, and either immediate values or pointers to heap-allocated structure as shown in [NanBox Value Structure (64-bit)](#nanbox-value-structure-64-bit).

- **0**: `Null` - Null value
- **1**: `Bool` - Boolean value (true/false)
- **2**: `Number` - Numeric value (f64)
- **3**: `String` - UTF-8 encoded string (pointer + length)
- **4**: `Object` - Key-value collection (pointer + length)
- **5**: `Array` - Indexed collection of values (pointer + length)
- **15**: `Error` - Read error codes

## NanBox Value Structure (64-bit)

### NaN-box value representation: 

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
- **Exponent**: 11 bits, all 1's
- **Quiet NaN**: 1 bit set to 1
- **Tag bits (TTTT)**: 4 bits indicating value type (0-15)
- **Length field**: 14 bits for string/array length
- **Value field**: 32 bits for actual data or pointer

### 64-bit floating point values:

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
