use anyhow::{Context, Result};
use std::fmt::Display;
use std::path::Path;
use wasmtime::{Config, Engine, Linker, Module, Store};

const STARTING_FUEL: u64 = u64::MAX;

/// Runs a pre-compiled WASM module through the provider+trampoline pipeline.
///
/// Takes a path to a `.wasm` file and JSON input, applies the trampoline,
/// runs the module, and returns the JSON output and any logs.
pub fn run_wasm_module(
    wasm_path: impl AsRef<Path>,
    input_json: &serde_json::Value,
) -> Result<(serde_json::Value, String)> {
    let wasm_path = wasm_path.as_ref();

    // Apply trampoline to the wasm module
    let trampolined_path = wasm_path.with_extension("trampolined.wasm");
    shopify_function_trampoline::trampoline_existing_module(wasm_path, &trampolined_path)
        .context("Failed to apply trampoline")?;

    // Encode input as MessagePack
    let input_bytes =
        rmp_serde::to_vec(input_json).context("Failed to encode input as MessagePack")?;

    let engine = Engine::new(Config::new().consume_fuel(true))?;

    let module = Module::from_file(&engine, &trampolined_path)
        .context("Failed to load trampolined WASM module")?;

    // Build the provider path relative to the workspace root
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let provider_path =
        workspace_root.join("target/wasm32-unknown-unknown/release/shopify_function_provider.wasm");

    let provider =
        Module::from_file(&engine, &provider_path).context("Failed to load provider WASM")?;

    let mut linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let provider_instance = linker.instantiate(&mut store, &provider)?;
    store.set_fuel(STARTING_FUEL)?;
    let init_func = provider_instance.get_typed_func::<i32, i32>(&mut store, "initialize")?;
    let input_buffer_offset = init_func.call(&mut store, input_bytes.len() as _)?;
    provider_instance
        .get_memory(&mut store, "memory")
        .unwrap()
        .write(&mut store, input_buffer_offset as usize, &input_bytes)?;
    linker.instance(
        &mut store,
        shopify_function_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    // Provide minimal WASI stubs for languages that inject WASI imports (e.g. TinyGo).
    // These are no-ops that satisfy the linker without requiring a full WASI implementation.
    let _ = linker.func_wrap("wasi_snapshot_preview1", "proc_exit", |_code: i32| {});
    let _ = linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |_fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 { 0 },
    );
    let _ = linker.func_wrap(
        "wasi_snapshot_preview1",
        "random_get",
        |_buf: i32, _len: i32| -> i32 { 0 },
    );

    store.set_fuel(STARTING_FUEL)?;
    let instance = linker.instantiate(&mut store, &module)?;

    let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    let result = func.call(&mut store, ());

    let results_offset = provider_instance
        .get_typed_func::<(), u32>(&mut store, "finalize")?
        .call(&mut store, ())?;
    let memory = provider_instance.get_memory(&mut store, "memory").unwrap();
    let mut buf = [0; 24];
    memory.read(&store, results_offset as usize, &mut buf)?;

    let output_offset = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let output_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let logs_offset1 = u32::from_le_bytes(buf[8..12].try_into().unwrap()) as usize;
    let logs_len1 = u32::from_le_bytes(buf[12..16].try_into().unwrap()) as usize;
    let logs_offset2 = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
    let logs_len2 = u32::from_le_bytes(buf[20..24].try_into().unwrap()) as usize;

    let mut output = vec![0; output_len];
    memory.read(&store, output_offset, &mut output)?;
    let mut logs1 = vec![0; logs_len1];
    memory.read(&store, logs_offset1, &mut logs1)?;
    let mut logs2 = vec![0; logs_len2];
    memory.read(&store, logs_offset2, &mut logs2)?;
    let mut logs = Vec::with_capacity(logs_len1 + logs_len2);
    logs.extend(logs1);
    logs.extend(logs2);

    drop(store);

    let logs = String::from_utf8_lossy(&logs).to_string();
    if let Err(e) = result {
        return Err(anyhow::anyhow!(CallFuncError {
            trap_error: e,
            logs,
        }));
    }

    let output_json: serde_json::Value =
        rmp_serde::from_slice(&output).context("Failed to decode MessagePack output")?;

    // Clean up trampolined file
    let _ = std::fs::remove_file(&trampolined_path);

    Ok((output_json, logs))
}

#[derive(Debug)]
struct CallFuncError {
    trap_error: anyhow::Error,
    logs: String,
}

impl Display for CallFuncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}\n\nLogs: {}", self.trap_error, self.logs)
    }
}

/// Build the provider WASM module (must be called before running conformance tests).
pub fn build_provider() -> Result<()> {
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "shopify_function_provider",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to build provider");
    }
    Ok(())
}
