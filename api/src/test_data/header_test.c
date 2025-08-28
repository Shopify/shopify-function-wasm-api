#include "shopify_function.h"

// Force the compiler to keep these imports by declaring function pointers
// This file is needed to test the imports of the shopify_function.h file
// To update this file you will need a compiler toolchain:
// `brew install llvm lld`
// On updating this file, regenerate the header_test.wasm file with the following command:
// `/opt/homebrew/opt/llvm/bin/clang --target=wasm32-unknown-unknown -I .. -nostdlib -Wl,--no-entry -Wl,--export-all -Wl,--allow-undefined -o header_test.wasm header_test.c`

volatile void* imports[] = {
    (void*)shopify_function_input_get,
    (void*)shopify_function_input_get_val_len,
    (void*)shopify_function_input_read_utf8_str,
    (void*)shopify_function_input_get_obj_prop,
    (void*)shopify_function_input_get_interned_obj_prop,
    (void*)shopify_function_input_get_at_index,
    (void*)shopify_function_input_get_obj_key_at_index,
    (void*)shopify_function_output_new_bool,
    (void*)shopify_function_output_new_null,
    (void*)shopify_function_output_new_i32,
    (void*)shopify_function_output_new_f64,
    (void*)shopify_function_output_new_utf8_str,
    (void*)shopify_function_output_new_interned_utf8_str,
    (void*)shopify_function_output_new_object,
    (void*)shopify_function_output_finish_object,
    (void*)shopify_function_output_new_array,
    (void*)shopify_function_output_finish_array,
    (void*)shopify_function_intern_utf8_str,
    (void*)shopify_function_log_new_utf8_str
};
