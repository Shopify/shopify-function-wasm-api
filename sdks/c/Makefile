CC ?= clang
CFLAGS = --target=wasm32-unknown-unknown -nostdlib -O2 -I include
LDFLAGS = -Wl,--no-entry -Wl,--export=_start -Wl,--export=memory -Wl,--allow-undefined

BUILD_DIR = build
EXAMPLES_DIR = examples

.PHONY: all clean echo

all: echo

echo: $(BUILD_DIR)/echo.wasm

$(BUILD_DIR)/echo.wasm: $(EXAMPLES_DIR)/echo.c src/shopify_function_value.c include/shopify_function.h include/shopify_function_value.h
	@mkdir -p $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(EXAMPLES_DIR)/echo.c src/shopify_function_value.c

clean:
	rm -rf $(BUILD_DIR)
