//! Utilities for working with FAT filesystem images.
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use fatfs::{FileSystem, FsOptions};
use fscommon::BufStream;
use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(test)]
use fatfs::FormatVolumeOptions;
#[cfg(test)]
use std::io::Cursor;

/// List files in the directory `dir` of a FAT image.
///
/// The image must be formatted before calling this function.
/// Passing `"/"` or an empty string will list the root directory.
pub fn list_dir<T>(mut image: T, dir: &str) -> std::io::Result<Vec<String>>
where
    T: Read + Write + Seek,
{
    image.seek(SeekFrom::Start(0))?;
    let buf_stream = BufStream::new(image);
    let fs = FileSystem::new(buf_stream, FsOptions::new())?;
    let root = fs.root_dir();
    let mut names = Vec::new();
    if dir.is_empty() || dir == "/" {
        for r in root.iter() {
            let entry = r?;
            names.push(entry.file_name().to_string());
        }
    } else {
        let subdir = root.open_dir(dir)?;
        for r in subdir.iter() {
            let entry = r?;
            names.push(entry.file_name().to_string());
        }
    }
    Ok(names)
}

/// Determine whether a file exists at `path` within the image.
pub fn file_exists<T>(mut image: T, path: &str) -> std::io::Result<bool>
where
    T: Read + Write + Seek,
{
    image.seek(SeekFrom::Start(0))?;
    let buf_stream = BufStream::new(image);
    let fs = FileSystem::new(buf_stream, FsOptions::new())?;
    Ok(fs.root_dir().open_file(path).is_ok())
}

/// Read the entire contents of the file at `path` within the image.
pub fn read_file<T>(mut image: T, path: &str) -> std::io::Result<Vec<u8>>
where
    T: Read + Write + Seek,
{
    image.seek(SeekFrom::Start(0))?;
    let buf_stream = BufStream::new(image);
    let fs = FileSystem::new(buf_stream, FsOptions::new())?;
    let mut file = fs.root_dir().open_file(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

/// Read a slice of the file at `path` starting at byte `offset`.
///
/// Returns up to `len` bytes. If the file is smaller than `offset + len`,
/// the returned buffer will contain all available bytes from `offset` to end.
pub fn read_file_range<T>(
    mut image: T,
    path: &str,
    offset: u64,
    len: usize,
) -> std::io::Result<Vec<u8>>
where
    T: Read + Write + Seek,
{
    image.seek(SeekFrom::Start(0))?;
    let buf_stream = BufStream::new(image);
    let fs = FileSystem::new(buf_stream, FsOptions::new())?;
    let mut file = fs.root_dir().open_file(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{SeekFrom, Write};

    #[test]
    fn basic_file_ops() {
        let mut img = Cursor::new(vec![0u8; 1024 * 512]);
        fatfs::format_volume(&mut img, FormatVolumeOptions::new()).unwrap();
        img.seek(SeekFrom::Start(0)).unwrap();
        {
            let buf_stream = BufStream::new(&mut img);
            let fs = FileSystem::new(buf_stream, FsOptions::new()).unwrap();
            fs.root_dir().create_dir("testdir").unwrap();
            fs.root_dir()
                .create_file("foo.txt")
                .unwrap()
                .write_all(b"hello")
                .unwrap();
        }
        img.seek(SeekFrom::Start(0)).unwrap();
        let names = list_dir(&mut img, "/").unwrap();
        assert!(names.contains(&"testdir".to_string()));
        assert!(names.contains(&"foo.txt".to_string()));

        img.seek(SeekFrom::Start(0)).unwrap();
        assert!(file_exists(&mut img, "foo.txt").unwrap());
        img.seek(SeekFrom::Start(0)).unwrap();
        assert!(!file_exists(&mut img, "missing.txt").unwrap());

        img.seek(SeekFrom::Start(0)).unwrap();
        let data = read_file(&mut img, "foo.txt").unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn partial_read_seek() {
        let mut img = Cursor::new(vec![0u8; 1024 * 512]);
        fatfs::format_volume(&mut img, FormatVolumeOptions::new()).unwrap();
        img.seek(SeekFrom::Start(0)).unwrap();
        {
            let buf_stream = BufStream::new(&mut img);
            let fs = FileSystem::new(buf_stream, FsOptions::new()).unwrap();
            let mut file = fs.root_dir().create_file("foo.bin").unwrap();
            let data: Vec<u8> = (0u8..=255).collect();
            file.write_all(&data).unwrap();
        }

        img.seek(SeekFrom::Start(0)).unwrap();
        let slice = read_file_range(&mut img, "foo.bin", 10, 5).unwrap();
        assert_eq!(slice, vec![10, 11, 12, 13, 14]);

        img.seek(SeekFrom::Start(0)).unwrap();
        let slice = read_file_range(&mut img, "foo.bin", 250, 10).unwrap();
        assert_eq!(slice, vec![250, 251, 252, 253, 254, 255]);
    }
}
