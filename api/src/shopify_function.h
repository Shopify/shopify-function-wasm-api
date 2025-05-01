#ifndef SHOPIFY_FUNCTION_H
#define SHOPIFY_FUNCTION_H

#include <stdint.h>
#include <stddef.h>

// Type definitions
typedef int32_t ContextPtr;
typedef int64_t Val;
typedef int32_t WriteResult;

// Constants for WriteResult
#define WRITE_RESULT_OK 0
#define WRITE_RESULT_ERROR 1

// Common API
// Creates a new context for the Shopify Function
ContextPtr shopify_function_context_new(void);

// Read API
// Gets the input value from the context
Val shopify_function_input_get(ContextPtr context);
// Gets the length of a value
size_t shopify_function_input_get_val_len(ContextPtr context, Val scope);
// Reads a UTF-8 string from the input
void shopify_function_input_read_utf8_str(ContextPtr context, size_t src, uint8_t* out, size_t len);
// Gets an object property by name
Val shopify_function_input_get_obj_prop(ContextPtr context, Val scope, const uint8_t* ptr, size_t len);
// Gets an object property by interned string ID
Val shopify_function_input_get_interned_obj_prop(ContextPtr context, Val scope, size_t interned_string_id);
// Gets an array element at the specified index
Val shopify_function_input_get_at_index(ContextPtr context, Val scope, size_t index);
// Gets an object key at the specified index
Val shopify_function_input_get_obj_key_at_index(ContextPtr context, Val scope, size_t index);

// Write API
// Creates a new boolean output value
WriteResult shopify_function_output_new_bool(ContextPtr context, uint32_t value);
// Creates a new null output value
WriteResult shopify_function_output_new_null(ContextPtr context);
// Finalizes the output
WriteResult shopify_function_output_finalize(ContextPtr context);
// Creates a new 32-bit integer output value
WriteResult shopify_function_output_new_i32(ContextPtr context, int32_t value);
// Creates a new 64-bit float output value
WriteResult shopify_function_output_new_f64(ContextPtr context, double value);
// Creates a new UTF-8 string output value
WriteResult shopify_function_output_new_utf8_str(ContextPtr context, const uint8_t* ptr, size_t len);
// Creates a new UTF-8 string output value from an interned string ID
WriteResult shopify_function_output_new_interned_utf8_str(ContextPtr context, size_t id);
// Creates a new object output value with the specified number of properties
WriteResult shopify_function_output_new_object(ContextPtr context, size_t len);
// Finalizes an object output value
WriteResult shopify_function_output_finish_object(ContextPtr context);
// Creates a new array output value with the specified length
WriteResult shopify_function_output_new_array(ContextPtr context, size_t len);
// Finalizes an array output value
WriteResult shopify_function_output_finish_array(ContextPtr context);

// Other
// Interns a UTF-8 string and returns its ID
size_t shopify_function_intern_utf8_str(ContextPtr context, const uint8_t* ptr, size_t len);

#endif // SHOPIFY_FUNCTION_H 