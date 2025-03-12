use walrus::{Module, ModuleConfig, ValType};

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

pub fn generate_trampoline() -> walrus::Result<Module> {
    // Construct a new Walrus module.
    let config = ModuleConfig::new();
    let mut module = Module::with_config(config);

    // Import the `memory` of the consumer module (aka the guest code).
    let (_memory, _) = module.add_import_memory("function", "memory", false, false, 1, None, None);

    // Import the `_shopify_function_input_get` function.
    let input_get_type = module.types.add(&[], &[ValType::I64]);
    let (input_get, _) = module.add_import_func(
        PROVIDER_MODULE_NAME,
        "_shopify_function_input_get",
        input_get_type,
    );

    // Export the `shopify_function_input_get` function.
    module.exports.add("shopify_function_input_get", input_get);

    Ok(module)
}
