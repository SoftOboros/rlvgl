//! List or print files from a FAT disk image.
//!
//! This utility mounts a FAT filesystem stored in a disk image and either
//! lists the directory contents or prints a file to standard output.

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use fatfs::{FileSystem, FsOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let image = args.next().ok_or("usage: fatcat <image> [path]")?;
    let path = args.next().map(PathBuf::from);

    let file = File::open(image)?;
    let fs = FileSystem::new(file, FsOptions::new())?;
    let root = fs.root_dir();

    match path {
        Some(p) if !p.as_os_str().is_empty() => {
            let path_str = p.to_string_lossy();
            let path = path_str.as_ref();
            if let Ok(dir) = root.open_dir(path) {
                for entry in dir.iter() {
                    let entry = entry?;
                    println!("{}", entry.file_name());
                }
            } else {
                let mut f = root.open_file(path)?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                io::stdout().write_all(&buf)?;
            }
        }
        _ => {
            for entry in root.iter() {
                let entry = entry?;
                println!("{}", entry.file_name());
            }
        }
    }

    Ok(())
}
