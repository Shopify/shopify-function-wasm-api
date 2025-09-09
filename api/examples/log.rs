use shopify_function_wasm_api::Context;

#[cfg_attr(target_family = "wasm", export_name = "_start")]
fn main() {
    shopify_function_wasm_api::init_panic_handler();
    let mut context = Context::new();
    context.log("Hi!\n");
    context.log("Hello\n");
    context.log("Here's a third string\n");
    context.log("✌️\n");
}
