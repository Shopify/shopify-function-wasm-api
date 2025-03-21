use std::cell::OnceCell;
use std::path::Path;
use walrus::{ir::BinaryOp, FunctionBuilder, FunctionId, ImportKind, MemoryId, Module, ValType};

pub const PROVIDER_MODULE_NAME: &str = concat!("shopify_function_v", env!("CARGO_PKG_VERSION"));

pub fn trampoline_existing_module(path: impl AsRef<Path>) -> walrus::Result<Module> {
    let module = Module::from_file(path)?;

    TrampolineCodegen::new(module)?.apply()
}

struct TrampolineCodegen {
    module: Module,
    guest_memory_id: MemoryId,
    provider_memory_id: OnceCell<MemoryId>,
    memcpy_to_guest: OnceCell<FunctionId>,
    memcpy_to_provider: OnceCell<FunctionId>,
    imported_shopify_function_realloc: OnceCell<FunctionId>,
    alloc: OnceCell<FunctionId>,
}

impl TrampolineCodegen {
    fn new(module: Module) -> walrus::Result<Self> {
        let guest_memory_id = module.get_memory_id()?;

        Ok(Self {
            module,
            guest_memory_id,
            provider_memory_id: OnceCell::new(),
            memcpy_to_guest: OnceCell::new(),
            memcpy_to_provider: OnceCell::new(),
            imported_shopify_function_realloc: OnceCell::new(),
            alloc: OnceCell::new(),
        })
    }

    fn provider_memory_id(&mut self) -> MemoryId {
        *self.provider_memory_id.get_or_init(|| {
            let (provider_memory_id, _) = self.module.add_import_memory(
                PROVIDER_MODULE_NAME,
                "memory",
                false,
                false,
                1,
                None,
                None,
            );
            provider_memory_id
        })
    }

    fn emit_memcpy_to_guest(&mut self) -> FunctionId {
        let provider_memory_id = self.provider_memory_id();

        *self.memcpy_to_guest.get_or_init(|| {
            let mut memcpy_to_guest = FunctionBuilder::new(
                &mut self.module.types,
                &[ValType::I32, ValType::I32, ValType::I32],
                &[],
            );

            let dst = self.module.locals.add(ValType::I32);
            let src = self.module.locals.add(ValType::I32);
            let size = self.module.locals.add(ValType::I32);

            memcpy_to_guest
                .func_body()
                .local_get(dst)
                .local_get(src)
                .local_get(size)
                .memory_copy(provider_memory_id, self.guest_memory_id);

            memcpy_to_guest.finish(vec![dst, src, size], &mut self.module.funcs)
        })
    }

    fn emit_memcpy_to_provider(&mut self) -> FunctionId {
        let provider_memory_id = self.provider_memory_id();

        *self.memcpy_to_provider.get_or_init(|| {
            let mut memcpy_to_provider = FunctionBuilder::new(
                &mut self.module.types,
                &[ValType::I32, ValType::I32, ValType::I32],
                &[],
            );

            let dst = self.module.locals.add(ValType::I32);
            let src = self.module.locals.add(ValType::I32);
            let size = self.module.locals.add(ValType::I32);

            memcpy_to_provider
                .func_body()
                .local_get(dst)
                .local_get(src)
                .local_get(size)
                .memory_copy(self.guest_memory_id, provider_memory_id);

            memcpy_to_provider.finish(vec![dst, src, size], &mut self.module.funcs)
        })
    }

    fn emit_shopify_function_realloc_import(&mut self) -> FunctionId {
        *self.imported_shopify_function_realloc.get_or_init(|| {
            let shopify_function_realloc_type = self.module.types.add(
                &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
                &[ValType::I32],
            );

            let (imported_shopify_function_realloc, _) = self.module.add_import_func(
                PROVIDER_MODULE_NAME,
                "shopify_function_realloc",
                shopify_function_realloc_type,
            );

            imported_shopify_function_realloc
        })
    }

    fn emit_alloc(&mut self) -> FunctionId {
        let imported_shopify_function_realloc = self.emit_shopify_function_realloc_import();

        *self.alloc.get_or_init(|| {
            let mut alloc =
                FunctionBuilder::new(&mut self.module.types, &[ValType::I32], &[ValType::I32]);

            let size = self.module.locals.add(ValType::I32);

            alloc
                .func_body()
                .i32_const(0)
                .i32_const(0)
                .i32_const(1)
                .local_get(size)
                .call(imported_shopify_function_realloc);

            alloc.finish(vec![size], &mut self.module.funcs)
        })
    }

    fn rename_imported_func(&mut self, func_name: &str, new_name: &str) -> walrus::Result<()> {
        let Some(import_id) = self.module.imports.find(PROVIDER_MODULE_NAME, func_name) else {
            return Ok(());
        };

        let import = self.module.imports.get_mut(import_id);

        if !matches!(import.kind, ImportKind::Function(_)) {
            anyhow::bail!("expected a function import");
        }

        import.name = new_name.to_string();

        Ok(())
    }

    fn emit_shopify_function_input_read_utf8_str(&mut self) -> walrus::Result<()> {
        let Ok(imported_shopify_function_input_read_utf8_str) = self
            .module
            .imports
            .get_func(PROVIDER_MODULE_NAME, "shopify_function_input_read_utf8_str")
        else {
            return Ok(());
        };

        let shopify_function_input_get_utf8_str_offset =
            self.module.types.add(&[ValType::I32], &[ValType::I32]);

        let (shopify_function_input_get_utf8_str_offset, _) = self.module.add_import_func(
            PROVIDER_MODULE_NAME,
            "_shopify_function_input_get_utf8_str_offset",
            shopify_function_input_get_utf8_str_offset,
        );

        let memcpy_to_guest = self.emit_memcpy_to_guest();

        self.module.replace_imported_func(
            imported_shopify_function_input_read_utf8_str,
            |(builder, arg_locals)| {
                builder
                    .func_body()
                    .local_get(arg_locals[1])
                    .local_get(arg_locals[0])
                    .call(shopify_function_input_get_utf8_str_offset)
                    .local_get(arg_locals[0])
                    .binop(BinaryOp::I32Add)
                    .local_get(arg_locals[2])
                    .call(memcpy_to_guest);
            },
        )?;

        Ok(())
    }

    fn emit_shopify_function_input_get_obj_prop(&mut self) -> walrus::Result<()> {
        if let Ok(imported_shopify_function_input_get_obj_prop) = self
            .module
            .imports
            .get_func(PROVIDER_MODULE_NAME, "shopify_function_input_get_obj_prop")
        {
            let shopify_function_input_get_obj_prop_type = self
                .module
                .types
                .add(&[ValType::I64, ValType::I32, ValType::I32], &[ValType::I64]);

            let (provider_shopify_function_input_get_obj_prop, _) = self.module.add_import_func(
                PROVIDER_MODULE_NAME,
                "_shopify_function_input_get_obj_prop",
                shopify_function_input_get_obj_prop_type,
            );

            let alloc = self.emit_alloc();
            let memcpy_to_provider = self.emit_memcpy_to_provider();

            let dst_ptr = self.module.locals.add(ValType::I32);

            self.module.replace_imported_func(
                imported_shopify_function_input_get_obj_prop,
                |(builder, arg_locals)| {
                    let scope = arg_locals[0];
                    let src_ptr = arg_locals[1];
                    let len = arg_locals[2];

                    builder
                        .func_body()
                        .local_get(len)
                        .call(alloc)
                        .local_tee(dst_ptr)
                        .local_get(src_ptr)
                        .local_get(len)
                        .call(memcpy_to_provider)
                        .local_get(scope)
                        .local_get(dst_ptr)
                        .local_get(len)
                        .call(provider_shopify_function_input_get_obj_prop);
                },
            )?;
        }

        Ok(())
    }

    fn apply(mut self) -> walrus::Result<Module> {
        self.rename_imported_func("shopify_function_input_get", "_shopify_function_input_get")?;
        self.rename_imported_func(
            "shopify_function_input_get_val_len",
            "_shopify_function_input_get_val_len",
        )?;
        self.emit_shopify_function_input_read_utf8_str()?;
        self.emit_shopify_function_input_get_obj_prop()?;
        self.rename_imported_func(
            "shopify_function_input_get_at_index",
            "_shopify_function_input_get_at_index",
        )?;
        self.rename_imported_func(
            "shopify_function_output_new",
            "_shopify_function_output_new",
        )?;
        self.rename_imported_func(
            "shopify_function_output_new_bool",
            "_shopify_function_output_new_bool",
        )?;
        self.rename_imported_func(
            "shopify_function_output_new_null",
            "_shopify_function_output_new_null",
        )?;
        self.rename_imported_func(
            "shopify_function_output_finalize",
            "_shopify_function_output_finalize",
        )?;
        self.rename_imported_func(
            "shopify_function_output_new_i32",
            "_shopify_function_output_new_i32",
        )?;
        self.rename_imported_func(
            "shopify_function_output_new_f64",
            "_shopify_function_output_new_f64",
        )?;

        Ok(self.module)
    }
}
