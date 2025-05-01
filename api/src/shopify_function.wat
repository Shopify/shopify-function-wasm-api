(module
  ;; Import the Shopify Function API module
  ;; Creates a new context for the function execution
  (import "shopify_function_v0.0.1" "shopify_function_context_new" (func $shopify_function_context_new (result i32)))
  ;; Interns a UTF-8 string and returns its ID for efficient reuse
  (import "shopify_function_v0.0.1" "shopify_function_intern_utf8_str" (func $shopify_function_intern_utf8_str (param $context i32) (param $ptr i32) (param $len i32) (result i32)))

  ;; Read API imports
  ;; Gets the input value from the context
  (import "shopify_function_v0.0.1" "shopify_function_input_get" (func $shopify_function_input_get (param $context i32) (result i64)))
  ;; Gets the length of a value (for arrays or strings)
  (import "shopify_function_v0.0.1" "shopify_function_input_get_val_len" (func $shopify_function_input_get_val_len (param $context i32) (param $scope i64) (result i64)))
  ;; Reads a UTF-8 string from the input into the provided buffer
  (import "shopify_function_v0.0.1" "shopify_function_input_read_utf8_str" (func $shopify_function_input_read_utf8_str (param $context i32) (param $src i32) (param $out i32) (param $len i32)))
  ;; Gets a property from an object by name
  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_prop" (func $shopify_function_input_get_obj_prop (param $context i32) (param $scope i64) (param $ptr i32) (param $len i32) (result i64)))
  ;; Gets a property from an object using an interned string ID
  (import "shopify_function_v0.0.1" "shopify_function_input_get_interned_obj_prop" (func $shopify_function_input_get_interned_obj_prop (param $context i32) (param $scope i64) (param $interned_string_id i32) (result i64)))
  ;; Gets an element from an array by index
  (import "shopify_function_v0.0.1" "shopify_function_input_get_at_index" (func $shopify_function_input_get_at_index (param $context i32) (param $scope i64) (param $index i32) (result i64)))
  ;; Gets a key from an object by index
  (import "shopify_function_v0.0.1" "shopify_function_input_get_obj_key_at_index" (func $shopify_function_input_get_obj_key_at_index (param $context i32) (param $scope i64) (param $index i32) (result i64)))

  ;; Write API imports
  ;; Creates a new boolean output value
  (import "shopify_function_v0.0.1" "shopify_function_output_new_bool" (func $shopify_function_output_new_bool (param $context i32) (param $value i32) (result i32)))
  ;; Creates a new null output value
  (import "shopify_function_v0.0.1" "shopify_function_output_new_null" (func $shopify_function_output_new_null (param $context i32) (result i32)))
  ;; Finalizes the output and returns the result
  (import "shopify_function_v0.0.1" "shopify_function_output_finalize" (func $shopify_function_output_finalize (param $context i32) (result i32)))
  ;; Creates a new 32-bit integer output value
  (import "shopify_function_v0.0.1" "shopify_function_output_new_i32" (func $shopify_function_output_new_i32 (param $context i32) (param $value i32) (result i32)))
  ;; Creates a new 64-bit float output value
  (import "shopify_function_v0.0.1" "shopify_function_output_new_f64" (func $shopify_function_output_new_f64 (param $context i32) (param $value f64) (result i32)))
  ;; Creates a new UTF-8 string output value
  (import "shopify_function_v0.0.1" "shopify_function_output_new_utf8_str" (func $shopify_function_output_new_utf8_str (param $context i32) (param $ptr i32) (param $len i32) (result i32)))
  ;; Creates a new UTF-8 string output value using an interned string ID
  (import "shopify_function_v0.0.1" "shopify_function_output_new_interned_utf8_str" (func $shopify_function_output_new_interned_utf8_str (param $context i32) (param $id i32) (result i32)))
  ;; Starts creating a new object output with specified number of properties
  (import "shopify_function_v0.0.1" "shopify_function_output_new_object" (func $shopify_function_output_new_object (param $context i32) (param $len i32) (result i32)))
  ;; Finishes creating an object output
  (import "shopify_function_v0.0.1" "shopify_function_output_finish_object" (func $shopify_function_output_finish_object (param $context i32) (result i32)))
  ;; Starts creating a new array output with specified number of elements
  (import "shopify_function_v0.0.1" "shopify_function_output_new_array" (func $shopify_function_output_new_array (param $context i32) (param $len i32) (result i32)))
  ;; Finishes creating an array output
  (import "shopify_function_v0.0.1" "shopify_function_output_finish_array" (func $shopify_function_output_finish_array (param $context i32) (result i32)))

  ;; Memory
  (memory 1)
  (export "memory" (memory 0))
) 