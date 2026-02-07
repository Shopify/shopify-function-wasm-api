TINYGO = tinygo

BUILD_DIR = build

.PHONY: all clean echo

all: echo

echo: $(BUILD_DIR)/echo.wasm

$(BUILD_DIR)/echo.wasm: examples/echo/main.go shopify_function/*.go
	@mkdir -p $(BUILD_DIR)
	$(TINYGO) build -o $@ -target wasm -no-debug -scheduler=none ./examples/echo/

clean:
	rm -rf $(BUILD_DIR)
