use anyhow::Result;
use integration_tests::prepare_example;
use std::sync::LazyLock;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{
    pipe::{MemoryInputPipe, MemoryOutputPipe},
    WasiCtxBuilder,
};

fn run_example_with_input(example: &str, input: serde_json::Value) -> Result<Vec<u8>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");
    let engine = Engine::new(&Config::new())?;
    let module = Module::from_file(
        &engine,
        workspace_root.join(format!(
            "target/wasm32-wasip1/release/examples/{example}.merged.wasm"
        )),
    )?;
    let provider = Module::from_file(
        &engine,
        workspace_root.join("target/wasm32-wasip1/release/shopify_function_wasm_api_provider.wasm"),
    )?;

    let input = rmp_serde::to_vec(&input)?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::preview0::add_to_linker_sync(&mut linker, |ctx| ctx)
        .expect("Failed to define wasi-ctx");
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| ctx)
        .expect("Failed to define wasi-ctx");
    deterministic_wasi_ctx::replace_scheduling_functions(&mut linker)
        .expect("Failed to replace scheduling functions in wasi-ctx");
    deterministic_wasi_ctx::replace_scheduling_functions_for_wasi_preview_0(&mut linker)
        .expect("Failed to replace scheduling functions in wasi-ctx");

    let stdin = MemoryInputPipe::new(input);
    let stderr = MemoryOutputPipe::new(usize::MAX);
    let stdout = MemoryOutputPipe::new(usize::MAX);
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder
        .stdin(stdin)
        .stdout(stdout.clone())
        .stderr(stderr.clone());
    deterministic_wasi_ctx::add_determinism_to_wasi_ctx_builder(&mut wasi_builder);
    let wasi = wasi_builder.build_p1();

    let mut store = Store::new(&engine, wasi);

    let provider_instance = linker.instantiate(&mut store, &provider)?;
    linker.instance(
        &mut store,
        shopify_function_wasm_api_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    let instance = linker.instantiate(&mut store, &module)?;

    let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

    let result = func.call(&mut store, ());

    drop(store);

    if let Err(e) = result {
        let error = stderr.contents().to_vec();
        return Err(anyhow::anyhow!(
            "{}\n\nSTDERR:\n{}",
            e,
            String::from_utf8(error)?
        ));
    }

    let output = stdout.contents().to_vec();
    Ok(output)
}

fn run_example_with_input_and_msgpack_output(
    example: &str,
    input: serde_json::Value,
) -> Result<serde_json::Value> {
    let output = run_example_with_input(example, input)?;
    Ok(rmp_serde::from_slice(&output)?)
}

static ECHO_EXAMPLE_RESULT: LazyLock<Result<()>> = LazyLock::new(|| prepare_example("echo"));

#[test]
fn test_echo_with_bool_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!(true))?,
        serde_json::json!(true)
    );
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!(false))?,
        serde_json::json!(false)
    );

    Ok(())
}

#[test]
fn test_echo_with_null_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!(null))?,
        serde_json::json!(null)
    );
    Ok(())
}

#[test]
fn test_echo_with_int_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    [0, 1, -1, i32::MAX, i32::MIN].iter().try_for_each(|&i| {
        assert_eq!(
            run_example_with_input_and_msgpack_output("echo", serde_json::json!(i))?,
            serde_json::json!(i)
        );
        Ok(())
    })
}

#[test]
fn test_echo_with_float_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    [0.1, 1.1, -1.1, f64::MAX, f64::MIN]
        .iter()
        .try_for_each(|&f| {
            assert_eq!(
                run_example_with_input_and_msgpack_output("echo", serde_json::json!(f))?,
                serde_json::json!(f)
            );
            Ok(())
        })
}

#[test]
fn test_echo_with_utf8_str_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!("Hello, world!"))?,
        serde_json::json!("Hello, world!")
    );
    Ok(())
}

#[test]
fn test_echo_with_obj_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_msgpack_output(
            "echo",
            serde_json::json!({ "foo": 1, "bar": 2 })
        )?,
        serde_json::json!({ "foo": 1, "bar": 2 })
    );
    Ok(())
}

#[test]
fn test_echo_with_array_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!([1, 2, 3]))?,
        serde_json::json!([1, 2, 3])
    );
    Ok(())
}

#[test]
fn test_echo_with_large_string_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    let large_string = "a".repeat(u16::MAX as usize);
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!(large_string))?,
        serde_json::json!(large_string)
    );

    Ok(())
}

#[test]
#[ignore = "large array test is disabled since it takes a long time to run"]
fn test_echo_with_large_array_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;

    let large_array: Vec<i32> = (0..=u16::MAX as usize).map(|x| x as i32).collect();
    assert_eq!(
        run_example_with_input_and_msgpack_output("echo", serde_json::json!(large_array))?,
        serde_json::json!(large_array)
    );
    Ok(())
}
