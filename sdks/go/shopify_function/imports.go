package shopify_function

// WASM import declarations for the Shopify Function API.
// These are the raw host function imports using //go:wasmimport directives.
// They are unexported and wrapped by the public API in value.go and output.go.

//go:wasmimport shopify_function_v2 shopify_function_input_get
func inputGet() int64

//go:wasmimport shopify_function_v2 shopify_function_input_get_val_len
func inputGetValLen(scope int64) uint32

//go:wasmimport shopify_function_v2 shopify_function_input_read_utf8_str
func inputReadUtf8Str(src uint32, out *byte, length uint32)

//go:wasmimport shopify_function_v2 shopify_function_input_get_obj_prop
func inputGetObjProp(scope int64, ptr *byte, length uint32) int64

//go:wasmimport shopify_function_v2 shopify_function_input_get_interned_obj_prop
func inputGetInternedObjProp(scope int64, id uint32) int64

//go:wasmimport shopify_function_v2 shopify_function_input_get_at_index
func inputGetAtIndex(scope int64, index uint32) int64

//go:wasmimport shopify_function_v2 shopify_function_input_get_obj_key_at_index
func inputGetObjKeyAtIndex(scope int64, index uint32) int64

//go:wasmimport shopify_function_v2 shopify_function_output_new_bool
func outputNewBool(value uint32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_null
func outputNewNull() int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_i32
func outputNewI32(value int32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_f64
func outputNewF64(value float64) int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_utf8_str
func outputNewUtf8Str(ptr *byte, length uint32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_interned_utf8_str
func outputNewInternedUtf8Str(id uint32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_object
func outputNewObject(length uint32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_finish_object
func outputFinishObject() int32

//go:wasmimport shopify_function_v2 shopify_function_output_new_array
func outputNewArray(length uint32) int32

//go:wasmimport shopify_function_v2 shopify_function_output_finish_array
func outputFinishArray() int32

//go:wasmimport shopify_function_v2 shopify_function_intern_utf8_str
func internUtf8Str(ptr *byte, length uint32) uint32

//go:wasmimport shopify_function_v2 shopify_function_log_new_utf8_str
func logNewUtf8Str(ptr *byte, length uint32)
