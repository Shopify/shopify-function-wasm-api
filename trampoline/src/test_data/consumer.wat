(module
    ;; General
    (import "shopify_function_v1" "shopify_function_context_new" (func (result i32)))
    (import "shopify_function_v1" "shopify_function_intern_utf8_str" (func (param i32 i32 i32) (result i32)))

    ;; Read.
    (import "shopify_function_v1" "shopify_function_input_get" (func (param i32) (result i64)))
    (import "shopify_function_v1" "shopify_function_input_get_obj_prop" (func (param i32 i64 i32 i32) (result i64)))
    (import "shopify_function_v1" "shopify_function_input_get_interned_obj_prop" (func (param i32 i64 i32) (result i64)))
    (import "shopify_function_v1" "shopify_function_input_get_at_index" (func (param i32 i64 i32) (result i64)))
    (import "shopify_function_v1" "shopify_function_input_get_obj_key_at_index" (func (param i32 i64 i32) (result i64)))
    (import "shopify_function_v1" "shopify_function_input_get_val_len" (func (param i32 i64) (result i32)))
    (import "shopify_function_v1" "shopify_function_input_read_utf8_str" (func (param i32 i32 i32 i32)))

    ;; Write.
    (import "shopify_function_v1" "shopify_function_output_new_bool" (func (param i32 i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_null" (func (param i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_i32" (func (param i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_f64" (func (param i32 f64) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_utf8_str" (func (param i32 i32 i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_object" (func (param i32 i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_finish_object" (func (param i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_array" (func (param i32 i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_finish_array" (func (param i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_new_interned_utf8_str" (func (param i32 i32) (result i32)))
    (import "shopify_function_v1" "shopify_function_output_finalize" (func (param i32) (result i32)))

    ;; Log.
    (import "shopify_function_v1" "shopify_function_log_new_utf8_str" (func (param i32 i32) (result i32)))

    ;; Memory
    (memory 1)

    (export "memory" (memory 0))
)
