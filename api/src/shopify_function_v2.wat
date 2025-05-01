;; WebAssembly Text Format (WAT) description of the shopify_function API
;; This WAT file describes the interfaces exposed by the shopify_function Wasm module

(module
  ;; Import module name for the shopify_function API
  (import "shopify_function_v0.0.1" "shopify_function_context_new" 
    (func $shopify_function_context_new (result i32)
      ;; Creates and returns a new context handle as an i32 pointer
    )
  )

  ;; READ API FUNCTIONS

  (import "shopify_function_v0.0.1" "shopify_function_input_get" 
    (func $shopify_function_input_get (param i32) (result i64)
      ;; Retrieves the root input value from the context
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i64 NanBox value representing the root input
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_val_len" 
    (func $shopify_function_input_get_val_len (param i32 i64) (result i32)
      ;; Gets the length of a string or array value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value to get the length of
      ;; Returns:
      ;;   - i32 value representing the length
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_read_utf8_str" 
    (func $shopify_function_input_read_utf8_str (param i32 i32 i32 i32)
      ;; Reads a UTF-8 string from source memory into destination buffer
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - src: i32 memory address of the string
      ;;   - out: i32 pointer to the destination buffer
      ;;   - len: i32 length of the string in bytes
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_prop" 
    (func $shopify_function_input_get_obj_prop (param i32 i64 i32 i32) (result i64)
      ;; Gets a property from an object
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the object
      ;;   - ptr: i32 pointer to the property name string
      ;;   - len: i32 length of the property name in bytes
      ;; Returns:
      ;;   - i64 NanBox value of the property
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_interned_obj_prop" 
    (func $shopify_function_input_get_interned_obj_prop (param i32 i64 i32) (result i64)
      ;; Gets a property from an object using an interned string
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the object
      ;;   - interned_string_id: i32 ID of the interned string
      ;; Returns:
      ;;   - i64 NanBox value of the property
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_at_index" 
    (func $shopify_function_input_get_at_index (param i32 i64 i32) (result i64)
      ;; Gets a value at specified index from an array
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the array
      ;;   - index: i32 index to retrieve
      ;; Returns:
      ;;   - i64 NanBox value at the index
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_key_at_index" 
    (func $shopify_function_input_get_obj_key_at_index (param i32 i64 i32) (result i64)
      ;; Gets a key at specified index from an object
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the object
      ;;   - index: i32 index of the key to retrieve
      ;; Returns:
      ;;   - i64 NanBox string value of the key
    )
  )

  ;; WRITE API FUNCTIONS

  (import "shopify_function_v0.0.1" "shopify_function_output_new_bool" 
    (func $shopify_function_output_new_bool (param i32 i32) (result i32)
      ;; Writes a new boolean output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - bool: i32 boolean value (0 = false, 1 = true)
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_null" 
    (func $shopify_function_output_new_null (param i32) (result i32)
      ;; Writes a new null output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finalize" 
    (func $shopify_function_output_finalize (param i32) (result i32)
      ;; Finalizes the output, making it available to the host
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_i32" 
    (func $shopify_function_output_new_i32 (param i32 i32) (result i32)
      ;; Writes a new integer output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - int: i32 integer value
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_f64" 
    (func $shopify_function_output_new_f64 (param i32 f64) (result i32)
      ;; Writes a new floating point output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - float: f64 floating point value
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_utf8_str" 
    (func $shopify_function_output_new_utf8_str (param i32 i32 i32) (result i32)
      ;; Writes a new string output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - ptr: i32 pointer to string data
      ;;   - len: i32 length of string in bytes
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_interned_utf8_str" 
    (func $shopify_function_output_new_interned_utf8_str (param i32 i32) (result i32)
      ;; Writes a new string output value from an interned string ID
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - id: i32 ID of the interned string
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_object" 
    (func $shopify_function_output_new_object (param i32 i32) (result i32)
      ;; Initializes a new object output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - len: i32 number of properties in the object
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finish_object" 
    (func $shopify_function_output_finish_object (param i32) (result i32)
      ;; Finalizes an object output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_array" 
    (func $shopify_function_output_new_array (param i32 i32) (result i32)
      ;; Initializes a new array output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - len: i32 number of elements in the array
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finish_array" 
    (func $shopify_function_output_finish_array (param i32) (result i32)
      ;; Finalizes an array output value
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  ;; OTHER FUNCTIONS

  (import "shopify_function_v0.0.1" "shopify_function_intern_utf8_str" 
    (func $shopify_function_intern_utf8_str (param i32 i32 i32) (result i32)
      ;; Interns a UTF-8 string for reuse
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - ptr: i32 pointer to string data
      ;;   - len: i32 length of string in bytes
      ;; Returns:
      ;;   - i32 ID of the interned string
    )
  )

  ;; Status Codes (returned by write functions):
  ;;   - 0: Success - The operation was successful
  ;;   - 1: IoError - Error occurred during writing
  ;;   - 2: ExpectedKey - Expected a key but received a value
  ;;   - 3: ObjectLengthError - Object length mismatch
  ;;   - 4: ValueAlreadyWritten - Value already written
  ;;   - 5: NotAnObject - Expected an object but received another type
  ;;   - 6: ValueNotFinished - Value creation not completed
  ;;   - 7: ArrayLengthError - Array length mismatch
  ;;   - 8: NotAnArray - Expected an array but received another type
  ;;
  ;; Error Codes (for read operations):
  ;;   - 0: DecodeError - Value could not be decoded
  ;;   - 1: NotAnObject - Expected an object but received another type
  ;;   - 2: ByteArrayOutOfBounds - Byte array index out of bounds
  ;;   - 3: ReadError - Error occurred during reading
  ;;   - 4: NotAnArray - Expected an array but received another type
  ;;   - 5: IndexOutOfBounds - Array index out of bounds
  ;;   - 6: NotIndexable - Value is not indexable (not an object or array)
  ;;
  ;; Type Codes (for typing in NanBox return values):
  ;;   - 0: Null - Null value
  ;;   - 1: Bool - Boolean value (true/false)
  ;;   - 2: Number - Numeric value (f64)
  ;;   - 3: String - UTF-8 encoded string (pointer + length)
  ;;   - 4: Object - Key-value collection (pointer + length)
  ;;   - 5: Array - Indexed collection of values (pointer + length)
  ;;   - 15: Error - Error information
  ;;
  ;; NanBox Value Structure (64-bit):
  ;;
  ;; For non-numeric values (using NaN boxing):
  ;;
  ;;  63  62        52 51 50 49    46 45       32 31                 0
  ;; +---+------------+--+--+--------+-----------+--------------------+
  ;; | 0 | 11111111111| 1 | 1 | TTTT  |   LENGTH  |       VALUE        |
  ;; +---+------------+--+--+--------+-----------+--------------------+
  ;;  ^        ^         ^     ^          ^              ^
  ;; Sign  Exponent   Quiet  Tag bits   Length      Value bits
  ;; (0)   (all 1s)   NaN    (type)    (14 bits)   (32 bits - data/ptr)
  ;;
  ;; - Sign bit: Always 0 for NaN boxes
  ;; - Exponent: Always all 1s (11 bits) for NaN representation
  ;; - Quiet NaN: Always 11 for NaN boxing
  ;; - Tag bits (TTTT): 4 bits indicating value type (0-15)
  ;; - Length field: 14 bits for string/array length
  ;; - Value field: 32 bits for actual data or pointer
  ;;
  ;; For Number values (normal f64 representation):
  ;;
  ;;  63  62        52 51                                         0
  ;; +---+------------+--------------------------------------------+
  ;; | S |  Exponent  |                  Mantissa                  |
  ;; +---+------------+--------------------------------------------+
  ;;  ^       ^                             ^
  ;; Sign  Exponent                      Mantissa
  ;; (variable) (variable)              (variable)
  ;;
  ;; When the exponent is not all 1s, the value is treated as a regular f64.
)