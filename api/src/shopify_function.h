#ifndef SHOPIFY_FUNCTION_H
#define SHOPIFY_FUNCTION_H

#include <stdint.h>
#include <stddef.h>

// Type definitions
typedef int64_t Val;
typedef int32_t WriteResult;
typedef int32_t LogResult;
typedef size_t InternedStringId;

// Constants for WriteResult
#define WRITE_RESULT_OK 0
#define WRITE_RESULT_ERROR 1

// Import module declaration
#define SHOPIFY_FUNCTION_IMPORT_MODULE "shopify_function_v1"

// Read API
/**
 * Gets the input value from the context
 * @return The input value
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get")))
extern Val shopify_function_input_get();

/**
 * Gets the length of a value (for arrays, objects, or strings)
 * @param scope The value to get the length of
 * @return The length of the value
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get_val_len")))
extern size_t shopify_function_input_get_val_len(Val scope);

/**
 * Reads a UTF-8 encoded string from the input into the provided buffer
 * @param src The source address of the string
 * @param out The output buffer to write the string to
 * @param len The length of the string
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_read_utf8_str")))
extern void shopify_function_input_read_utf8_str(size_t src, uint8_t* out, size_t len);

/**
 * Gets an object property by name
 * @param scope The object to get the property from
 * @param ptr The property name (as a UTF-8 string)
 * @param len The length of the property name
 * @return The property value
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get_obj_prop")))
extern Val shopify_function_input_get_obj_prop(Val scope, const uint8_t* ptr, size_t len);

/**
 * Gets an object property by interned string ID
 * @param scope The object to get the property from
 * @param interned_string_id The interned string ID of the property name
 * @return The property value
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get_interned_obj_prop")))
extern Val shopify_function_input_get_interned_obj_prop(Val scope, InternedStringId interned_string_id);

/**
 * Gets an element from an array by index
 * @param scope The array to get the element from
 * @param index The index of the element
 * @return The element value
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get_at_index")))
extern Val shopify_function_input_get_at_index(Val scope, size_t index);

/**
 * Gets an object key at the specified index
 * @param scope The object to get the key from
 * @param index The index of the key
 * @return The key value (as a string)
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_input_get_obj_key_at_index")))
extern Val shopify_function_input_get_obj_key_at_index(Val scope, size_t index);

// Write API
/**
 * Creates a new boolean output value
 * @param value The boolean value (0 for false, non-zero for true)
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_bool")))
extern WriteResult shopify_function_output_new_bool(uint32_t value);

/**
 * Creates a new null output value
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_null")))
extern WriteResult shopify_function_output_new_null();

/**
 * Creates a new 32-bit integer output value
 * @param value The integer value
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_i32")))
extern WriteResult shopify_function_output_new_i32(int32_t value);

/**
 * Creates a new 64-bit float output value
 * @param value The float value
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_f64")))
extern WriteResult shopify_function_output_new_f64(double value);

/**
 * Creates a new UTF-8 string output value
 * @param ptr The string data
 * @param len The length of the string
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_utf8_str")))
extern WriteResult shopify_function_output_new_utf8_str(const uint8_t* ptr, size_t len);

/**
 * Creates a new UTF-8 string output value from an interned string ID
 * @param id The interned string ID
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_interned_utf8_str")))
extern WriteResult shopify_function_output_new_interned_utf8_str(InternedStringId id);

/**
 * Creates a new object output value with the specified number of properties
 * @param len The number of properties
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_object")))
extern WriteResult shopify_function_output_new_object(size_t len);

/**
 * Finalizes an object output value
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_finish_object")))
extern WriteResult shopify_function_output_finish_object();

/**
 * Creates a new array output value with the specified length
 * @param len The length of the array
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_new_array")))
extern WriteResult shopify_function_output_new_array(size_t len);

/**
 * Finalizes an array output value
 * @return WriteResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_output_finish_array")))
extern WriteResult shopify_function_output_finish_array();

// Other
/**
 * Interns a UTF-8 string and returns its ID for efficient reuse
 * @param ptr The string data
 * @param len The length of the string
 * @return The interned string ID
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_intern_utf8_str")))
extern InternedStringId shopify_function_intern_utf8_str(const uint8_t* ptr, size_t len);

/**
 * Logs a new UTF-8 string output value
 * @param ptr The string data
 * @param len The length of the string
 * @return LogResult indicating success or failure
 */
__attribute__((import_module(SHOPIFY_FUNCTION_IMPORT_MODULE)))
__attribute__((import_name("shopify_function_log_new_utf8_str")))
extern void shopify_function_log_new_utf8_str(const uint8_t* ptr, size_t len);

#endif // SHOPIFY_FUNCTION_H
