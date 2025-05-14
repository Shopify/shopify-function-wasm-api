use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walrus::Module as WalrusModule;

/// Helper to convert a HashSet to a sorted Vec for stable snapshotting.
fn sorted_vec_from_hashset<T: Ord + Clone>(set: &HashSet<T>) -> Vec<T> {
    let mut vec: Vec<T> = set.iter().cloned().collect();
    vec.sort();
    vec
}

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

/// Helper to extract "<module_name>.<function_name>" IDs for function imports.
/// Also verifies that all function imports come from a single module if any exist.
fn get_module_function_concatenated_ids(file_path: &str) -> Result<HashSet<String>> {
    let wasm_bytes = load_wasm_bytes(file_path)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)
        .with_context(|| format!("Failed to parse WASM/WAT bytes from: {}", file_path))?;

    let mut function_api_module_names = HashSet::new();
    let mut concatenated_ids = HashSet::new();

    for import_item in module.imports.iter() {
        if matches!(import_item.kind, walrus::ImportKind::Function(_)) {
            function_api_module_names.insert(import_item.module.clone());
            concatenated_ids.insert(format!("{}.{}", import_item.module, import_item.name));
        }
    }

    if function_api_module_names.len() > 1 {
        anyhow::bail!(
            "File '{}' imports functions from multiple modules: {:?}. Expected a single function API module for all function imports.",
            file_path,
            function_api_module_names
        );
    }

    Ok(concatenated_ids)
}

/// Helper to extract imported functions from a WASM or WAT file for a specific module
fn extract_imported_functions(file_path: &str, expected_module: &str) -> Result<HashSet<String>> {
    let wasm_bytes = load_wasm_bytes(file_path)?;
    let module = WalrusModule::from_buffer(&wasm_bytes)
        .with_context(|| format!("Failed to parse WASM/WAT bytes from: {}", file_path))?;

    let imports: HashSet<String> = module
        .imports
        .iter()
        .filter(|import| {
            import.module == expected_module
                && matches!(import.kind, walrus::ImportKind::Function(_))
        })
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

/// Test that the shopify_function.wat and consumer.wat imports are consistent.
#[test]
fn test_shopify_function_wat_imports_consistency() -> Result<()> {
    let shopify_function_wat_path = "api/src/shopify_function.wat";
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat";

    let sf_ids = get_module_function_concatenated_ids(shopify_function_wat_path)?;
    let c_ids = get_module_function_concatenated_ids(consumer_wat_path)?;

    assert!(
        !sf_ids.is_empty(),
        "No function imports (`<module>.<function>`) found in '{}'. It should define the API.",
        shopify_function_wat_path
    );
    assert!(
        !c_ids.is_empty(),
        "No function imports (`<module>.<function>`) found in '{}'. It should consume the API.",
        consumer_wat_path
    );

    let sf_ids_sorted = sorted_vec_from_hashset(&sf_ids);
    let c_ids_sorted = sorted_vec_from_hashset(&c_ids);

    let ids_missing_in_consumer: Vec<_> = sf_ids_sorted
        .iter()
        .filter(|id| !c_ids_sorted.contains(id))
        .cloned()
        .collect();
    let ids_missing_in_shopify_function: Vec<_> = c_ids_sorted
        .iter()
        .filter(|id| !sf_ids_sorted.contains(id))
        .cloned()
        .collect();

    insta::assert_debug_snapshot!(
        "shopify_function_wat_imports_diff",
        (&ids_missing_in_consumer, &ids_missing_in_shopify_function)
    );

    Ok(())
}

/// Test that the compiled header_test.wasm generated from header_test.c file uses the same import module as consumer.wat
#[test]
fn test_header_wasm_module_name_consistency() -> Result<()> {
    let header_wasm_path = "api/src/test_data/header_test.wasm";
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat";

    let header_wasm_modules_set = extract_imported_modules(header_wasm_path)?;
    let consumer_wat_modules_set = extract_imported_modules(consumer_wat_path)?;

    assert_eq!(
        header_wasm_modules_set.len(),
        1,
        "Expected header_test.wasm to have exactly one import module, got: {:?}",
        header_wasm_modules_set
    );
    assert_eq!(
        consumer_wat_modules_set.len(),
        1,
        "Expected consumer.wat to have exactly one import module, got: {:?}",
        consumer_wat_modules_set
    );

    let header_modules_sorted = sorted_vec_from_hashset(&header_wasm_modules_set);
    let consumer_modules_sorted = sorted_vec_from_hashset(&consumer_wat_modules_set);

    let modules_missing_in_consumer: Vec<_> = header_modules_sorted
        .iter()
        .filter(|m| !consumer_modules_sorted.contains(m))
        .cloned()
        .collect();
    let modules_missing_in_header: Vec<_> = consumer_modules_sorted
        .iter()
        .filter(|m| !header_modules_sorted.contains(m))
        .cloned()
        .collect();

    insta::assert_debug_snapshot!(
        "header_wasm_module_name_consistency_diff",
        (&modules_missing_in_consumer, &modules_missing_in_header)
    );

    assert_eq!(
        header_wasm_modules_set,
        consumer_wat_modules_set,
        "Module names MUST be identical between '{}' and '{}'. Review snapshot 'header_wasm_module_name_consistency_diff'.",
        header_wasm_path, consumer_wat_path
    );

    Ok(())
}

/// Test that the functions imported in compiled header_test.wasm generated from header_test.c match those in consumer.wat
#[test]
fn test_header_wasm_function_name_consistency() -> Result<()> {
    let header_wasm_path = "api/src/test_data/header_test.wasm";
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat";

    let header_module_names = extract_imported_modules(header_wasm_path)?;
    let module_name = header_module_names.iter().next().ok_or_else(||
        anyhow::anyhow!("No import module found in '{}', this test assumes module consistency is checked separately.", header_wasm_path)
    )?.clone();

    let consumer_module_names = extract_imported_modules(consumer_wat_path)?;
    assert!(
        consumer_module_names.contains(&module_name) && consumer_module_names.len() == 1,
        "consumer.wat (module(s): {:?}) does not align with the single module name '{}' from header_test.wasm. Ensure module consistency test passes.", 
        consumer_module_names, module_name
    );

    let header_wasm_imports_set = extract_imported_functions(header_wasm_path, &module_name)?;
    let consumer_wat_imports_set = extract_imported_functions(consumer_wat_path, &module_name)?;

    let header_imports_sorted = sorted_vec_from_hashset(&header_wasm_imports_set);
    let consumer_imports_sorted = sorted_vec_from_hashset(&consumer_wat_imports_set);

    let functions_missing_in_consumer: Vec<_> = header_imports_sorted
        .iter()
        .filter(|f| !consumer_imports_sorted.contains(f))
        .cloned()
        .collect();
    let functions_missing_in_header: Vec<_> = consumer_imports_sorted
        .iter()
        .filter(|f| !header_imports_sorted.contains(f))
        .cloned()
        .collect();

    insta::assert_debug_snapshot!(
        "header_wasm_function_name_consistency_diff",
        (&functions_missing_in_consumer, &functions_missing_in_header)
    );

    Ok(())
}

/// Compare all three files to ensure complete consistency
#[test]
fn test_consistency_across_all_files() -> Result<()> {
    let header_wasm_path = "api/src/test_data/header_test.wasm";
    let api_wat_path = "api/src/shopify_function.wat";
    let consumer_wat_path = "trampoline/src/test_data/consumer.wat";

    let header_wasm_modules_set = extract_imported_modules(header_wasm_path)?;
    let module_name = header_wasm_modules_set.iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "No module name found in {} for overall consistency check",
            header_wasm_path
        )
    })?;

    // Check if all files use the same module name (basic sanity check)
    let api_wat_module_names = extract_imported_modules(api_wat_path)?;
    assert!(
        api_wat_module_names.contains(module_name),
        "api/src/shopify_function.wat does not use module name '{}'",
        module_name
    );
    let consumer_wat_module_names = extract_imported_modules(consumer_wat_path)?;
    assert!(
        consumer_wat_module_names.contains(module_name),
        "trampoline/src/test_data/consumer.wat does not use module name '{}'",
        module_name
    );

    let header_wasm_imports_set = extract_imported_functions(header_wasm_path, module_name)?;
    let api_wat_imports_set = extract_imported_functions(api_wat_path, module_name)?;
    let consumer_wat_imports_set = extract_imported_functions(consumer_wat_path, module_name)?;

    let mut function_map: HashMap<String, [bool; 3]> = HashMap::new();

    for func in &header_wasm_imports_set {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[0] = true;
    }
    for func in &api_wat_imports_set {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[1] = true;
    }
    for func in &consumer_wat_imports_set {
        let entry = function_map.entry(func.clone()).or_insert([false; 3]);
        entry[2] = true;
    }

    let mut inconsistencies = Vec::new();
    // To ensure stable snapshots, collect to a BTreeMap (sorted by function name)
    // or sort the final inconsistencies vector.
    // Let's sort the final vector of strings.
    let mut sorted_function_names: Vec<String> = function_map.keys().cloned().collect();
    sorted_function_names.sort();

    for func_name in sorted_function_names {
        if let Some(presence) = function_map.get(&func_name) {
            if presence[0] != presence[1] || presence[1] != presence[2] {
                inconsistencies.push(format!(
                    "Function '{}' presence inconsistent: header_test.wasm={}, shopify_function.wat={}, consumer.wat={}",
                    func_name, presence[0], presence[1], presence[2]
                ));
            }
        }
    }
    // No need to sort `inconsistencies` again if derived from sorted_function_names in order.

    insta::assert_debug_snapshot!("all_files_consistency_issues", inconsistencies);

    Ok(())
}
