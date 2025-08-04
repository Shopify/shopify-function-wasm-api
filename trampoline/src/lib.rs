use std::cell::OnceCell;
use std::path::Path;
use walrus::{
    ir::{BinaryOp, UnaryOp},
    FunctionBuilder, FunctionId, ImportKind, MemoryId, Module, ValType,
};

static IMPORTS: &[(&str, &str)] = &[
    (
        "shopify_function_context_new",
        "_shopify_function_context_new",
    ),
    ("shopify_function_input_get", "_shopify_function_input_get"),
    (
        "shopify_function_input_get_val_len",
        "_shopify_function_input_get_val_len",
    ),
    ("shopify_function_input_read_utf8_str", ""),
    (
        "shopify_function_input_get_obj_prop",
        "_shopify_function_input_get_obj_prop",
    ),
    (
        "shopify_function_input_get_interned_obj_prop",
        "_shopify_function_input_get_interned_obj_prop",
    ),
    (
        "shopify_function_input_get_at_index",
        "_shopify_function_input_get_at_index",
    ),
    (
        "shopify_function_input_get_obj_key_at_index",
        "_shopify_function_input_get_obj_key_at_index",
    ),
    (
        "shopify_function_output_new_bool",
        "_shopify_function_output_new_bool",
    ),
    (
        "shopify_function_output_new_null",
        "_shopify_function_output_new_null",
    ),
    (
        "shopify_function_output_finalize",
        "_shopify_function_output_finalize",
    ),
    (
        "shopify_function_output_new_i32",
        "_shopify_function_output_new_i32",
    ),
    (
        "shopify_function_output_new_f64",
        "_shopify_function_output_new_f64",
    ),
    (
        "shopify_function_output_new_utf8_str",
        "_shopify_function_output_new_utf8_str",
    ),
    (
        "shopify_function_intern_utf8_str",
        "_shopify_function_intern_utf8_str",
    ),
    (
        "shopify_function_output_new_interned_utf8_str",
        "_shopify_function_output_new_interned_utf8_str",
    ),
    (
        "shopify_function_output_new_object",
        "_shopify_function_output_new_object",
    ),
    (
        "shopify_function_output_finish_object",
        "_shopify_function_output_finish_object",
    ),
    (
        "shopify_function_output_new_array",
        "_shopify_function_output_new_array",
    ),
    (
        "shopify_function_output_finish_array",
        "_shopify_function_output_finish_array",
    ),
];

pub const PROVIDER_MODULE_NAME: &str =
    concat!("shopify_function_v", env!("CARGO_PKG_VERSION_MAJOR"));

pub fn trampoline_existing_module(
    source_path: impl AsRef<Path>,
    destination_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let module = Module::from_file(source_path)?;

    TrampolineCodegen::new(module)?
        .apply()?
        .emit_wasm_file(destination_path)
}

pub struct TrampolineCodegen {
    module: Module,
    guest_memory_id: Option<MemoryId>,
    provider_memory_id: OnceCell<MemoryId>,
    memcpy_to_guest: OnceCell<FunctionId>,
    memcpy_to_provider: OnceCell<FunctionId>,
    imported_shopify_function_realloc: OnceCell<FunctionId>,
    alloc: OnceCell<FunctionId>,
}

impl TrampolineCodegen {
    pub fn new(module: Module) -> walrus::Result<Self> {
        let guest_memory_id = Self::guest_memory_id(&module)?;

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

    fn guest_memory_id(module: &Module) -> walrus::Result<Option<MemoryId>> {
        let non_imported_memories = module
            .memories
            .iter()
            .filter(|&memory| memory.import.is_none())
            .map(|memory| memory.id())
            .collect::<Vec<_>>();

        match non_imported_memories.split_first() {
            Some((memory_id, [])) => Ok(Some(*memory_id)),
            Some(_) => anyhow::bail!("multiple non-imported memories are not supported"),
            None => Ok(None),
        }
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
                .memory_copy(
                    provider_memory_id,
                    self.guest_memory_id.expect("no guest memory"),
                );

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
                .memory_copy(
                    self.guest_memory_id.expect("no guest memory"),
                    provider_memory_id,
                );

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

        let shopify_function_input_get_utf8_str_addr =
            self.module.types.add(&[ValType::I32], &[ValType::I32]);

        let (shopify_function_input_get_utf8_str_addr, _) = self.module.add_import_func(
            PROVIDER_MODULE_NAME,
            "_shopify_function_input_get_utf8_str_addr",
            shopify_function_input_get_utf8_str_addr,
        );

        let memcpy_to_guest = self.emit_memcpy_to_guest();

        self.module.replace_imported_func(
            imported_shopify_function_input_read_utf8_str,
            |(builder, arg_locals)| {
                let dst_ptr = arg_locals[0];
                let src_ptr = arg_locals[1];
                let len = arg_locals[2];

                builder
                    .func_body()
                    .local_get(src_ptr)
                    .local_get(dst_ptr)
                    .call(shopify_function_input_get_utf8_str_addr)
                    .local_get(len)
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

    fn emit_shopify_function_output_new_utf8_str(&mut self) -> walrus::Result<()> {
        let Ok(imported_shopify_function_output_new_utf8_str) = self
            .module
            .imports
            .get_func(PROVIDER_MODULE_NAME, "shopify_function_output_new_utf8_str")
        else {
            return Ok(());
        };

        let shopify_function_output_new_utf8_str_type =
            self.module.types.add(&[ValType::I32], &[ValType::I64]);

        let (provider_shopify_function_output_new_utf8_str, _) = self.module.add_import_func(
            PROVIDER_MODULE_NAME,
            "_shopify_function_output_new_utf8_str",
            shopify_function_output_new_utf8_str_type,
        );

        let memcpy_to_provider = self.emit_memcpy_to_provider();

        let output = self.module.locals.add(ValType::I64);

        self.module.replace_imported_func(
            imported_shopify_function_output_new_utf8_str,
            |(builder, arg_locals)| {
                let src_ptr = arg_locals[0];
                let len = arg_locals[1];

                builder
                    .func_body()
                    .local_get(len)
                    // most significant 32 bits are the result, least significant 32 bits are the pointer
                    .call(provider_shopify_function_output_new_utf8_str)
                    .local_tee(output)
                    // extract the result with a bit shift and wrap it to i32
                    .i64_const(32)
                    .binop(BinaryOp::I64ShrU)
                    .unop(UnaryOp::I32WrapI64) // result is on the stack now
                    // extract the pointer by wrapping the output to i32
                    .local_get(output)
                    .unop(UnaryOp::I32WrapI64) // dst_ptr is on the stack now
                    .local_get(src_ptr)
                    .local_get(len)
                    .call(memcpy_to_provider);
            },
        )?;

        Ok(())
    }

    fn emit_shopify_function_intern_utf8_str(&mut self) -> walrus::Result<()> {
        let Ok(imported_shopify_function_intern_utf8_str) = self
            .module
            .imports
            .get_func(PROVIDER_MODULE_NAME, "shopify_function_intern_utf8_str")
        else {
            return Ok(());
        };

        let shopify_function_intern_utf8_str_type =
            self.module.types.add(&[ValType::I32], &[ValType::I64]);

        let (provider_shopify_function_intern_utf8_str, _) = self.module.add_import_func(
            PROVIDER_MODULE_NAME,
            "_shopify_function_intern_utf8_str",
            shopify_function_intern_utf8_str_type,
        );

        let memcpy_to_provider = self.emit_memcpy_to_provider();

        let output = self.module.locals.add(ValType::I64);

        self.module.replace_imported_func(
            imported_shopify_function_intern_utf8_str,
            |(builder, arg_locals)| {
                let src_ptr = arg_locals[0];
                let len = arg_locals[1];

                builder
                    .func_body()
                    .local_get(len)
                    // most significant 32 bits are the ID, least significant 32 bits are the pointer
                    .call(provider_shopify_function_intern_utf8_str)
                    .local_tee(output)
                    // extract the ID with a bit shift and wrap it to i32
                    .i64_const(32)
                    .binop(BinaryOp::I64ShrU)
                    .unop(UnaryOp::I32WrapI64) // ID is on the stack now
                    // extract the pointer with a bit shift and wrap it to i32
                    .local_get(output)
                    .unop(UnaryOp::I32WrapI64) // dst_ptr is on the stack now
                    .local_get(src_ptr)
                    .local_get(len)
                    .call(memcpy_to_provider);
            },
        )?;

        Ok(())
    }

    pub fn apply(mut self) -> walrus::Result<Module> {
        // If the module does not have a memory, we should no-op
        if self.guest_memory_id.is_none() {
            return Ok(self.module);
        }

        for (original, new) in IMPORTS {
            match *original {
                "shopify_function_input_read_utf8_str" => {
                    self.emit_shopify_function_input_read_utf8_str()?
                }
                "shopify_function_input_get_obj_prop" => {
                    self.emit_shopify_function_input_get_obj_prop()?
                }
                "shopify_function_output_new_utf8_str" => {
                    self.emit_shopify_function_output_new_utf8_str()?
                }
                "shopify_function_intern_utf8_str" => {
                    self.emit_shopify_function_intern_utf8_str()?
                }
                original => self.rename_imported_func(original, new)?,
            };
        }

        Ok(self.module)
    }
}

#[cfg(test)]
mod test {
    use super::{TrampolineCodegen, IMPORTS, PROVIDER_MODULE_NAME};
    use walrus::Module;

    fn trampoline_wat(wat_bytes: &[u8]) -> walrus::Result<String> {
        let wasm_buf = wat::parse_bytes(wat_bytes)?;
        trampoline_wasm(&wasm_buf)
    }

    fn trampoline_wasm(wasm_bytes: &[u8]) -> walrus::Result<String> {
        let module = Module::from_buffer(wasm_bytes)?;
        let codegen = TrampolineCodegen::new(module)?;
        let mut result = codegen.apply()?;
        wasmprinter::print_bytes(result.emit_wasm())
    }

    #[test]
    fn disassemble_trampoline() {
        // to add a new test case, add a new file to the `test_data` directory and run `cargo test`
        //
        // to update the snapshots, either run `cargo insta review` or use the `INSTA_UPDATE`
        // environment variable as documented at https://docs.rs/insta/latest/insta/index.html#updating-snapshots
        insta::glob!("test_data/*.wat", |path| {
            let input = wat::parse_file(path).unwrap();
            let actual = trampoline_wat(&input).unwrap();
            insta::assert_snapshot!(actual);
        });
    }

    #[test]
    fn test_consumer_imports_the_entire_api_surface() {
        let input = include_bytes!("test_data/consumer.wat");
        let buf = wat::parse_bytes(input).unwrap();
        let module = Module::from_buffer(&buf).unwrap();
        for (import, _) in IMPORTS {
            assert!(module.imports.find(PROVIDER_MODULE_NAME, import).is_some());
        }
    }

    #[test]
    fn test_consumer_second_pass_is_a_no_op() {
        let input = include_bytes!("test_data/consumer.wat");
        let first_wat = trampoline_wat(input).unwrap();
        let second_wat = trampoline_wat(first_wat.as_bytes()).unwrap();

        assert_eq!(first_wat, second_wat);
    }

    #[test]
    fn test_error_for_multiple_guest_memories() {
        let module = r#"
        (module
            (memory 1)
            (memory 1)
        )
        "#;
        let result = trampoline_wat(module.as_bytes());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "multiple non-imported memories are not supported"
        );
    }
}
