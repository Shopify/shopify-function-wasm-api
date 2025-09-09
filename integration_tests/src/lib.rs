use anyhow::Result;
use std::process::Command;
use std::sync::LazyLock;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest_dir).join("..")
}

/// Builds the provider library to a `.wasm` file
fn build_provider() -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "shopify_function_provider",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!(status);
    }
    Ok(())
}

/// Builds the example to a `.wasm` file
fn build_example(name: &str, target: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            target,
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

/// Applies the trampoline to the example
fn apply_trampoline_to_example(name: &str, target: &str) -> Result<()> {
    let workspace_root = workspace_root();
    let examples_dir = workspace_root
        .join("target")
        .join(target)
        .join("release/examples");
    let example_path = examples_dir.join(name).with_extension("wasm");
    let merged_path = example_path.with_extension("merged.wasm");
    shopify_function_trampoline::trampoline_existing_module(example_path, merged_path)?;

    Ok(())
}

static BUILD_PROVIDER_RESULT: LazyLock<Result<()>> = LazyLock::new(build_provider);

/// Builds the trampoline, provider, and example, and merges the example with the trampoline
pub fn prepare_example(name: &str, target: &str) -> Result<()> {
    BUILD_PROVIDER_RESULT
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Failed to build provider: {}", e))?;
    build_example(name, target).map_err(|e| anyhow::anyhow!("Failed to build example: {}", e))?;
    apply_trampoline_to_example(name, target)
        .map_err(|e| anyhow::anyhow!("Failed to apply trampoline: {}", e))?;
    Ok(())
}
