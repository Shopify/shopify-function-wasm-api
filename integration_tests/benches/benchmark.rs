use std::sync::LazyLock;

use anyhow::Result;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use integration_tests::prepare_example;
use wasmtime::{Engine, Linker, Module, Store};

static LOG_LEN_EXAMPLE_RESULT: LazyLock<Result<()>> = LazyLock::new(|| prepare_example("log-len"));

pub fn criterion_benchmark(c: &mut Criterion) {
    benchmark(c).unwrap();
}

fn benchmark(c: &mut Criterion) -> Result<()> {
    LOG_LEN_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {e}"))?;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");
    let engine = Engine::default();

    let module_path =
        workspace_root.join("target/wasm32-unknown-unknown/release/examples/log-len.merged.wasm");

    let module = Module::from_file(&engine, workspace_root.join(module_path))?;

    let provider = Module::from_file(
        &engine,
        workspace_root.join("target/wasm32-unknown-unknown/release/shopify_function_provider.wasm"),
    )?;

    for count in [1, 500, 1_000, 5_000, 10_000, 100_000] {
        c.bench_with_input(BenchmarkId::new("log_len", count), &count, |b, _i| {
            b.iter_with_setup(
                || {
                    let linker = Linker::new(&engine);
                    let store = Store::new(&engine, ());
                    let input_bytes = rmp_serde::to_vec(&count).unwrap();
                    (linker, store, input_bytes)
                },
                |(linker, store, input_bytes)| {
                    routine(&provider, &module, (linker, store, &input_bytes)).unwrap()
                },
            );
        });
    }
    Ok(())
}

fn routine(
    provider: &Module,
    module: &Module,
    (mut linker, mut store, input_bytes): (Linker<()>, Store<()>, &[u8]),
) -> Result<()> {
    let provider_instance = linker.instantiate(&mut store, &provider)?;
    let init_func =
        provider_instance.get_typed_func::<(i32, i32), i32>(&mut store, "initialize")?;
    let input_buffer_offset = init_func.call(&mut store, (input_bytes.len() as i32, 1024))?;
    provider_instance
        .get_memory(&mut store, "memory")
        .unwrap()
        .write(&mut store, input_buffer_offset as usize, &input_bytes)?;

    linker.instance(
        &mut store,
        shopify_function_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    let instance = linker.instantiate(&mut store, &module)?;

    let func = instance.get_typed_func::<(), ()>(&mut store, "run")?;

    func.call(&mut store, ())?;

    let results_offset = provider_instance
        .get_typed_func::<(), u32>(&mut store, "finalize")?
        .call(&mut store, ())?;
    let memory = provider_instance.get_memory(&mut store, "memory").unwrap();
    let mut buf = [0; 16];
    memory.read(&store, results_offset as usize, &mut buf)?;

    let output_offset = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let output_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let logs_offset = u32::from_le_bytes(buf[8..12].try_into().unwrap()) as usize;
    let logs_len = u32::from_le_bytes(buf[12..16].try_into().unwrap()) as usize;
    let logs_len = logs_len.min(1024);
    let mut output = vec![0; output_len];
    memory.read(&store, output_offset, &mut output)?;
    let mut logs = vec![0; logs_len];
    memory.read(&store, logs_offset, &mut logs)?;

    Ok(())
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
