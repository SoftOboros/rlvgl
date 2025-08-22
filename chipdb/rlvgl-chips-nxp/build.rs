/*!
Build script for vendor chip database crates.

Reads board definition files from the directory specified by the
`RLVGL_CHIP_SRC` environment variable and packs them into a single
binary blob at build time.
*/
use std::{env, fs, io::Write, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=RLVGL_CHIP_SRC");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("chipdb.bin");
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
                    writeln!(out_file, "<").unwrap();
                }
            }
        }
    }
}
