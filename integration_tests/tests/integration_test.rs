use anyhow::Result;
use integration_tests::prepare_example;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use walrus::Module as WalrusModule;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{
    pipe::{MemoryInputPipe, MemoryOutputPipe},
    WasiCtxBuilder,
};
use wat::parse_str;

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
        assert!(
            false,
            "fuel_consumed ({}) was significantly better than target_fuel value ({}) by more than {:.2}%. This is a significant improvement! Please double check your changes and update the target fuel if this is a legitimate improvement.",
            fuel_consumed,
            target_fuel,
            THRESHOLD_PERCENTAGE
        );
    }
}

fn run_example(example: &str, input_bytes: Vec<u8>) -> Result<(Vec<u8>, u64)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");
    let engine = Engine::new(&Config::new().consume_fuel(true))?;

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

    let stdin = MemoryInputPipe::new(input_bytes);
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
    store.set_fuel(STARTING_FUEL)?;

    let provider_instance = linker.instantiate(&mut store, &provider)?;
    linker.instance(
        &mut store,
        shopify_function_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    let instance = linker.instantiate(&mut store, &module)?;

    let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

    let result = func.call(&mut store, ());

    let instructions = STARTING_FUEL.saturating_sub(store.get_fuel().unwrap_or_default());

    drop(store);

    if let Err(e) = result {
        let error = stderr.contents().to_vec();
        return Err(anyhow::anyhow!(
            "{}

STDERR:
{}",
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
    let (output, _fuel) = run_example(example, input_bytes)?;
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

<<<<<<< HEAD
#[test]
fn test_fuel_consumption_within_threshold() -> Result<()> {
    BENCHMARK_EXAMPLE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to prepare example: {}", e))?;
    let input = generate_cart_with_size(2, true);
    let wasm_api_input = prepare_wasm_api_input(input.clone())?;
    let (_, wasm_api_fuel) = run_example("cart-checkout-validation-wasm-api", wasm_api_input)?;
    eprintln!("WASM API fuel: {}", wasm_api_fuel);
    // Using a target fuel value as reference similar to the Javy example
    assert_fuel_consumed_within_threshold(16522, wasm_api_fuel);
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
        run_example("cart-checkout-validation-wasm-api", wasm_api_input)?;
    let wasm_api_value = decode_msgpack_output(wasm_api_output)?;

    let wasi_json_input = prepare_wasi_json_input(input)?;
    let (non_wasm_api_output, non_wasm_api_fuel) =
        run_example("cart-checkout-validation-wasi-json", wasi_json_input)?;
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

    assert_fuel_consumed_within_threshold(16522, wasm_api_fuel);
    assert_fuel_consumed_within_threshold(26043, non_wasm_api_fuel);
=======
// --- Consistency Tests ---

// Helper to get the set of unique module names used in imports in a WAT file
fn get_wat_import_module_names(wat_file_path_str: &str) -> Result<HashSet<String>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wat_file_path = PathBuf::from(manifest_dir).join(wat_file_path_str);
    let wat_content = fs::read_to_string(&wat_file_path)
         .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", wat_file_path.display(), e))?;
    let wasm_bytes = parse_str(&wat_content)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)?;
    let module_names: HashSet<String> = module.imports.iter().map(|imp| imp.module.clone()).collect();
    // Ensure we found at least one module name if the file isn't empty of imports
    if !module_names.is_empty() || module.imports.iter().count() == 0 {
        Ok(module_names)
    } else {
        Err(anyhow::anyhow!("No import module names found in {} although imports exist", wat_file_path.display()))
    }
}

// Helper to get imported function names from a WAT file for a specific module
fn get_wat_imported_function_names(wat_file_path_str: &str, expected_module: &str) -> Result<HashSet<String>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wat_file_path = PathBuf::from(manifest_dir).join(wat_file_path_str);
    let wat_content = fs::read_to_string(&wat_file_path)
         .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", wat_file_path.display(), e))?;
    let wasm_bytes = parse_str(&wat_content)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)?;
    let mut wat_names = HashSet::new();
    for import in module.imports.iter() { 
        if import.module == expected_module {
            wat_names.insert(import.name.clone());
        }
    }
    Ok(wat_names)
}

#[test]
fn test_interface_vs_consumer_module_name_consistency() -> Result<()> {
    let interface_wat_path = "../api/src/shopify_function.wat"; // Path relative to integration_tests
    let consumer_wat_path = "../trampoline/src/consumer.wat"; // Path relative to integration_tests

    let interface_module_names = get_wat_import_module_names(interface_wat_path)?;
    let consumer_module_names = get_wat_import_module_names(consumer_wat_path)?;

    assert!(
        !interface_module_names.is_empty(),
        "No import modules found in {}", interface_wat_path
    );
    assert!(
        !consumer_module_names.is_empty(),
        "No import modules found in {}", consumer_wat_path
    );

    assert_eq!(
        interface_module_names,
        consumer_module_names,
        "Import module name mismatch between {} and {}:\nInterface Modules: {:?}\nConsumer Modules:  {:?}",
        interface_wat_path,
        consumer_wat_path,
        interface_module_names,
        consumer_module_names
    );

    // Optional: Check if there's only one unique module name used (as expected)
    assert_eq!(interface_module_names.len(), 1, 
        "Expected only one unique import module name in {}, found: {:?}", 
        interface_wat_path, interface_module_names);
>>>>>>> 194f6ce (fixing docs and simplifying tests)

    Ok(())
}

#[test]
<<<<<<< HEAD
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
        run_example("cart-checkout-validation-wasm-api", wasm_api_input)?;
    let wasm_api_value = decode_msgpack_output(wasm_api_output)?;

    let wasi_json_input = prepare_wasi_json_input(input)?;
    let (non_wasm_api_output, non_wasm_api_fuel) =
        run_example("cart-checkout-validation-wasi-json", wasi_json_input)?;
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
    assert_fuel_consumed_within_threshold(18486, wasm_api_fuel);
    assert_fuel_consumed_within_threshold(807817, non_wasm_api_fuel);

    Ok(())
}
=======
fn test_interface_vs_consumer_wat_function_name_consistency() -> Result<()> {
    let interface_wat_path = "../api/src/shopify_function.wat"; 
    let consumer_wat_path = "../trampoline/src/consumer.wat"; 

    // Assuming the module name consistency test passed and there's only one module name,
    // we extract it to filter function names.
    let module_names = get_wat_import_module_names(interface_wat_path)?;
    let expected_module_name = module_names.iter().next().ok_or_else(|| anyhow::anyhow!("No module name found in {} to use for function name comparison", interface_wat_path))?;

    let interface_import_names = get_wat_imported_function_names(interface_wat_path, expected_module_name)?;
    let consumer_import_names = get_wat_imported_function_names(consumer_wat_path, expected_module_name)?;

    let missing_in_consumer: Vec<_> = interface_import_names.difference(&consumer_import_names).collect();
    let missing_in_interface: Vec<_> = consumer_import_names.difference(&interface_import_names).collect();

    let mut error_messages = Vec::new();
    if !missing_in_consumer.is_empty() {
        error_messages.push(format!(
            "Functions in interface WAT ({}) but not in consumer WAT ({}) for module '{}': {:?}",
            interface_wat_path, consumer_wat_path, expected_module_name, missing_in_consumer
        ));
    }
    if !missing_in_interface.is_empty() {
        error_messages.push(format!(
            "Functions in consumer WAT ({}) but not in interface WAT ({}) for module '{}': {:?}",
            consumer_wat_path, interface_wat_path, expected_module_name, missing_in_interface
        ));
    }

    assert!(
        error_messages.is_empty(),
        "Function name mismatch between {} and {} imports (module '{}'):\n{}",
        interface_wat_path, consumer_wat_path, expected_module_name, error_messages.join("\n")
    );
    assert!(
        !interface_import_names.is_empty(),
        "No functions imported from module {} found in {}",
        expected_module_name, interface_wat_path
    );
     assert!(
        !consumer_import_names.is_empty(),
        "No functions imported from module {} found in {}",
        expected_module_name, consumer_wat_path
    );

    Ok(())
}
>>>>>>> 194f6ce (fixing docs and simplifying tests)
