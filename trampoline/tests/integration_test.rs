use anyhow::Result;
use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use uuid::Uuid;

fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("..")
}

fn echo_module_path() -> PathBuf {
    example_module_path("echo")
}

fn generate_output_path() -> PathBuf {
    example_module_path(&format!("{}.merged", Uuid::new_v4()))
}

fn example_module_path(name: &str) -> PathBuf {
    let workspace_root = workspace_root();
    workspace_root
        .join("target/wasm32-wasip1/release/examples")
        .join(format!("{}.wasm", name))
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
fn test_cli_trampolines_wasm_module() -> Result<()> {
    ECHO_EXAMPLE
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build example: {}", e))?;
    let output_path = generate_output_path();

    // Run the trampoline-cli on the example
    let status = Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .args([
            "--input",
            echo_module_path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()?;

    assert!(status.success(), "Trampoline CLI failed to run");
    assert!(output_path.exists(), "Output file was not created");

    Ok(())
}

#[test]
fn test_outputs_error_if_input_does_not_exist() -> Result<()> {
    let output_path = generate_output_path();
    let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .args([
            "--input",
            "non-existent-module.wasm",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        !output.status.success(),
        "Trampoline CLI should have failed"
    );
    assert!(!output_path.exists(), "An output file was created");
    assert!(
        String::from_utf8(output.stderr)?.contains("No such file or directory"),
        "Expected missing file error"
    );

    Ok(())
}

#[test]
fn test_overwrites_existing_output_file() -> Result<()> {
    ECHO_EXAMPLE
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build example: {}", e))?;
    let output_path = generate_output_path();

    // Create empty file at output path
    std::fs::write(&output_path, "")?;
    assert_eq!(
        output_path.metadata()?.len(),
        0,
        "Initial output file should be empty"
    );

    // Run the trampoline-cli on the example
    let status = Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .args([
            "--input",
            echo_module_path().to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()?;

    assert!(status.success(), "Trampoline CLI failed to run");
    assert_ne!(
        output_path.metadata()?.len(),
        0,
        "Initial output file was not overwritten"
    );

    Ok(())
}
