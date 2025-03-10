fn main() {
    println!("cargo::rerun-if-changed=../trampoline.wat");
    println!("cargo::rerun-if-changed=../provider");
    println!("cargo::rerun-if-changed=../api");
    println!("cargo::rerun-if-changed=build.rs");
}
