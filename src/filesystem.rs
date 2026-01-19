use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};

pub trait FileSystem: Sync {
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>>;
    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>>;
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
}

pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        let file = File::create(path)?;
        Ok(Box::new(file))
    }

    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        let file = File::open(path)?;
        Ok(Box::new(file))
    }

    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        std::fs::write(path, data)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

#[derive(Default, Clone)]
pub struct MemoryFileSystem {
    files: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryFileSystem {
    /// Renames a file from `from` to `to`.
    /// This is for now only available in test builds.
    #[cfg(test)]
    pub(crate) fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let from_str = from.to_string_lossy().to_string();
        let to_str = to.to_string_lossy().to_string();
        let mut files = self.files.write().unwrap();

        // Clone the data instead of removing it first
        if let Some(data) = files.get(&from_str).cloned() {
            files.insert(to_str, data);
            files.remove(&from_str);
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", from_str),
            ))
        }
    }

    /// Reads the entire file at `path` as a UTF-8 string.
    ///
    /// Throws an error if the file does not exist or if the content is not valid UTF-8.
    ///
    /// This is for now only available in test builds.
    #[cfg(test)]
    pub(crate) fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let path_str = path.to_string_lossy().to_string();
        let files = self.files.read().unwrap();
        match files.get(&path_str) {
            Some(data) => String::from_utf8(data.clone()).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid UTF-8 data in file {}: {}", path_str, e),
                )
            }),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path_str),
            )),
        }
    }
}

impl MemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        let files = self.files.read().unwrap();
        files.get(path).cloned()
    }

    pub fn list_files(&self) -> Vec<String> {
        let files = self.files.read().unwrap();
        files.keys().cloned().collect()
    }

    pub fn clear(&self) {
        let mut files = self.files.write().unwrap();
        files.clear();
    }

    pub fn delete_file(&self, path: &str) {
        let mut files = self.files.write().unwrap();
        files.remove(path);
    }
}

struct MemoryFileWriter {
    path: String,
    buffer: Vec<u8>,
    files: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl Write for MemoryFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut files = self.files.write().unwrap();
        files.insert(self.path.clone(), self.buffer.clone());
        Ok(())
    }
}

impl Drop for MemoryFileWriter {
    fn drop(&mut self) {
        let mut files = self.files.write().unwrap();
        files.insert(self.path.clone(), std::mem::take(&mut self.buffer));
    }
}

impl FileSystem for MemoryFileSystem {
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        let path_str = path.to_string_lossy().to_string();
        Ok(Box::new(MemoryFileWriter {
            path: path_str,
            buffer: Vec::new(),
            files: Arc::clone(&self.files),
        }))
    }

    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        let path_str = path.to_string_lossy().to_string();
        let files = self.files.read().unwrap();
        match files.get(&path_str) {
            Some(data) => Ok(Box::new(Cursor::new(data.clone()))),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path_str),
            )),
        }
    }

    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        let path_str = path.to_string_lossy().to_string();
        let files = self.files.read().unwrap();
        files.get(&path_str).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path_str),
            )
        })
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let mut files = self.files.write().unwrap();
        files.insert(path_str, data.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_string();
        let files = self.files.read().unwrap();
        files.contains_key(&path_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_family = "wasm"))]
    use testdir::testdir;

    #[test]
    fn test_memory_fs_write_read() {
        let fs = MemoryFileSystem::new();
        let path = Path::new("/test/file.txt");

        fs.write_file(path, b"hello world").unwrap();

        assert!(fs.exists(path));
        let data = fs.read_file(path).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_memory_fs_create_file() {
        let fs = MemoryFileSystem::new();
        let path = Path::new("/test/file.txt");

        {
            let mut writer = fs.create_file(path).unwrap();
            writer.write_all(b"hello").unwrap();
        }

        assert!(fs.exists(path));
        let data = fs.read_file(path).unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_real_fs_write_read() {
        let test_dir = testdir!();
        let fs = RealFileSystem;
        let path = test_dir.join("file.txt");

        fs.write_file(&path, b"hello world").unwrap();
        assert!(fs.exists(&path));
        let data = fs.read_file(&path).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_real_fs_create_file() {
        let test_dir = testdir!();
        let fs = RealFileSystem;
        let path = test_dir.join("file2.txt");

        {
            let mut writer = fs.create_file(&path).unwrap();
            writer.write_all(b"hello").unwrap();
        }

        assert!(fs.exists(&path));
        let data = fs.read_file(&path).unwrap();
        assert_eq!(data, b"hello");
    }
}
