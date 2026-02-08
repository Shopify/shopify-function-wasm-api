use anyhow::Result;
use wasmtime::{Config, Engine, Linker, Module, Store};

fn run_c_example(
    export_name: &str,
    input: serde_json::Value,
) -> Result<(serde_json::Value, String)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");

    let engine = Engine::new(Config::new().consume_fuel(true))?;

    let c_wasm_path = workspace_root
        .join("..")
        .join("shopify-function-c/build/example_with_targets.merged.wasm");

    if !c_wasm_path.exists() {
        anyhow::bail!(
            "C example WASM not found at {:?}. Build and trampoline it first.",
            c_wasm_path
        );
    }

    let module = Module::from_file(&engine, &c_wasm_path)?;

    let provider_wasm_path =
        workspace_root.join("target/wasm32-unknown-unknown/release/shopify_function_provider.wasm");

    if !provider_wasm_path.exists() {
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
    }

    let provider = Module::from_file(&engine, &provider_wasm_path)?;

    let mut linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let provider_instance = linker.instantiate(&mut store, &provider)?;

    // Prepare input
    let input_bytes = rmp_serde::to_vec(&input)?;
    store.set_fuel(u64::MAX)?;
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

    // Instantiate the C module
    store.set_fuel(u64::MAX)?;
    let instance = linker.instantiate(&mut store, &module)?;

    // Call the named export
    let func = instance.get_typed_func::<(), ()>(&mut store, export_name)?;
    func.call(&mut store, ())?;

    // Read results
    let results_offset = provider_instance
        .get_typed_func::<(), u32>(&mut store, "finalize")?
        .call(&mut store, ())?;
    let memory = provider_instance
        .get_memory(&mut store, "memory")
        .unwrap();
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

    let logs = String::from_utf8_lossy(&logs).to_string();
    let output_value: serde_json::Value = rmp_serde::from_slice(&output)?;

    Ok((output_value, logs))
}

#[test]
fn test_c_target_a() -> Result<()> {
    let input = serde_json::json!({
        "id": "gid://shopify/Product/123",
        "num": 42,
        "name": "Test Product"
    });

    let (output, logs) = run_c_example("target_a", input)?;

    eprintln!("C target_a output: {}", serde_json::to_string_pretty(&output)?);
    eprintln!("C target_a logs: {:?}", logs);

    assert_eq!(output, serde_json::json!({"status": 200}));
    assert!(logs.contains("In target_a"), "Expected 'In target_a' in logs, got: {}", logs);
    Ok(())
}

#[test]
fn test_c_target_b() -> Result<()> {
    let input = serde_json::json!({
        "id": "gid://shopify/Product/123",
        "targetAResult": 200
    });

    let (output, logs) = run_c_example("target_b", input)?;

    eprintln!("C target_b output: {}", serde_json::to_string_pretty(&output)?);
    eprintln!("C target_b logs: {:?}", logs);

    assert_eq!(
        output,
        serde_json::json!({
            "name": "new name: \"gid://shopify/Product/123\"",
            "operations": [
                {"doThis": {"thisField": "this field"}},
                {"doThat": {"thatField": 42}}
            ]
        })
    );
    assert!(logs.contains("In target_b"), "Expected 'In target_b' in logs, got: {}", logs);
    Ok(())
}
