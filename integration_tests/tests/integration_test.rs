use anyhow::Result;
use integration_tests::prepare_example;
use std::sync::LazyLock;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{
    pipe::{MemoryInputPipe, MemoryOutputPipe},
    WasiCtxBuilder,
};

const STARTING_FUEL: u64 = u64::MAX;
const THRESHOLD_PERCENTAGE: f64 = 2.0;

/// Used to detect any significant changes in the fuel consumption when making
/// changes in Shopify Function Wasm API.
///
/// A threshold is used here so that we can decide how much of a change is
/// acceptable. The threshold value needs to be sufficiently large enough to
/// account for fuel differences between different operating systems.
///
/// We check for both increases and decreases in fuel consumption:
/// - If fuel_consumed is significantly higher than target_fuel, we fail the test and ask to consider if the changes are worth the increase
/// - If fuel_consumed is significantly lower than target_fuel, we show a message to double check the changes and update the target fuel if it's a legitimate improvement
fn assert_fuel_consumed_within_threshold(target_fuel: u64, fuel_consumed: u64) {
    let target_fuel = target_fuel as f64;
    let fuel_consumed = fuel_consumed as f64;
    let percentage_difference = ((fuel_consumed - target_fuel) / target_fuel).abs() * 100.0;

    if fuel_consumed > target_fuel {
        assert!(
            percentage_difference <= THRESHOLD_PERCENTAGE,
            "fuel_consumed ({}) was not within {:.2}% of the target_fuel value ({}). Please consider if the changes are worth the increase in fuel consumption.",
            fuel_consumed,
            THRESHOLD_PERCENTAGE,
            target_fuel
        );
    } else if percentage_difference > THRESHOLD_PERCENTAGE {
        panic!(
            "fuel_consumed ({}) was significantly better than target_fuel value ({}) by more than {:.2}%. This is a significant improvement! Please double check your changes and update the target fuel if this is a legitimate improvement.",
            fuel_consumed,
            target_fuel,
            THRESHOLD_PERCENTAGE
        );
    }
}

fn run_example(example: &str, input_bytes: Vec<u8>, use_wasi: bool) -> Result<(Vec<u8>, u64)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");
    let engine = Engine::new(Config::new().consume_fuel(true))?;

    let module_path = workspace_root.join(format!(
        "target/wasm32-wasip1/release/examples/{example}.merged.wasm"
    ));

    let module = Module::from_file(&engine, workspace_root.join(module_path))?;

    let provider = Module::from_file(
        &engine,
        workspace_root.join("target/wasm32-wasip1/release/shopify_function_provider.wasm"),
    )?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::preview0::add_to_linker_sync(&mut linker, |ctx| ctx)
        .expect("Failed to define wasi-ctx");
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| ctx)
        .expect("Failed to define wasi-ctx");
    deterministic_wasi_ctx::replace_scheduling_functions(&mut linker)
        .expect("Failed to replace scheduling functions in wasi-ctx");
    deterministic_wasi_ctx::replace_scheduling_functions_for_wasi_preview_0(&mut linker)
        .expect("Failed to replace scheduling functions in wasi-ctx");

    let stderr = MemoryOutputPipe::new(usize::MAX);
    let stdout = MemoryOutputPipe::new(usize::MAX);
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdout(stdout.clone()).stderr(stderr.clone());
    if use_wasi {
        let stdin = MemoryInputPipe::new(input_bytes.clone());
        wasi_builder.stdin(stdin);
    }
    deterministic_wasi_ctx::add_determinism_to_wasi_ctx_builder(&mut wasi_builder);
    let wasi = wasi_builder.build_p1();
    let mut store = Store::new(&engine, wasi);

    let provider_instance = linker.instantiate(&mut store, &provider)?;
    if !use_wasi {
        store.set_fuel(STARTING_FUEL)?;
        let init_func = provider_instance.get_typed_func::<i32, i32>(&mut store, "initialize")?;
        let input_buffer_ptr = init_func.call(&mut store, input_bytes.len() as i32)?;
        provider_instance
            .get_memory(&mut store, "memory")
            .unwrap()
            .write(&mut store, input_buffer_ptr as usize, &input_bytes)?;
    }
    linker.instance(
        &mut store,
        shopify_function_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    let instance = linker.instantiate(&mut store, &module)?;

    store.set_fuel(STARTING_FUEL)?;
    let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

    let result = func.call(&mut store, ());

    let instructions = STARTING_FUEL.saturating_sub(store.get_fuel().unwrap_or_default());

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
    Ok((output, instructions))
}

fn decode_msgpack_output(output: Vec<u8>) -> Result<serde_json::Value> {
    Ok(rmp_serde::from_slice(&output)?)
}

fn decode_json_output(output: Vec<u8>) -> Result<serde_json::Value> {
    match serde_json::from_slice(&output) {
        Ok(json_value) => Ok(json_value),
        Err(_) => match rmp_serde::from_slice(&output) {
            Ok(msgpack_value) => Ok(msgpack_value),
            Err(_) => match String::from_utf8(output.clone()) {
                Ok(string_output) => {
                    eprintln!(
                        "Failed to parse output as JSON or MessagePack. Raw output: {}",
                        string_output
                    );
                    Ok(serde_json::json!({ "raw_output": string_output }))
                }
                Err(_) => {
                    eprintln!(
                        "Output is not valid UTF-8. Raw binary length: {} bytes",
                        output.len()
                    );
                    Ok(serde_json::json!({ "raw_output_length": output.len() }))
                }
            },
        },
    }
}

fn prepare_wasm_api_input(input: serde_json::Value) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(&input)?)
}

fn prepare_wasi_json_input(input: serde_json::Value) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&input)?)
}

fn run_wasm_api_example(example: &str, input: serde_json::Value) -> Result<serde_json::Value> {
    let input_bytes = prepare_wasm_api_input(input)?;
    let (output, _fuel) = run_example(example, input_bytes, false)?;
    decode_msgpack_output(output)
}

static ECHO_EXAMPLE_RESULT: LazyLock<Result<()>> = LazyLock::new(|| prepare_example("echo"));
static BENCHMARK_EXAMPLE_RESULT: LazyLock<Result<()>> =
    LazyLock::new(|| prepare_example("cart-checkout-validation-wasm-api"));
static BENCHMARK_NON_WASM_API_EXAMPLE_RESULT: LazyLock<Result<()>> =
    LazyLock::new(|| prepare_example("cart-checkout-validation-wasi-json"));

#[test]
fn test_echo_with_bool_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!(true))?,
        serde_json::json!(true)
    );
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!(false))?,
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
        run_wasm_api_example("echo", serde_json::json!(null))?,
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
            run_wasm_api_example("echo", serde_json::json!(i))?,
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
                run_wasm_api_example("echo", serde_json::json!(f))?,
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
        run_wasm_api_example("echo", serde_json::json!("Hello, world!"))?,
        serde_json::json!("Hello, world!")
    );
    Ok(())
}

#[test]
fn test_echo_with_obj_input_with_interned_strings() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!({ "foo": 1, "bar": 2 }))?,
        serde_json::json!({ "foo": 1, "bar": 2 })
    );
    Ok(())
}

#[test]
fn test_echo_with_obj_input_with_get_obj_prop() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!({ "abc": 1, "def": 2 }))?,
        serde_json::json!({ "abc": 1, "def": 2 })
    );
    Ok(())
}

#[test]
fn test_echo_with_obj_input_with_get_at_index() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!({ "uvw": 1, "xyz": 2 }))?,
        serde_json::json!({ "uvw": 1, "xyz": 2 })
    );
    Ok(())
}

#[test]
fn test_echo_with_array_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!([1, 2, 3]))?,
        serde_json::json!([1, 2, 3])
    );
    Ok(())
}

/// Generates a cart with the specified number of items for testing.
///
/// # Arguments
/// * `size` - The number of items to generate in the cart
/// * `traverse_all` - Controls whether the cart validation should process all items or can exit early
fn generate_cart_with_size(size: usize, traverse_all: bool) -> serde_json::Value {
    let mut lines = Vec::with_capacity(size);

    for i in 0..size {
        lines.push(serde_json::json!({
            "quantity": if traverse_all { 1 } else { 2 },
            "merchandise": {
                "id": format!("gid://shopify/ProductVariant/{}", i + 1),
                "title": format!("Sample Product {}", i + 1)
            }
        }));
    }

    serde_json::json!({
        "cart": {
            "lines": lines
        }
    })
}

#[test]
fn test_echo_with_large_string_input() -> Result<()> {
    ECHO_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    let large_string = "a".repeat(u16::MAX as usize);
    assert_eq!(
        run_wasm_api_example("echo", serde_json::json!(large_string))?,
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
        run_wasm_api_example("echo", serde_json::json!(large_array))?,
        serde_json::json!(large_array)
    );
    Ok(())
}

#[test]
fn test_fuel_consumption_within_threshold() -> Result<()> {
    BENCHMARK_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    let input = generate_cart_with_size(2, true);
    let wasm_api_input = prepare_wasm_api_input(input.clone())?;
    let (_, wasm_api_fuel) =
        run_example("cart-checkout-validation-wasm-api", wasm_api_input, false)?;
    eprintln!("WASM API fuel: {}", wasm_api_fuel);
    // Using a target fuel value as reference similar to the Javy example
    assert_fuel_consumed_within_threshold(13479, wasm_api_fuel);
    Ok(())
}

#[test]
fn test_benchmark_comparison_with_input() -> Result<()> {
    BENCHMARK_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    BENCHMARK_NON_WASM_API_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare non-WASM API example: {}", e))?;

    let input = generate_cart_with_size(2, true);

    let wasm_api_input = prepare_wasm_api_input(input.clone())?;
    let (wasm_api_output, wasm_api_fuel) =
        run_example("cart-checkout-validation-wasm-api", wasm_api_input, false)?;
    let wasm_api_value = decode_msgpack_output(wasm_api_output)?;

    let wasi_json_input = prepare_wasi_json_input(input)?;
    let (non_wasm_api_output, non_wasm_api_fuel) =
        run_example("cart-checkout-validation-wasi-json", wasi_json_input, true)?;
    let non_wasm_api_value = decode_json_output(non_wasm_api_output)?;

    assert_eq!(wasm_api_value, non_wasm_api_value);
    assert!(
        wasm_api_fuel < non_wasm_api_fuel,
        "WASM API fuel usage ({}) should be less than non-WASM API fuel usage ({})",
        wasm_api_fuel,
        non_wasm_api_fuel
    );

    let improvement =
        ((non_wasm_api_fuel as f64 - wasm_api_fuel as f64) / non_wasm_api_fuel as f64) * 100.0;
    println!(
        "WASM API fuel: {}, Non-WASM API fuel: {}, Improvement: {:.2}%",
        wasm_api_fuel, non_wasm_api_fuel, improvement
    );

    assert_fuel_consumed_within_threshold(13479, wasm_api_fuel);
    assert_fuel_consumed_within_threshold(23858, non_wasm_api_fuel);

    Ok(())
}

#[test]
fn test_benchmark_comparison_with_input_early_exit() -> Result<()> {
    BENCHMARK_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    BENCHMARK_NON_WASM_API_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare non-WASM API example: {}", e))?;

    let input = generate_cart_with_size(100, false);

    let wasm_api_input = prepare_wasm_api_input(input.clone())?;
    let (wasm_api_output, wasm_api_fuel) =
        run_example("cart-checkout-validation-wasm-api", wasm_api_input, false)?;
    let wasm_api_value = decode_msgpack_output(wasm_api_output)?;

    let wasi_json_input = prepare_wasi_json_input(input)?;
    let (non_wasm_api_output, non_wasm_api_fuel) =
        run_example("cart-checkout-validation-wasi-json", wasi_json_input, true)?;
    let non_wasm_api_value = decode_json_output(non_wasm_api_output)?;

    assert_eq!(wasm_api_value, non_wasm_api_value);
    assert!(
        wasm_api_fuel < non_wasm_api_fuel,
        "WASM API fuel usage ({}) should be less than non-WASM API fuel usage ({})",
        wasm_api_fuel,
        non_wasm_api_fuel
    );

    let improvement =
        ((non_wasm_api_fuel as f64 - wasm_api_fuel as f64) / non_wasm_api_fuel as f64) * 100.0;
    println!(
        "WASM API fuel: {}, Non-WASM API fuel: {}, Improvement: {:.2}%",
        wasm_api_fuel, non_wasm_api_fuel, improvement
    );

    // Add fuel consumption threshold checks for both implementations
    assert_fuel_consumed_within_threshold(13485, wasm_api_fuel);
    assert_fuel_consumed_within_threshold(736695, non_wasm_api_fuel);

    Ok(())
}
