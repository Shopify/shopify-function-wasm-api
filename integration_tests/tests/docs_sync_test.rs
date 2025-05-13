use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walrus::Module as WalrusModule;

/// Helper to load WASM bytes from a file path, handling both .wasm and .wat files.
fn load_wasm_bytes(relative_path: &str) -> Result<Vec<u8>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = PathBuf::from(manifest_dir).join("..");
    let full_path = workspace_root.join(relative_path);

    let path = Path::new(&full_path);
    let extension = path.extension().and_then(std::ffi::OsStr::to_str);

    match extension {
        Some("wasm") => fs::read(&full_path)
            .with_context(|| format!("Failed to read WASM file: {}", full_path.display())),
        Some("wat") => {
            let wat_content = fs::read_to_string(&full_path)
                .with_context(|| format!("Failed to read WAT file: {}", full_path.display()))?;
            wat::parse_bytes(wat_content.as_bytes())
                .map(|cow| cow.into_owned())
                .with_context(|| format!("Failed to parse WAT file: {}", full_path.display()))
        }
        _ => anyhow::bail!(
            "Unsupported file extension for path: {}",
            full_path.display()
        ),
    }
}

/// Helper to extract imported functions from a WASM or WAT file for a specific module
fn extract_imported_functions(file_path: &str, expected_module: &str) -> Result<HashSet<String>> {
    let wasm_bytes = load_wasm_bytes(file_path)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)
        .with_context(|| format!("Failed to parse WASM/WAT bytes from: {}", file_path))?;

    let imports: HashSet<String> = module
        .imports
        .iter()
        .filter(|import| import.module == expected_module)
        .map(|import| import.name.clone())
        .collect();

    Ok(imports)
}

/// Helper to get all module names used in a WASM or WAT file's imports
fn extract_imported_modules(file_path: &str) -> Result<HashSet<String>> {
    let wasm_bytes = load_wasm_bytes(file_path)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)
        .with_context(|| format!("Failed to parse WASM/WAT bytes from: {}", file_path))?;

    let modules: HashSet<String> = module
        .imports
        .iter()
        .map(|import| import.module.clone())
        .collect();

    Ok(modules)
}

/// Test that the module names used in the shopify_function.wat and consumer.wat files are consistent
#[test]
fn test_wat_module_name_consistency() -> Result<()> {
    let shopify_function_wat_path = "api/src/shopify_function.wat"; // Path relative to workspace root
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat"; // Path relative to workspace root

    let shopify_function_module_names = extract_imported_modules(shopify_function_wat_path)?;
    let consumer_module_names = extract_imported_modules(consumer_wat_path)?;

    assert!(
        !shopify_function_module_names.is_empty(),
        "No import modules found in {}",
        shopify_function_wat_path
    );
    assert!(
        !consumer_module_names.is_empty(),
        "No import modules found in {}",
        consumer_wat_path
    );

    assert_eq!(
        shopify_function_module_names,
        consumer_module_names,
        "Import module name mismatch between {} and {}:\nShopify Function Modules: {:?}\nConsumer Modules:  {:?}",
        shopify_function_wat_path,
        consumer_wat_path,
        shopify_function_module_names,
        consumer_module_names
    );

    // Optional: Check if there's only one unique module name used (as expected)
    assert_eq!(
        shopify_function_module_names.len(),
        1,
        "Expected only one unique import module name in {}, found: {:?}",
        shopify_function_wat_path,
        shopify_function_module_names
    );

    Ok(())
}

/// Test that the function names used in the shopify_function.wat and consumer.wat files are consistent
#[test]
fn test_wat_function_name_consistency() -> Result<()> {
    let shopify_function_wat_path = "api/src/shopify_function.wat";
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat";

    // Assuming the module name consistency test passed and there's only one module name,
    // we extract it to filter function names.
    let module_names = extract_imported_modules(shopify_function_wat_path)?;
    let expected_module_name = module_names.iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "No module name found in {} to use for function name comparison",
            shopify_function_wat_path
        )
    })?;

    let shopify_function_import_names =
        extract_imported_functions(shopify_function_wat_path, expected_module_name)?;
    let consumer_import_names =
        extract_imported_functions(consumer_wat_path, expected_module_name)?;

    let missing_in_consumer: Vec<_> = shopify_function_import_names
        .difference(&consumer_import_names)
        .collect();
    let missing_in_shopify_function: Vec<_> = consumer_import_names
        .difference(&shopify_function_import_names)
        .collect();

    let mut error_messages = Vec::new();
    if !missing_in_consumer.is_empty() {
        error_messages.push(format!(
            "Functions in shopify function WAT ({}) but not in consumer WAT ({}) for module '{}': {:?}",
            shopify_function_wat_path, consumer_wat_path, expected_module_name, missing_in_consumer
        ));
    }
    if !missing_in_shopify_function.is_empty() {
        error_messages.push(format!(
            "Functions in consumer WAT ({}) but not in shopify function WAT ({}) for module '{}': {:?}",
            consumer_wat_path, shopify_function_wat_path, expected_module_name, missing_in_shopify_function
        ));
    }

    assert!(
        error_messages.is_empty(),
        "Function name mismatch between {} and {} imports (module '{}'):\n{}",
        shopify_function_wat_path,
        consumer_wat_path,
        expected_module_name,
        error_messages.join("\n")
    );
    assert!(
        !shopify_function_import_names.is_empty(),
        "No functions imported from module {} found in {}",
        expected_module_name,
        shopify_function_wat_path
    );
    assert!(
        !consumer_import_names.is_empty(),
        "No functions imported from module {} found in {}",
        expected_module_name,
        consumer_wat_path
    );

    Ok(())
}

/// Test that the compiled header_test.wasm generated from header_test.c file uses the same import module as consumer.wat
#[test]
fn test_header_wasm_module_name_consistency() -> Result<()> {
    let header_wasm_modules = extract_imported_modules("api/src/test_data/header_test.wasm")?;
    let consumer_wat_modules = extract_imported_modules("trampoline/src/test_data/consumer.wat")?;

    // Print the modules for debugging
    println!("header_test.wasm modules: {:?}", header_wasm_modules);
    println!("consumer.wat modules: {:?}", consumer_wat_modules);

    // Check that the header WASM has exactly one module
    assert_eq!(
        header_wasm_modules.len(),
        1,
        "Expected header_test.wasm to have exactly one import module, got: {:?}",
        header_wasm_modules
    );

    // Check that the consumer WAT has exactly one module
    assert_eq!(
        consumer_wat_modules.len(),
        1,
        "Expected consumer.wat to have exactly one import module, got: {:?}",
        consumer_wat_modules
    );

    // Get the single module from each
    let header_module = header_wasm_modules.iter().next().unwrap();
    let consumer_module = consumer_wat_modules.iter().next().unwrap();

    // Make sure they match
    assert_eq!(
        header_module, consumer_module,
        "Import module mismatch: header_test.wasm uses '{}', consumer.wat uses '{}'",
        header_module, consumer_module
    );

    Ok(())
}

/// Test that the functions imported in compiled header_test.wasm generated from header_test.c match those in consumer.wat
#[test]
fn test_header_wasm_function_name_consistency() -> Result<()> {
    // First get the common module name
    let header_wasm_modules = extract_imported_modules("api/src/test_data/header_test.wasm")?;
    let module_name = header_wasm_modules.iter().next().unwrap();

    // Extract the imports
    let header_wasm_imports =
        extract_imported_functions("api/src/test_data/header_test.wasm", module_name)?;
    let consumer_wat_imports =
        extract_imported_functions("trampoline/src/test_data/consumer.wat", module_name)?;

    // Find mismatches
    let missing_in_consumer: Vec<_> = header_wasm_imports
        .difference(&consumer_wat_imports)
        .collect();
    let missing_in_header: Vec<_> = consumer_wat_imports
        .difference(&header_wasm_imports)
        .collect();

    // Build error messages if mismatches found
    let mut error_messages = Vec::new();

    if !missing_in_consumer.is_empty() {
        error_messages.push(format!(
            "Functions imported in header_test.wasm but missing in consumer.wat: {:?}",
            missing_in_consumer
        ));
    }

    if !missing_in_header.is_empty() {
        error_messages.push(format!(
            "Functions imported in consumer.wat but missing in header_test.wasm: {:?}",
            missing_in_header
        ));
    }

    // Assert no mismatches
    assert!(
        error_messages.is_empty(),
        "Import function mismatches found:\n{}",
        error_messages.join("\n")
    );

    Ok(())
}

/// Compare all three files to ensure complete consistency
#[test]
fn test_consistency_across_all_files() -> Result<()> {
    // First get the common module name
    let header_wasm_modules = extract_imported_modules("api/src/test_data/header_test.wasm")?;
    let module_name = header_wasm_modules.iter().next().unwrap();

    // Extract imports from all three files
    let header_wasm_imports =
        extract_imported_functions("api/src/test_data/header_test.wasm", module_name)?;
    let api_wat_imports = extract_imported_functions("api/src/shopify_function.wat", module_name)?;
    let consumer_wat_imports =
        extract_imported_functions("trampoline/src/test_data/consumer.wat", module_name)?;

    // Create a map to track which functions appear in which files
    let mut function_map: HashMap<String, [bool; 3]> = HashMap::new();

    // Register all functions from all files
    for func in &header_wasm_imports {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[0] = true; // header_test.wasm
    }

    for func in &api_wat_imports {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[1] = true; // shopify_function.wat
    }

    for func in &consumer_wat_imports {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[2] = true; // consumer.wat
    }

    // Look for any inconsistencies
    let mut inconsistencies = Vec::new();

    for (func, presence) in &function_map {
        if presence[0] != presence[1] || presence[1] != presence[2] {
            inconsistencies.push(format!(
                "Function '{}' presence inconsistent: header_test.wasm={}, shopify_function.wat={}, consumer.wat={}",
                func, presence[0], presence[1], presence[2]
            ));
        }
    }

    // Assert no inconsistencies
    assert!(
        inconsistencies.is_empty(),
        "Function inconsistencies found across files:\n{}",
        inconsistencies.join("\n")
    );

    Ok(())
}
