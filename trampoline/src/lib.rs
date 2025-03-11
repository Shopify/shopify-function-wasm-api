use anyhow::Context;
use std::path::Path;
use walrus::{FunctionBuilder, FunctionId, ImportKind, MemoryId, Module, ValType};

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

pub fn trampoline_existing_module(path: impl AsRef<Path>) -> walrus::Result<Module> {
    let mut module = Module::from_file(path)?;

    rename_imported_func(
        &mut module,
        PROVIDER_MODULE_NAME,
        "shopify_function_input_get",
        "_shopify_function_input_get",
    )?;

    let guest_memory_id = module.get_memory_id()?;

    let (provider_memory_id, _) =
        module.add_import_memory(PROVIDER_MODULE_NAME, "memory", false, false, 1, None, None);

    let memcpy_to_guest = add_memcpy_to_guest(&mut module, guest_memory_id, provider_memory_id)?;

    let imported_shopify_function_input_read_utf8_str = module
        .imports
        .get_func(PROVIDER_MODULE_NAME, "shopify_function_input_read_utf8_str")?;

    module.replace_imported_func(
        imported_shopify_function_input_read_utf8_str,
        |(builder, arg_locals)| {
            builder
                .func_body()
                .local_get(arg_locals[1])
                .local_get(arg_locals[0])
                .local_get(arg_locals[2])
                .call(memcpy_to_guest);
        },
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

/// (func $memcpy_to_guest (param $dst i32) (param $src i32) (param $size i32)
///   local.get $dst
///   local.get $src
///   local.get $size
///   (memory.copy 0 1)) ;; dst, src
fn add_memcpy_to_guest(
    module: &mut Module,
    guest_memory_id: MemoryId,
    provider_memory_id: MemoryId,
) -> walrus::Result<FunctionId> {
    let mut memcpy_to_guest = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32, ValType::I32, ValType::I32],
        &[],
    );

    let dst = module.locals.add(ValType::I32);
    let src = module.locals.add(ValType::I32);
    let size = module.locals.add(ValType::I32);

    memcpy_to_guest
        .func_body()
        .local_get(dst)
        .local_get(src)
        .local_get(size)
        .memory_copy(provider_memory_id, guest_memory_id);

    let function_id = memcpy_to_guest.finish(vec![dst, src, size], &mut module.funcs);

    Ok(function_id)
}
