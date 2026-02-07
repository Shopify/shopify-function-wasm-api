#include "shopify_function_value.h"

static const uint8_t STR_CART[] = "cart";
static const uint8_t STR_LINES[] = "lines";
static const uint8_t STR_QUANTITY[] = "quantity";
static const uint8_t STR_ERRORS[] = "errors";
static const uint8_t STR_LOCALIZED_MESSAGE[] = "localizedMessage";
static const uint8_t STR_TARGET[] = "target";
static const uint8_t STR_DOLLAR_CART[] = "$.cart";
static const uint8_t STR_ERROR_MSG[] = "Not possible to order more than one of each";

void _start(void) {
    Val input = shopify_function_input_get();

    Val cart = shopify_function_input_get_obj_prop(input, STR_CART, sizeof(STR_CART) - 1);
    if (!sf_value_is_object(cart)) {
        // Output empty errors
        shopify_function_output_new_object(1);
        shopify_function_output_new_utf8_str(STR_ERRORS, sizeof(STR_ERRORS) - 1);
        shopify_function_output_new_array(0);
        shopify_function_output_finish_array();
        shopify_function_output_finish_object();
        return;
    }

    Val lines = shopify_function_input_get_obj_prop(cart, STR_LINES, sizeof(STR_LINES) - 1);
    int has_error = 0;

    if (sf_value_is_array(lines)) {
        size_t lines_len = sf_array_len(lines);
        for (size_t i = 0; i < lines_len; i++) {
            Val line = shopify_function_input_get_at_index(lines, i);
            if (sf_value_is_object(line)) {
                Val quantity = shopify_function_input_get_obj_prop(
                    line, STR_QUANTITY, sizeof(STR_QUANTITY) - 1);
                if (sf_value_is_number(quantity)) {
                    double q = sf_value_as_number(quantity);
                    if (q > 1.0) {
                        has_error = 1;
                        break;
                    }
                }
            }
        }
    }

    // Write output: {"errors": [...]}
    shopify_function_output_new_object(1);
    shopify_function_output_new_utf8_str(STR_ERRORS, sizeof(STR_ERRORS) - 1);

    if (has_error) {
        shopify_function_output_new_array(1);
        // {"localizedMessage": "...", "target": "$.cart"}
        shopify_function_output_new_object(2);
        shopify_function_output_new_utf8_str(STR_LOCALIZED_MESSAGE, sizeof(STR_LOCALIZED_MESSAGE) - 1);
        shopify_function_output_new_utf8_str(STR_ERROR_MSG, sizeof(STR_ERROR_MSG) - 1);
        shopify_function_output_new_utf8_str(STR_TARGET, sizeof(STR_TARGET) - 1);
        shopify_function_output_new_utf8_str(STR_DOLLAR_CART, sizeof(STR_DOLLAR_CART) - 1);
        shopify_function_output_finish_object();
        shopify_function_output_finish_array();
    } else {
        shopify_function_output_new_array(0);
        shopify_function_output_finish_array();
    }

    shopify_function_output_finish_object();
}
