use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn find_files<P: AsRef<Path>>(tables_path: P, extension: &str) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut entries = WalkDir::new(tables_path).into_iter();
    let os_extension = OsStr::new(extension);
    entries.try_for_each(|entry| {
        let dir_entry = entry?;
        let path = dir_entry.path();
        if path.is_file() {
            match path.extension() {
                Some(ex) if ex == os_extension => found.push(path.to_path_buf()),
                _ => {}
            }
        }
        Ok::<(), io::Error>(())
    })?;
    Ok(found)
}
