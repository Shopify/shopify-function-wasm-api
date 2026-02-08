TINYGO = tinygo

BUILD_DIR = build

.PHONY: all clean echo cart-checkout-validation interned-echo

all: echo cart-checkout-validation interned-echo

echo: $(BUILD_DIR)/echo.wasm

$(BUILD_DIR)/echo.wasm: examples/echo/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/echo/

cart-checkout-validation: $(BUILD_DIR)/cart-checkout-validation.wasm

$(BUILD_DIR)/cart-checkout-validation.wasm: examples/cart-checkout-validation/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/cart-checkout-validation/

interned-echo: $(BUILD_DIR)/interned-echo.wasm

$(BUILD_DIR)/interned-echo.wasm: examples/interned-echo/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/interned-echo/

clean:
	rm -rf $(BUILD_DIR)
