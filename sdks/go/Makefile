TINYGO = tinygo

BUILD_DIR = build

.PHONY: all clean echo cart-checkout-validation

all: echo cart-checkout-validation

echo: $(BUILD_DIR)/echo.wasm

$(BUILD_DIR)/echo.wasm: examples/echo/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/echo/

cart-checkout-validation: $(BUILD_DIR)/cart-checkout-validation.wasm

$(BUILD_DIR)/cart-checkout-validation.wasm: examples/cart-checkout-validation/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/cart-checkout-validation/

clean:
	rm -rf $(BUILD_DIR)
