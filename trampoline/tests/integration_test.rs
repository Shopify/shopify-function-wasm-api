use anyhow::Result;
use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("..")
}

fn build_example(name: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "-p",
            "shopify_function_wasm_api",
            "--example",
            name,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to build example");
    }
    Ok(())
}

static ECHO_EXAMPLE: LazyLock<Result<()>> = LazyLock::new(|| build_example("echo"));

#[test]
fn test_trampoline_cli() -> Result<()> {
    ECHO_EXAMPLE
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build example: {}", e))?;
    let workspace_root = workspace_root();
    let input_path = workspace_root
        .join("target/wasm32-wasip1/release/examples")
        .join("echo.wasm");
    let output_path = workspace_root
        .join("target/wasm32-wasip1/release/examples")
        .join("echo.merged.wasm");

    // Run the trampoline-cli on the example
    let status = Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .args([
            "--input",
            input_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()?;

    assert!(status.success(), "Trampoline CLI failed to run");
    assert!(output_path.exists(), "Output file was not created");

    Ok(())
}
