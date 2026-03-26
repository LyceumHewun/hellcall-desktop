use std::env;
use std::path::PathBuf;

fn main() {
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let lib_path = PathBuf::from(dir).join("lib");
    println!("cargo:rustc-link-search=native={}", lib_path.display());

    tauri_build::build()
}
