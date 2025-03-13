use anyhow::Context;
use std::path::Path;
use walrus::{ImportKind, Module};

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

pub fn trampoline_existing_module(path: impl AsRef<Path>) -> walrus::Result<Module> {
    let mut module = Module::from_file(path)?;

    rename_imported_func(
        &mut module,
        PROVIDER_MODULE_NAME,
        "shopify_function_input_get",
        "_shopify_function_input_get",
    )?;

    Ok(module)
}

fn rename_imported_func(
    module: &mut Module,
    module_name: &str,
    func_name: &str,
    new_name: &str,
) -> walrus::Result<()> {
    let import_id = module
        .imports
        .find(module_name, func_name)
        .context("no imported function found")?;

    let import = module.imports.get_mut(import_id);

    if !matches!(import.kind, ImportKind::Function(_)) {
        anyhow::bail!("expected a function import");
    }

    import.name = new_name.to_string();

    Ok(())
}
