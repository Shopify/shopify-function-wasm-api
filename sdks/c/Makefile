CC ?= clang
CFLAGS = --target=wasm32-unknown-unknown -nostdlib -O2 -I include
LDFLAGS = -Wl,--no-entry -Wl,--export=_start -Wl,--export=memory -Wl,--allow-undefined

BUILD_DIR = build
EXAMPLES_DIR = examples

.PHONY: all clean echo cart-checkout-validation interned-echo

all: echo cart-checkout-validation interned-echo

echo: $(BUILD_DIR)/echo.wasm

$(BUILD_DIR)/echo.wasm: $(EXAMPLES_DIR)/echo.c src/shopify_function_value.c include/shopify_function.h include/shopify_function_value.h
	@mkdir -p $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(EXAMPLES_DIR)/echo.c src/shopify_function_value.c

cart-checkout-validation: $(BUILD_DIR)/cart-checkout-validation.wasm

$(BUILD_DIR)/cart-checkout-validation.wasm: $(EXAMPLES_DIR)/cart-checkout-validation.c src/shopify_function_value.c include/shopify_function.h include/shopify_function_value.h
	@mkdir -p $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(EXAMPLES_DIR)/cart-checkout-validation.c src/shopify_function_value.c

interned-echo: $(BUILD_DIR)/interned-echo.wasm

$(BUILD_DIR)/interned-echo.wasm: $(EXAMPLES_DIR)/interned-echo.c src/shopify_function_value.c include/shopify_function.h include/shopify_function_value.h
	@mkdir -p $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(EXAMPLES_DIR)/interned-echo.c src/shopify_function_value.c

clean:
	rm -rf $(BUILD_DIR)
