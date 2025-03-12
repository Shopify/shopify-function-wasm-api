use std::path::Path;
use walrus::{Module, ValType};

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

pub fn trampoline_existing_module(path: impl AsRef<Path>) -> walrus::Result<Module> {
    let mut module = Module::from_file(path)?;

    // Import the `_shopify_function_input_get` function.
    let input_get_type = module.types.add(&[], &[ValType::I64]);
    let (input_get, _) = module.add_import_func(
        PROVIDER_MODULE_NAME,
        "_shopify_function_input_get",
        input_get_type,
    );

    let imported_input_get = module
        .imports
        .get_func(PROVIDER_MODULE_NAME, "shopify_function_input_get")?;
    module.replace_imported_func(imported_input_get, |(builder, _arg_locals)| {
        builder.func_body().call(input_get);
    })?;

    Ok(module)
}
