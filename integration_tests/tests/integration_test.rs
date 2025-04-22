use anyhow::Result;
use integration_tests::prepare_example;
use std::io::Cursor;
use std::sync::LazyLock;
use wasmtime::{Config, Engine, Linker, Module, Store};

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

    let input_stream = wasi_common::pipe::ReadPipe::new(Cursor::new(input));
    let output_stream = wasi_common::pipe::WritePipe::new_in_memory();
    let error_stream = wasi_common::pipe::WritePipe::new_in_memory();

    let mut linker = Linker::new(&engine);
    wasi_common::sync::add_to_linker(&mut linker, |ctx| ctx)?;
    let wasi = deterministic_wasi_ctx::build_wasi_ctx();
    wasi.set_stdin(Box::new(input_stream));
    wasi.set_stdout(Box::new(output_stream.clone()));
    wasi.set_stderr(Box::new(error_stream.clone()));
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
        let error = error_stream
            .try_into_inner()
            .map_err(|_| anyhow::anyhow!("Error stream reference still exists"))?
            .into_inner();
        return Err(anyhow::anyhow!(
            "{}\n\nSTDERR:\n{}",
            e,
            String::from_utf8(error)?
        ));
    }

    let output = output_stream
        .try_into_inner()
        .map_err(|_| anyhow::anyhow!("Output stream reference still exists"))?
        .into_inner();
    Ok(output)
}

fn run_example_with_input_and_string_output(
    example: &str,
    input: serde_json::Value,
) -> Result<String> {
    let output = run_example_with_input(example, input)?;
    Ok(String::from_utf8(output)?)
}

fn run_example_with_input_and_msgpack_output(
    example: &str,
    input: serde_json::Value,
) -> Result<serde_json::Value> {
    let output = run_example_with_input(example, input)?;
    Ok(rmp_serde::from_slice(&output)?)
}

static SIMPLE_EXAMPLE_RESULT: LazyLock<Result<()>> = LazyLock::new(|| prepare_example("simple"));
static ECHO_EXAMPLE_RESULT: LazyLock<Result<()>> = LazyLock::new(|| prepare_example("echo"));
static SERIALIZATION_EXAMPLE_RESULT: LazyLock<Result<()>> =
    LazyLock::new(|| prepare_example("serialization"));

#[test]
fn test_simple_with_bool_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(true))?,
        "got value true\n",
    );
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(false))?,
        "got value false\n",
    );
    Ok(())
}

#[test]
fn test_simple_with_null_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(null))?,
        "got value null\n"
    );
    Ok(())
}

#[test]
fn test_simple_with_number_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(0.0))?,
        "got value 0\n"
    );
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(1.0))?,
        "got value 1\n"
    );
    assert_eq!(
        run_example_with_input_and_string_output(
            "simple",
            serde_json::json!(std::f64::consts::PI)
        )?,
        "got value 3.141592653589793\n"
    );
    Ok(())
}

#[test]
fn test_simple_with_string_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!("Hello, world!"))?,
        "got value Hello, world!\n"
    );
    Ok(())
}

#[test]
fn test_simple_with_obj_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_example_with_input_and_string_output(
            "simple",
            serde_json::json!({ "other_key": "other_value", "key": "Hello, world!" })
        )?,
        "got value obj; key: Hello, world!, other_key: other_value\n"
    );
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!({}))?,
        "got value obj; key: null, other_key: null\n"
    );
    Ok(())
}

#[test]
fn test_simple_with_array_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;

    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!([]))?,
        "got value array; []\n"
    );

    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!([42]))?,
        "got value array; [42]\n"
    );

    let mixed_array_result =
        run_example_with_input_and_string_output("simple", serde_json::json!([1, "string", true]))?;

    assert_eq!(mixed_array_result, "got value array; [1, string, true]\n");

    Ok(())
}

#[test]
fn test_simple_with_large_string_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    let large_string = "a".repeat(u16::MAX as usize);
    assert_eq!(
        run_example_with_input_and_string_output("simple", serde_json::json!(large_string))?,
        format!("got value {}\n", large_string)
    );

    Ok(())
}

#[test]
#[ignore = "large array test is disabled since it takes a long time to run"]
fn test_simple_with_large_array_input() -> Result<()> {
    SIMPLE_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;

    let large_array: Vec<i32> = (0..=u16::MAX as usize).map(|x| x as i32).collect();
    let result =
        run_example_with_input_and_string_output("simple", serde_json::json!(large_array))?;

    assert!(result.starts_with("got value array; [0, 1, 2"));
    assert!(result.ends_with("65534, 65535]\n"));
    Ok(())
}

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
fn test_serialization() -> Result<()> {
    SERIALIZATION_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;

    let output = run_example_with_input_and_msgpack_output("serialization", serde_json::json!({}))?;

    assert_eq!(
        output,
        serde_json::json!({
            "my_string": "Hello, world!",
            "my_i32": 42,
            "my_f64": 1.23,
            "my_bool": true,
            "my_vec": [1, 2, 3],
            "my_hash_map": {
                "foo": 1,
                "bar": 2,
            },
            "my_option": null,
        })
    );
    Ok(())
}
