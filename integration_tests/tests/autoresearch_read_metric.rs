//! FROZEN autoresearch evaluation harness. DO NOT MODIFY during experiments.
//!
//! Prints `METRIC=<total fuel>` (lower is better) for a read-heavy workload mix,
//! asserting output correctness so broken reads fail instead of scoring.
//! Composition (dominated by the 100-line scans, i.e. per-line read cost):
//!   A benchmark 2-line cart, optimized encoding (fixed costs)
//!   B benchmark 2-line cart, plain encoding
//!   C benchmark 100-line traverse-all, optimized (per-line, shaped/strref)
//!   D benchmark 100-line traverse-all, plain (per-line, inline maps)
//!   E benchmark 100-line early-exit, optimized (skip-past-unread)
//!   F log-len(1) (root scalar read + minimal run)
//!
//! Run: cargo test -p integration_tests --test autoresearch_read_metric -- --nocapture

use anyhow::Result;
use integration_tests::prepare_example;
use std::sync::LazyLock;
use wasmtime::{Config, Engine, Linker, Module, Store};

const STARTING_FUEL: u64 = u64::MAX;

fn run_example(example: &str, input_bytes: Vec<u8>) -> Result<(Vec<u8>, u64)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::PathBuf::from(manifest_dir).join("..");
    let engine = Engine::new(Config::new().consume_fuel(true))?;

    let module_path = workspace_root.join(format!(
        "target/wasm32-unknown-unknown/release/examples/{example}.merged.wasm"
    ));
    let module = Module::from_file(&engine, workspace_root.join(module_path))?;
    let provider = Module::from_file(
        &engine,
        workspace_root.join("target/wasm32-unknown-unknown/release/shopify_function_provider.wasm"),
    )?;

    let mut linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let provider_instance = linker.instantiate(&mut store, &provider)?;
    store.set_fuel(STARTING_FUEL)?;
    let init_func = provider_instance.get_typed_func::<i32, i32>(&mut store, "initialize")?;
    let input_buffer_offset = init_func.call(&mut store, input_bytes.len() as _)?;
    provider_instance
        .get_memory(&mut store, "memory")
        .unwrap()
        .write(&mut store, input_buffer_offset as usize, &input_bytes)?;
    linker.instance(
        &mut store,
        shopify_function_provider::PROVIDER_MODULE_NAME,
        provider_instance,
    )?;

    store.set_fuel(STARTING_FUEL)?;
    let instance = linker.instantiate(&mut store, &module)?;
    let func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    func.call(&mut store, ())?;
    let fuel = STARTING_FUEL.saturating_sub(store.get_fuel().unwrap_or_default());

    let results_offset = provider_instance
        .get_typed_func::<(), u32>(&mut store, "finalize")?
        .call(&mut store, ())?;
    let memory = provider_instance.get_memory(&mut store, "memory").unwrap();
    let mut buf = [0; 8];
    memory.read(&store, results_offset as usize, &mut buf)?;
    let output_offset = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let output_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let mut output = vec![0; output_len];
    memory.read(&store, output_offset, &mut output)?;

    Ok((output, fuel))
}

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
    serde_json::json!({ "cart": { "lines": lines } })
}

fn expected_output(errors: bool) -> serde_json::Value {
    if errors {
        serde_json::json!({ "errors": [{
            "localizedMessage": "Not possible to order more than one of each",
            "target": "$.cart"
        }]})
    } else {
        serde_json::json!({ "errors": [] })
    }
}

static PREPARED: LazyLock<Result<()>> = LazyLock::new(|| {
    prepare_example("cart-checkout-validation-wasm-api")?;
    prepare_example("log-len")?;
    Ok(())
});

#[test]
fn autoresearch_read_metric() -> Result<()> {
    PREPARED
        .as_ref()
        .map_err(|e| anyhow::anyhow!("prepare failed: {e}"))?;

    let mut total: u64 = 0;
    let bench = "cart-checkout-validation-wasm-api";

    // A + B: 2-line cart, both encodings.
    let small = generate_cart_with_size(2, true);
    for (label, bytes) in [
        ("A", fbf::to_vec_optimized(&small)?),
        ("B", fbf::to_vec(&small)?),
    ] {
        let (out, fuel) = run_example(bench, bytes)?;
        assert_eq!(
            fbf::from_slice::<serde_json::Value>(&out)?,
            expected_output(false),
            "case {label}"
        );
        eprintln!("CASE {label} fuel={fuel}");
        total += fuel;
    }

    // C + D: 100-line traverse-all, both encodings.
    let large = generate_cart_with_size(100, true);
    for (label, bytes) in [
        ("C", fbf::to_vec_optimized(&large)?),
        ("D", fbf::to_vec(&large)?),
    ] {
        let (out, fuel) = run_example(bench, bytes)?;
        assert_eq!(
            fbf::from_slice::<serde_json::Value>(&out)?,
            expected_output(false),
            "case {label}"
        );
        eprintln!("CASE {label} fuel={fuel}");
        total += fuel;
    }

    // E: 100-line early-exit, optimized.
    let early = generate_cart_with_size(100, false);
    let (out, fuel) = run_example(bench, fbf::to_vec_optimized(&early)?)?;
    assert_eq!(
        fbf::from_slice::<serde_json::Value>(&out)?,
        expected_output(true),
        "case E"
    );
    eprintln!("CASE E fuel={fuel}");
    total += fuel;

    // F: log-len(1) — root scalar read.
    let (_, fuel) = run_example("log-len", fbf::to_vec_optimized(&serde_json::json!(1))?)?;
    eprintln!("CASE F fuel={fuel}");
    total += fuel;

    eprintln!("METRIC={total}");
    Ok(())
}
