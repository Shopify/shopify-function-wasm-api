(module
  (type (;0;) (func (result i32)))
  (type (;1;) (func (param i32) (result i32)))
  (type (;2;) (func (param i32) (result i64)))
  (type (;3;) (func (param i32 i32) (result i32)))
  (type (;4;) (func (param i32 i32) (result i64)))
  (type (;5;) (func (param i32 i32 i32)))
  (type (;6;) (func (param i32 i32 i32) (result i32)))
  (type (;7;) (func (param i32 i32 i32 i32) (result i32)))
  (type (;8;) (func (param i32 i64) (result i64)))
  (type (;9;) (func (param i32 i64 i32) (result i64)))
  (type (;10;) (func (param i32 i64 i32 i32) (result i64)))
  (type (;11;) (func (param i32 f64) (result i32)))
  (import "shopify_function_v0.0.1" "_shopify_function_context_new" (func (;0;) (type 0)))
  (import "shopify_function_v0.0.1" "_shopify_function_input_get" (func (;1;) (type 2)))
  (import "shopify_function_v0.0.1" "_shopify_function_input_get_interned_obj_prop" (func (;2;) (type 9)))
  (import "shopify_function_v0.0.1" "_shopify_function_input_get_at_index" (func (;3;) (type 9)))
  (import "shopify_function_v0.0.1" "_shopify_function_input_get_val_len" (func (;4;) (type 8)))
  (import "shopify_function_v0.0.1" "shopify_function_input_get_utf8_str_addr" (func (;5;) (type 3)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_bool" (func (;6;) (type 3)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_null" (func (;7;) (type 1)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_i32" (func (;8;) (type 1)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_f64" (func (;9;) (type 11)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_object" (func (;10;) (type 3)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_finish_object" (func (;11;) (type 1)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_array" (func (;12;) (type 3)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_finish_array" (func (;13;) (type 1)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_interned_utf8_str" (func (;14;) (type 3)))
  (import "shopify_function_v0.0.1" "_shopify_function_output_finalize" (func (;15;) (type 1)))
  (import "shopify_function_v0.0.1" "_shopify_function_input_get_obj_prop" (func (;16;) (type 10)))
  (import "shopify_function_v0.0.1" "shopify_function_realloc" (func (;17;) (type 7)))
  (import "shopify_function_v0.0.1" "memory" (memory (;0;) 1))
  (import "shopify_function_v0.0.1" "_shopify_function_output_new_utf8_str" (func (;18;) (type 4)))
  (memory (;1;) 1)
  (export "memory" (memory 1))
  (func (;19;) (type 6) (param i32 i32 i32) (result i32)
    (local i64)
    local.get 0
    local.get 2
    call 18
    local.tee 3
    i64.const 32
    i64.shr_u
    i32.wrap_i64
    local.get 3
    i32.wrap_i64
    local.get 1
    local.get 2
    call 22
  )
  (func (;20;) (type 10) (param i32 i64 i32 i32) (result i64)
    (local i32)
    local.get 3
    call 21
    local.tee 4
    local.get 2
    local.get 3
    call 22
    local.get 0
    local.get 1
    local.get 4
    local.get 3
    call 16
  )
  (func (;21;) (type 1) (param i32) (result i32)
    i32.const 0
    i32.const 0
    i32.const 1
    local.get 0
    call 17
  )
  (func (;22;) (type 5) (param i32 i32 i32)
    local.get 0
    local.get 1
    local.get 2
    memory.copy 0 1
  )
  (@producers
    (processed-by "walrus" "0.23.3")
  )
)
