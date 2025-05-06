use std::path::PathBuf;

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

    trampoline_existing_module(args.input, args.output)
}
