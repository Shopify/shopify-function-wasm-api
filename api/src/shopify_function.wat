;; WebAssembly Text Format (WAT) description of the shopify_function API.
;; This WAT file describes the interfaces exposed by the shopify_function Wasm API.
;; 
;; The Shopify Function Wasm API provides a standardized interface for
;; WebAssembly modules to interact with the Shopify Functions
;; platform. It enables efficient interactions between the WebAsembly
;; host and guest.

(module
  ;; Creates and returns a new Context handle.
  ;;
  ;; The context represents an isolated scope in which values and state
  ;; are bound. Every API call operates within this context, ensuring
  ;; that the execution environment remains distinct and independent
  ;; The returned context handle is used in all subsequent function
  ;; calls.
  ;;
  ;; Returns
  ;;   - i32 representing the handdle to the new Context.
  ;; Errors
  ;;   - Returns 0 if the Context creation fails.
  (import "shopify_function_v1" "shopify_function_context_new" 
    (func (result i32))
  )

  ;; Read API Functions - Used to access input data.

  ;; Retrieves the root input value from the context.
  ;; This is the main entry point for accessing input data.
  ;; Returns a NanBox representing a complex object structure.
  ;; The resulting value can be traversed using the other input API functions.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;; Returns:
  ;;   - NanBox value representing the root input value.
  (import "shopify_function_v1" "shopify_function_input_get" 
    (func (param $context i32) (result i64))
  )

  ;; Retrieve the length of a string, array, or object value.
  ;; The returned length depends on the value type:
  ;;   - Array: number of elements.
  ;;   - String: number of utf8-encoded bytes.
  ;;   - Object: number of entries.
  ;;   - -1 for all other values.
  ;;
  ;; Note that calling this function is not required in all cases, if
  ;; the length value is equal or less than (2^14) - 1, the length
  ;; value will be encoded as an immediate in its NaNBox
  ;; representation.
  ;;
  ;; Parameters
  ;;   - context: Context handle.
  ;;   - scope: NaNBox encoded value.
  ;; Returns
  ;;   - The value length.
  (import "shopify_function_v1" "shopify_function_input_get_val_len" 
    (func (param $context i32) (param $scope i64) (result i32))
  )

  ;; Reads a UTF-8 encoded string from source memory into destination buffer.
  ;; Length can be obtained from the length bits of the NanBox or from `shopify_function_input_get_val_len` call.
  ;; The caller must allocate a buffer of sufficient size.
  ;; No return value - the string is copied directly into the provided buffer.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - src: i32 memory address of the string.
  ;;   - out: i32 pointer to the destination buffer.
  ;;   - len: i32 length of the string in bytes.
  (import "shopify_function_v1" "shopify_function_input_read_utf8_str" 
    (func (param $context i32) (param $src i32) (param $out i32) (param $len i32))
  )

  ;; Gets a property from an object by name.
  ;; If property doesn't exist, returns a NanBox null value.
  ;; See `shopify_function_input_get_interned_obj_prop` for more efficient lookups involving the same property name.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - scope: i64 NanBox value of the object.
  ;;   - ptr: i32 pointer to the property name string.
  ;;   - len: i32 length of the property name in bytes.
  ;; Returns:
  ;;   - i64 NanBox value of the property.
  (import "shopify_function_v1" "shopify_function_input_get_obj_prop" 
    (func (param $context i32) (param $scope i64) (param $ptr i32) (param $len i32) (result i64))
  )

  ;; Gets a property from an object using a pre-interned string ID.
  ;; More efficient than shopify_function_input_get_obj_prop for repeated lookups.
  ;; Uses string interning to reduce overhead of property name lookups.
  ;; Recommended when accessing the same property on multiple objects.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - scope: i64 NanBox value of the object.
  ;;   - interned_string_id: i32 ID of the interned string.
  ;; Returns:
  ;;   - i64 NanBox value of the property.
  (import "shopify_function_v1" "shopify_function_input_get_interned_obj_prop" 
    (func (param $context i32) (param $scope i64) (param $interned_string_id i32) (result i64))
  )

  ;; Gets a value at specified index from an array or object.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - scope: i64 NanBox value of the array.
  ;;   - index: i32 index to retrieve (zero-based).
  ;; Returns:
  ;;   - i64 NanBox value at the index.
  ;; Errors:
  ;;   - If index is out of bounds, returns a NanBox with ErrorCode::IndexOutOfBounds.
  (import "shopify_function_v1" "shopify_function_input_get_at_index" 
    (func (param $context i32) (param $scope i64) (param $index i32) (result i64))
  )

  ;; Gets a key name at specified index from an object.
  ;; Used for dynamic iteration of object keys by index.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - scope: i64 NanBox value of the object.
  ;;   - index: i32 index of the key to retrieve (zero-based).
  ;; Returns:
  ;;   - i64 NanBox string value of the key.
  ;; Errors:
  ;;   - If index is out of bounds, returns a NanBox with ErrorCode::IndexOutOfBounds.
  (import "shopify_function_v1" "shopify_function_input_get_obj_key_at_index" 
    (func (param $context i32) (param $scope i64) (param $index i32) (result i64))
  )

  ;; Write API Functions - Used to build response data to return from the function.

  ;; Writes a new boolean output value.
  ;; Used to add boolean values to the output object/array being constructed.
  ;; Part of building structured output data.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - value: i32 boolean value (0 = false, 1 = true).
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_output_new_bool" 
    (func (param $context i32) (param $value i32) (result i32))
  )

  ;; Writes a new null output value.
  ;; Used to explicitly indicate null/absence of value in response.
  ;; Different from omitting a property.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_output_new_null" 
    (func (param $context i32) (result i32))
  )

  ;; Finalizes the output, making it available to the host.
  ;; Must be called after output construction is complete.
  ;; This is typically the last API call made before function returns.
  ;; Signals that the response is complete and ready to be used.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_output_finalize" 
    (func (param $context i32) (result i32))
  )

  ;; Writes a new integer output value.
  ;; Used for numeric values that fit within 32 bits.
  ;; More efficient than f64 for integral values.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - value: i32 integer value.
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_output_new_i32" 
    (func (param $context i32) (param $value i32) (result i32))
  )

  ;; Writes a new floating point output value.
  ;; Used for decimal or large numeric values.
  ;; Provides full IEEE 754 double precision.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - value: f64 floating point value.
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_new_f64" 
    (func (param $context i32) (param $value f64) (result i32))
  )

  ;; Writes a new string output value.
  ;; Used for text values in the response.
  ;; The string data is copied from WebAssembly memory.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - ptr: i32 pointer to string data in WebAssembly memory.
  ;;   - len: i32 length of string in bytes.
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_output_new_utf8_str" 
    (func (param $context i32) (param $ptr i32) (param $len i32) (result i32))
  )

  ;; Writes a new string output value from an interned string.
  ;; More efficient than direct string when reusing string values.
  ;; Especially useful for repetitive property names.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - id: i32 ID of the interned string from shopify_function_intern_utf8_str.
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_new_interned_utf8_str" 
    (func (param $context i32) (param $id i32) (result i32))
  )

  ;; Initializes a new object output value.
  ;; Must be paired with shopify_function_output_finish_object.
  ;; Properties are added using alternating key/value calls to write API functions.
  ;; Object construction follows a builder pattern.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - len: i32 number of properties in the object (key-value pairs).
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_new_object" 
    (func (param $context i32) (param $len i32) (result i32))
  )

  ;; Finalizes an object output value.
  ;; Must be called after adding all properties to the object.
  ;; Validates that the correct number of properties were added.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_finish_object" 
    (func (param $context i32) (result i32))
  )

  ;; Initializes a new array output value.
  ;; Must be paired with shopify_function_output_finish_array.
  ;; Elements are added using sequential calls to write API functions.
  ;; Array construction follows a builder pattern.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - len: i32 number of elements in the array.
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_new_array" 
    (func (param $context i32) (param $len i32) (result i32))
  )

  ;; Finalizes an array output value.
  ;; Must be called after adding all elements to the array.
  ;; Validates that the correct number of elements were added.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;; Returns:
  ;;   - i32 status code indicating success or failure.
  (import "shopify_function_v1" "shopify_function_output_finish_array" 
    (func (param $context i32) (result i32))
  )

  ;; Other Functions

  ;; Interns a UTF-8 string for reuse.
  ;; Optimizes memory usage and performance for repeated string operations.
  ;; Particularly useful for repeated property lookups or output property names.
  ;; The string is stored in the context and assigned a unique ID.
  ;; Note: Each call to this function is going to return a new interned string ID even if the same string is passed in. 
  ;; Its the caller's responsibility to ensure that the same string is interned only once.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - ptr: i32 pointer to string data in WebAssembly memory.
  ;;   - len: i32 length of string in bytes.
  ;; Returns:
  ;;   - i32 ID of the interned string (to be used in other API calls).
  (import "shopify_function_v1" "shopify_function_intern_utf8_str" 
    (func (param $context i32) (param $ptr i32) (param $len i32) (result i32))
  )

  ;; Logs a string output value.
  ;; The string data is copied from WebAssembly memory.
  ;; Parameters:
  ;;   - context: i32 pointer to the context.
  ;;   - ptr: i32 pointer to string data in WebAssembly memory.
  ;;   - len: i32 length of string in bytes.
  ;; Returns:
  ;;   - i32 status code indicating success or failure
  (import "shopify_function_v1" "shopify_function_log_utf8_str" 
    (func (param $context i32) (param $ptr i32) (param $len i32) (result i32))
  )
)
