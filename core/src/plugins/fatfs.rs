use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::io::{Read, Seek, Write};
use fatfs::{FileSystem, FsOptions};
use fscommon::BufStream;

#[cfg(test)]
use std::io::{Cursor, SeekFrom};
#[cfg(test)]
use fatfs::FormatVolumeOptions;

/// List files in the root directory of a FAT image.
/// The image must be formatted before calling this function.
pub fn list_root<T>(image: T) -> std::io::Result<Vec<String>>
where
    T: Read + Write + Seek,
{
    let buf_stream = BufStream::new(image);
    let fs = FileSystem::new(buf_stream, FsOptions::new())?;
    let root_dir = fs.root_dir();
    let mut names = Vec::new();
    for r in root_dir.iter() {
        let entry = r?;
        names.push(entry.file_name().to_string());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn list_root_files() {
        let mut img = Cursor::new(vec![0u8; 1024 * 512]);
        fatfs::format_volume(&mut img, FormatVolumeOptions::new()).unwrap();
        img.seek(SeekFrom::Start(0)).unwrap();
        {
            let buf_stream = BufStream::new(&mut img);
            let fs = FileSystem::new(buf_stream, FsOptions::new()).unwrap();
            fs.root_dir().create_dir("testdir").unwrap();
            fs.root_dir().create_file("foo.txt").unwrap().write_all(b"hello").unwrap();
        }
        img.seek(SeekFrom::Start(0)).unwrap();
        let names = list_root(&mut img).unwrap();
        assert!(names.contains(&"testdir".to_string()));
        assert!(names.contains(&"foo.txt".to_string()));
    }
}
