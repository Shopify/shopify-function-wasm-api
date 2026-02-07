#include "shopify_function_value.h"

// Forward declaration
static void echo_value(Val val);

static void echo_value(Val val) {
    SFValueTag tag = sf_value_get_tag(val);

    switch (tag) {
        case SF_TAG_NULL:
            shopify_function_output_new_null();
            break;

        case SF_TAG_BOOL:
            shopify_function_output_new_bool((uint32_t)sf_value_as_bool(val));
            break;

        case SF_TAG_NUMBER: {
            double num = sf_value_as_number(val);
            // Check if the number is an integer that fits in i32
            double truncated = num >= 0 ? (double)(int64_t)num : (double)(int64_t)num;
            if (truncated == num && num >= -2147483648.0 && num <= 2147483647.0) {
                shopify_function_output_new_i32((int32_t)num);
            } else {
                shopify_function_output_new_f64(num);
            }
            break;
        }

        case SF_TAG_STRING: {
            size_t len = sf_string_len(val);
            uint8_t* buf = sf_bump_alloc(len);
            if (buf) {
                sf_read_string(val, buf, len);
                shopify_function_output_new_utf8_str(buf, len);
            }
            break;
        }

        case SF_TAG_OBJECT: {
            size_t len = sf_object_len(val);
            shopify_function_output_new_object(len);
            for (size_t i = 0; i < len; i++) {
                // Write key
                Val key_val = shopify_function_input_get_obj_key_at_index(val, i);
                size_t key_len = sf_string_len(key_val);
                uint8_t* key_buf = sf_bump_alloc(key_len);
                if (key_buf) {
                    sf_read_string(key_val, key_buf, key_len);
                    shopify_function_output_new_utf8_str(key_buf, key_len);
                }

                // Write value
                Val child = shopify_function_input_get_at_index(val, i);
                echo_value(child);
            }
            shopify_function_output_finish_object();
            break;
        }

        case SF_TAG_ARRAY: {
            size_t len = sf_array_len(val);
            shopify_function_output_new_array(len);
            for (size_t i = 0; i < len; i++) {
                Val child = shopify_function_input_get_at_index(val, i);
                echo_value(child);
            }
            shopify_function_output_finish_array();
            break;
        }

        default:
            shopify_function_output_new_null();
            break;
    }
}

void _start(void) {
    sf_bump_reset();
    Val input = shopify_function_input_get();
    echo_value(input);
}
