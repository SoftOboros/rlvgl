fn main() {
    // Tell the linker where to find memory.x
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::fs::copy(
        format!("{manifest_dir}/memory.x"),
        format!("{out_dir}/memory.x"),
    )
    .ok();
    println!("cargo:rustc-link-search={out_dir}");
    println!("cargo:rerun-if-changed=memory.x");
}
