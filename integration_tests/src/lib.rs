use anyhow::{Context, Result};
use std::process::Command;
use std::sync::LazyLock;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest_dir).join("..")
}

fn trampoline_wasm() -> std::path::PathBuf {
    let workspace_root = workspace_root();
    workspace_root.join("trampoline.wasm")
}

/// Builds the trampoline.wasm file from the trampoline.wat file
fn build_trampoline() -> Result<()> {
    let workspace_root = workspace_root();
    let status = Command::new("wasm-tools")
        .current_dir(workspace_root)
        .args(["parse", "trampoline.wat", "-o", "trampoline.wasm"])
        .status()?;
    if !status.success() {
        anyhow::bail!(status);
    }

    Ok(())
}

/// Builds the provider library to a `.wasm` file
fn build_provider() -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip1",
            "-p",
            "shopify_function_wasm_api_provider",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!(status);
    }
    Ok(())
}

/// Builds the example to a `.wasm` file
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
        anyhow::bail!(status);
    }
    Ok(())
}

/// Merges the example with the trampoline
fn merge_example(name: &str) -> Result<()> {
    let workspace_root = workspace_root();
    let examples_dir = workspace_root.join("target/wasm32-wasip1/release/examples");
    let trampoline_wasm = trampoline_wasm();
    let trampoline_wasm = trampoline_wasm
        .to_str()
        .context("Failed to convert path to string")?;
    let example_path = examples_dir.join(name).with_extension("wasm");
    let merged_path = example_path.with_extension("merged.wasm");
    let merged_path = merged_path
        .to_str()
        .context("Failed to convert path to string")?;
    let example_path = example_path
        .to_str()
        .context("Failed to convert path to string")?;

    let status = Command::new("wasm-merge")
        .args([
            "--enable-bulk-memory",
            "--enable-multimemory",
            trampoline_wasm,
            "shopify_function_v0.1.0",
            example_path,
            "function",
            "-o",
            merged_path,
        ])
        .current_dir(workspace_root)
        .status()?;

    if !status.success() {
        anyhow::bail!(status);
    }

    Ok(())
}

static BUILD_TRAMPOLINE_RESULT: LazyLock<Result<()>> = LazyLock::new(build_trampoline);
static BUILD_PROVIDER_RESULT: LazyLock<Result<()>> = LazyLock::new(build_provider);

/// Builds the trampoline, provider, and example, and merges the example with the trampoline
pub fn prepare_example(name: &str) -> Result<()> {
    BUILD_TRAMPOLINE_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build trampoline: {}", e))?;
    BUILD_PROVIDER_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build provider: {}", e))?;
    build_example(name).map_err(|e| anyhow::anyhow!("Failed to build example: {}", e))?;
    merge_example(name).map_err(|e| anyhow::anyhow!("Failed to merge example: {}", e))?;
    Ok(())
}
