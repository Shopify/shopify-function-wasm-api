use std::path::PathBuf;

use clap::Parser;
use shopify_function_wasm_api_trampoline::trampoline_existing_module;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to input Wasm file
    #[arg(short, long)]
    input_path: PathBuf,

    /// Path to output Wasm file. If not provided, output will be written to stdout.
    #[arg(short, long)]
    output_path: PathBuf,
}

fn main() {
    let args = Args::parse();

    trampoline_existing_module(args.input_path, args.output_path).unwrap();
}
