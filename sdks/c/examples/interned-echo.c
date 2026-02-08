#include "shopify_function_value.h"

// Interned string IDs (populated at startup)
static InternedStringId interned_foo;
static InternedStringId interned_bar;

static const uint8_t STR_FOO[] = "foo";
static const uint8_t STR_BAR[] = "bar";
static const uint8_t LOG_MSG[] = "interned-echo";

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
                // Read key to decide if it's internable
                Val key_val = shopify_function_input_get_obj_key_at_index(val, i);
                size_t key_len = sf_string_len(key_val);
                uint8_t* key_buf = sf_bump_alloc(key_len);
                if (!key_buf) break;
                sf_read_string(key_val, key_buf, key_len);

                // Check if key matches "foo" or "bar" for interned output
                int is_foo = (key_len == 3 && key_buf[0] == 'f' && key_buf[1] == 'o' && key_buf[2] == 'o');
                int is_bar = (key_len == 3 && key_buf[0] == 'b' && key_buf[1] == 'a' && key_buf[2] == 'r');

                if (is_foo) {
                    // Write key using interned string
                    shopify_function_output_new_interned_utf8_str(interned_foo);
                    // Read value using interned obj prop
                    Val child = shopify_function_input_get_interned_obj_prop(val, interned_foo);
                    echo_value(child);
                } else if (is_bar) {
                    shopify_function_output_new_interned_utf8_str(interned_bar);
                    Val child = shopify_function_input_get_interned_obj_prop(val, interned_bar);
                    echo_value(child);
                } else {
                    // Regular string key
                    shopify_function_output_new_utf8_str(key_buf, key_len);
                    Val child = shopify_function_input_get_at_index(val, i);
                    echo_value(child);
                }
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

    // Intern strings at startup
    interned_foo = shopify_function_intern_utf8_str(STR_FOO, sizeof(STR_FOO) - 1);
    interned_bar = shopify_function_intern_utf8_str(STR_BAR, sizeof(STR_BAR) - 1);

    // Log to exercise log API
    shopify_function_log_new_utf8_str(LOG_MSG, sizeof(LOG_MSG) - 1);

    Val input = shopify_function_input_get();
    echo_value(input);
}
