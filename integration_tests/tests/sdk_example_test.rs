use anyhow::{Context, Result};
use conformance_tests::{build_provider, run_wasm_module_export};
use std::path::PathBuf;
use std::sync::LazyLock;

static PROVIDER_BUILT: LazyLock<Result<()>> = LazyLock::new(build_provider);

fn sdk_wasm() -> Option<PathBuf> {
    std::env::var_os("SHOPIFY_FUNCTION_SDK_WASM").map(PathBuf::from)
}

#[test]
fn sdk_example_targets_match_the_abi_contract() -> Result<()> {
    let Some(wasm_path) = sdk_wasm() else {
        eprintln!("SHOPIFY_FUNCTION_SDK_WASM is not set; skipping external SDK integration test");
        return Ok(());
    };

    PROVIDER_BUILT
        .as_ref()
        .map_err(|error| anyhow::anyhow!("failed to build provider: {error}"))?;

    let (output, logs) = run_wasm_module_export(
        &wasm_path,
        "target_a",
        &serde_json::json!({
            "id": "gid://shopify/Product/123",
            "num": 42,
            "name": "Test Product"
        }),
    )
    .with_context(|| format!("target_a failed for {}", wasm_path.display()))?;
    assert_eq!(output, serde_json::json!({"status": 200}));
    assert!(logs.contains("In target_a"));

    let (output, logs) = run_wasm_module_export(
        &wasm_path,
        "target_b",
        &serde_json::json!({
            "id": "gid://shopify/Product/123",
            "targetAResult": 200
        }),
    )
    .with_context(|| format!("target_b failed for {}", wasm_path.display()))?;
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
    assert!(logs.contains("In target_b"));

    Ok(())
}
