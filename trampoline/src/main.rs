use std::{path::PathBuf, process};

use clap::Parser;
use shopify_function_trampoline::trampoline_existing_module;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to input Wasm file
    #[arg(short, long)]
    input: PathBuf,

    /// Path to output Wasm file
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Err(err) = trampoline_existing_module(args.input, args.output) {
        eprintln!("Error: {err:?}");
        process::exit(1);
    }
    Ok(())
}
