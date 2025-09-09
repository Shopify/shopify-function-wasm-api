use shopify_function_wasm_api::init_panic_handler;

#[cfg_attr(target_family = "wasm", export_name = "_start")]
fn main() {
    init_panic_handler();
    panic!("at the disco");
}
