use anyhow::Result;
use conformance_tests::{build_provider, run_wasm_module};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct TestSuite {
    test_cases: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    name: String,
    category: String,
    input: serde_json::Value,
    expected_output: serde_json::Value,
}

static PROVIDER_BUILT: LazyLock<Result<()>> = LazyLock::new(build_provider);

fn wasm_dir() -> PathBuf {
    std::env::var("CONFORMANCE_WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("target")
                .join("conformance_wasm")
        })
}

fn discover_wasm_files() -> Vec<PathBuf> {
    let dir = wasm_dir();
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("Failed to read WASM directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "wasm") {
                // Skip trampolined files (intermediate artifacts)
                let name = path.file_name()?.to_string_lossy();
                if name.contains("trampolined") {
                    return None;
                }
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

fn load_test_cases() -> Vec<TestCase> {
    let test_cases_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_cases.json");
    let content =
        std::fs::read_to_string(&test_cases_path).expect("Failed to read test_cases.json");
    let suite: TestSuite = serde_json::from_str(&content).expect("Failed to parse test_cases.json");
    suite.test_cases
}

#[test]
fn conformance_tests() -> Result<()> {
    PROVIDER_BUILT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build provider: {}", e))?;

    let wasm_files = discover_wasm_files();
    if wasm_files.is_empty() {
        eprintln!(
            "No WASM files found in {:?}. Set CONFORMANCE_WASM_DIR or build SDK examples first.",
            wasm_dir()
        );
        return Ok(());
    }

    let test_cases = load_test_cases();
    let mut failures = Vec::new();

    for wasm_path in &wasm_files {
        let wasm_name = wasm_path.file_stem().unwrap().to_string_lossy().to_string();

        for test_case in &test_cases {
            let result = run_wasm_module(wasm_path, &test_case.input);
            match result {
                Ok((output, _logs)) => {
                    if output != test_case.expected_output {
                        failures.push(format!(
                            "FAIL [{wasm_name}] {}/{}: expected {}, got {}",
                            test_case.category, test_case.name, test_case.expected_output, output
                        ));
                    }
                }
                Err(e) => {
                    failures.push(format!(
                        "ERROR [{wasm_name}] {}/{}: {}",
                        test_case.category, test_case.name, e
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        let msg = failures.join("\n");
        panic!("Conformance test failures:\n{msg}");
    }

    eprintln!(
        "All conformance tests passed for {} WASM module(s) x {} test cases",
        wasm_files.len(),
        test_cases.len()
    );

    Ok(())
}
