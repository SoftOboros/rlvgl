/*!
Build script for vendor chip database crates.

Reads board definition files from the directory specified by the
`RLVGL_CHIP_SRC` environment variable and packs them into a single
binary blob at build time.
*/
use std::{env, fs, io::Write, path::PathBuf};

/// Compresses `input` into `output` using zstd level 19.
fn compress_zstd(input: &PathBuf, output: &PathBuf) {
    let data = fs::read(input).expect("read uncompressed");
    let file = fs::File::create(output).expect("create zst");
    let mut encoder = zstd::Encoder::new(file, 19).expect("encoder");
    encoder.write_all(&data).expect("write zst");
    encoder.finish().expect("finish zst");
}

fn copy_asset(out_dir: &PathBuf) -> bool {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let asset = manifest.join("assets/chipdb.bin.zst");
    if asset.exists() {
        println!("cargo:rerun-if-changed={}", asset.display());
        fs::copy(asset, out_dir.join("chipdb.bin.zst")).expect("copy asset");
        return true;
    }
    false
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    if copy_asset(&out_dir) {
        return;
    }
    println!("cargo:rerun-if-env-changed=RLVGL_CHIP_SRC");
    let out = out_dir.join("chipdb.bin");
    let src_dir = env::var("RLVGL_CHIP_SRC").ok();
    let mut out_file = fs::File::create(&out).expect("create output");
    if let Some(dir) = src_dir {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                let entry = entry.expect("entry");
                if entry.file_type().expect("ft").is_file() {
                    let name = entry.file_name().into_string().unwrap();
                    writeln!(out_file, ">{}", name).unwrap();
                    let data = fs::read(entry.path()).expect("read file");
                    out_file.write_all(&data).unwrap();
                    if !data.ends_with(b"\n") {
                        writeln!(out_file).unwrap();
                    }
                    writeln!(out_file, "<").unwrap();
                }
            }
        }
    }
    let zst = out_dir.join("chipdb.bin.zst");
    compress_zstd(&out, &zst);
}
