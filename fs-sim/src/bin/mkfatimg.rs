//! Create a FAT disk image from a directory.
//!
//! This CLI utility builds a FAT-formatted disk image of a specified size
//! and populates it with the contents of a host directory. The resulting
//! image can be mounted by [`SimBlockDevice`] for simulator testing.

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use fatfs::{FileSystem, FormatVolumeOptions, FsOptions, ReadWriteSeek};
use walkdir::WalkDir;

/// Parse a human-readable size string like `32M` or `512K` into bytes.
fn parse_size(s: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Some(m) = s.strip_suffix('M') {
        Ok(m.parse::<u64>()? * 1024 * 1024)
    } else if let Some(k) = s.strip_suffix('K') {
        Ok(k.parse::<u64>()? * 1024)
    } else {
        Ok(s.parse::<u64>()?)
    }
}

/// Ensure that all directories for `path` exist within the FAT filesystem.
fn ensure_fat_dirs<'a, F>(fs: &'a FileSystem<F>, path: &Path) -> io::Result<fatfs::Dir<'a, F>>
where
    F: ReadWriteSeek,
{
    let mut dir = fs.root_dir();
    for comp in path.components() {
        let name = comp
            .as_os_str()
            .to_str()
            .ok_or_else(|| io::Error::other("invalid path"))?;
        dir = match dir.open_dir(name) {
            Ok(d) => d,
            Err(_) => dir.create_dir(name)?,
        };
    }
    Ok(dir)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut size = None;
    let mut from = None;
    let mut out = PathBuf::from("fat.img");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => size = Some(parse_size(&args.next().ok_or("missing value for --size")?)?),
            "--from" => {
                from = Some(PathBuf::from(
                    args.next().ok_or("missing value for --from")?,
                ))
            }
            "--out" => out = PathBuf::from(args.next().ok_or("missing value for --out")?),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    let size = size.ok_or("--size is required")?;
    let from = from.ok_or("--from is required")?;

    let mut image = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&out)?;
    image.set_len(size)?;
    fatfs::format_volume(&mut image, FormatVolumeOptions::new())?;
    image.seek(SeekFrom::Start(0))?;
    let fs = FileSystem::new(image, FsOptions::new())?;

    for entry in WalkDir::new(&from) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(&from)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        if entry.file_type().is_dir() {
            ensure_fat_dirs(&fs, rel)?;
        } else {
            let parent = rel.parent().unwrap_or_else(|| Path::new(""));
            let dir = ensure_fat_dirs(&fs, parent)?;
            let name = rel
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("invalid filename")?;
            let mut fat_file = dir.create_file(name)?;
            let mut host_file = File::open(entry.path())?;
            io::copy(&mut host_file, &mut fat_file)?;
        }
    }

    println!("Created {} from {}", out.display(), from.display());
    Ok(())
}
