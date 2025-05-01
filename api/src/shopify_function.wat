;; WebAssembly Text Format (WAT) description of the shopify_function API
;; This WAT file describes the interfaces exposed by the shopify_function Wasm module
;; 
;; The Shopify Function API provides a standardized interface for WebAssembly modules to
;; communicate with the Shopify platform. It handles JSON deserialization/serialization
;; via a NanBox representation to efficiently pass values between the host and WebAssembly.

(module
  ;; Import module name for the shopify_function API
  (import "shopify_function_v0.0.1" "shopify_function_context_new" 
    (func $shopify_function_context_new (result i32)
      ;; Creates and returns a new context handle as an i32 pointer
      ;; This is the first function called to initialize the API
      ;; The returned context handle is used in all subsequent function calls
      ;; Must be called before any other API functions
    )
  )

  ;; READ API FUNCTIONS - Used to access input data passed to the function

  (import "shopify_function_v0.0.1" "shopify_function_input_get" 
    (func $shopify_function_input_get (param $context i32) (result i64)
      ;; Retrieves the root input value from the context
      ;; This is the main entry point for accessing input data
      ;; Typically returns a NanBox representing a complex object structure
      ;; The resulting value can be traversed using the other input API functions
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i64 NanBox value representing the root input
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_val_len" 
    (func $shopify_function_input_get_val_len (param $context i32) (param $scope i64) (result i64)
      ;; Gets the length of a string, array, or object value
      ;; Used to determine buffer size needed for strings or iteration counts for collections
      ;; Essential before reading strings or iterating over collections
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value to get the length of
      ;; Returns:
      ;;   - i64 value representing the length
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_read_utf8_str" 
    (func $shopify_function_input_read_utf8_str 
      (param $context i32) (param $src i32) (param $out i32) (param $len i32)
      ;; Reads a UTF-8 string from source memory into destination buffer
      ;; Used after determining string length with shopify_function_input_get_val_len
      ;; The caller must allocate a buffer of sufficient size
      ;; No return value - the string is copied directly into the provided buffer
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - src: i32 memory address of the string
      ;;   - out: i32 pointer to the destination buffer
      ;;   - len: i32 length of the string in bytes
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_prop" 
    (func $shopify_function_input_get_obj_prop 
      (param $context i32) (param $scope i64) (param $ptr i32) (param $len i32) (result i64)
      ;; Gets a property from an object by name
      ;; Allows traversing object hierarchies using dot notation
      ;; If property doesn't exist, returns a NanBox null value
      ;; This version uses a direct string lookup which is less efficient for repeated lookups
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
    (func $shopify_function_input_get_interned_obj_prop 
      (param $context i32) (param $scope i64) (param $interned_string_id i32) (result i64)
      ;; Gets a property from an object using a pre-interned string ID
      ;; More efficient than shopify_function_input_get_obj_prop for repeated lookups
      ;; Uses string interning to reduce overhead of property name lookups
      ;; Recommended when accessing the same property on multiple objects
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the object
      ;;   - interned_string_id: i32 ID of the interned string
      ;; Returns:
      ;;   - i64 NanBox value of the property
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_at_index" 
    (func $shopify_function_input_get_at_index 
      (param $context i32) (param $scope i64) (param $index i32) (result i64)
      ;; Gets a value at specified index from an array
      ;; Used for iterating through array elements
      ;; Returns null if index is out of bounds
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the array
      ;;   - index: i32 index to retrieve (zero-based)
      ;; Returns:
      ;;   - i64 NanBox value at the index
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_key_at_index" 
    (func $shopify_function_input_get_obj_key_at_index 
      (param $context i32) (param $scope i64) (param $index i32) (result i64)
      ;; Gets a key name at specified index from an object
      ;; Used for dynamic iteration of object properties when keys aren't known
      ;; Combined with shopify_function_input_get_obj_prop to access values
      ;; Returns null if index is out of bounds
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - scope: i64 NanBox value of the object
      ;;   - index: i32 index of the key to retrieve (zero-based)
      ;; Returns:
      ;;   - i64 NanBox string value of the key
    )
  )

  ;; WRITE API FUNCTIONS - Used to build response data to return from the function

  (import "shopify_function_v0.0.1" "shopify_function_output_new_bool" 
    (func $shopify_function_output_new_bool (param $context i32) (param $value i32) (result i32)
      ;; Writes a new boolean output value
      ;; Used to add boolean values to the output object/array being constructed
      ;; Part of building structured output data
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - value: i32 boolean value (0 = false, 1 = true)
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_null" 
    (func $shopify_function_output_new_null (param $context i32) (result i32)
      ;; Writes a new null output value
      ;; Used to explicitly indicate null/absence of value in response
      ;; Different from omitting a property
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finalize" 
    (func $shopify_function_output_finalize (param $context i32) (result i32)
      ;; Finalizes the output, making it available to the host
      ;; Must be called after output construction is complete
      ;; This is typically the last API call made before function returns
      ;; Signals that the response is complete and ready to be used
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_i32" 
    (func $shopify_function_output_new_i32 (param $context i32) (param $value i32) (result i32)
      ;; Writes a new integer output value
      ;; Used for numeric values that fit within 32 bits
      ;; More efficient than f64 for integral values
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - value: i32 integer value
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_f64" 
    (func $shopify_function_output_new_f64 (param $context i32) (param $value f64) (result i32)
      ;; Writes a new floating point output value
      ;; Used for decimal or large numeric values
      ;; Provides full IEEE 754 double precision
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - value: f64 floating point value
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_utf8_str" 
    (func $shopify_function_output_new_utf8_str 
      (param $context i32) (param $ptr i32) (param $len i32) (result i32)
      ;; Writes a new string output value
      ;; Used for text values in the response
      ;; The string data is copied from WebAssembly memory
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - ptr: i32 pointer to string data in WebAssembly memory
      ;;   - len: i32 length of string in bytes
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_interned_utf8_str" 
    (func $shopify_function_output_new_interned_utf8_str 
      (param $context i32) (param $id i32) (result i32)
      ;; Writes a new string output value from an interned string ID
      ;; More efficient than direct string when reusing string values
      ;; Especially useful for repetitive property names
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - id: i32 ID of the interned string from shopify_function_intern_utf8_str
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_object" 
    (func $shopify_function_output_new_object (param $context i32) (param $len i32) (result i32)
      ;; Initializes a new object output value
      ;; Must be paired with shopify_function_output_finish_object
      ;; Properties are added using alternating key/value calls to write API functions
      ;; Object construction follows a builder pattern
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - len: i32 number of properties in the object (key-value pairs)
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finish_object" 
    (func $shopify_function_output_finish_object (param $context i32) (result i32)
      ;; Finalizes an object output value
      ;; Must be called after adding all properties to the object
      ;; Validates that the correct number of properties were added
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_new_array" 
    (func $shopify_function_output_new_array (param $context i32) (param $len i32) (result i32)
      ;; Initializes a new array output value
      ;; Must be paired with shopify_function_output_finish_array
      ;; Elements are added using sequential calls to write API functions
      ;; Array construction follows a builder pattern
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - len: i32 number of elements in the array
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  (import "shopify_function_v0.0.1" "shopify_function_output_finish_array" 
    (func $shopify_function_output_finish_array (param $context i32) (result i32)
      ;; Finalizes an array output value
      ;; Must be called after adding all elements to the array
      ;; Validates that the correct number of elements were added
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;; Returns:
      ;;   - i32 status code indicating success or failure
    )
  )

  ;; OTHER FUNCTIONS

  (import "shopify_function_v0.0.1" "shopify_function_intern_utf8_str" 
    (func $shopify_function_intern_utf8_str 
      (param $context i32) (param $ptr i32) (param $len i32) (result i32)
      ;; Interns a UTF-8 string for reuse
      ;; Optimizes memory usage and performance for repeated string operations
      ;; Particularly useful for repeated property lookups or output property names
      ;; The string is stored in the context and assigned a unique ID
      ;; Parameters:
      ;;   - context: i32 pointer to the context
      ;;   - ptr: i32 pointer to string data in WebAssembly memory
      ;;   - len: i32 length of string in bytes
      ;; Returns:
      ;;   - i32 ID of the interned string (to be used in other API calls)
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