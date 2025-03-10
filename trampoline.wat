(module
  ;; The memory of the consumer module (aka the guest code).
  (import "function" "memory" (memory 1)) ;; 0
  ;; Read API.
  (import "shopify_function_v0.1.0" "shopify_function_input_get" (func $lib_shopify_function_input_get (result i64)))
  ;; Exports

  ;; All the exports in this module are used for the merging process.
  ;; These could be removed by post-processing the module, as they are not
  ;; used in any shape or form.

  ;; Re-exports
  (export "trampoline_input_get" (func $lib_shopify_function_input_get))
)
