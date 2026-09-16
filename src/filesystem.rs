use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Where the expanded directory format reads and writes its files.
///
/// The expanded reader and writer take the file system as a parameter so
/// the same code serves a directory on disk ([`RealFileSystem`]) and an
/// in-memory tree ([`MemoryFileSystem`]), which the wasm bindings and the
/// tests use. Paths are whatever the caller passes; the trait does not
/// resolve or normalize them.
pub trait FileSystem: Sync {
    /// Creates the file at `path`, replacing any existing content, and
    /// returns a writer for it. The written data must be visible to
    /// [`read_file`](Self::read_file) once the writer is flushed or
    /// dropped.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be created, for example when its parent
    /// directory does not exist.
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>>;

    /// Opens the file at `path` for reading.
    ///
    /// # Errors
    ///
    /// Fails with [`io::ErrorKind::NotFound`] when there is no file at
    /// `path`; the message names the path.
    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>>;

    /// Reads the whole file at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`io::ErrorKind::NotFound`] when there is no file at
    /// `path`; the message names the path.
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// Writes `data` as the whole content of the file at `path`, creating
    /// the file or replacing what it held.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be created or written.
    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()>;

    /// Creates the directory at `path` and every missing parent. Succeeds
    /// when the directory already exists.
    ///
    /// # Errors
    ///
    /// Fails when a directory cannot be created.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Whether something exists at `path`. For an implementation without
    /// directories this is whether a file was written at that exact path.
    fn exists(&self, path: &Path) -> bool;

    /// Writer suitable for many small writes. Default impl wraps
    /// [`create_file`](Self::create_file) in a [`BufWriter`]. Implementations whose
    /// writer already buffers (e.g. an in-memory [`Vec<u8>`]-backed one) can
    /// override to skip the extra layer.
    ///
    /// Callers should still invoke `flush()?` before dropping the writer:
    /// errors from a `Drop`-time flush are otherwise swallowed.
    fn create_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        Ok(Box::new(BufWriter::new(self.create_file(path)?)))
    }

    /// Reader suitable for many small reads. Default impl wraps
    /// [`open_file`](Self::open_file) in a [`BufReader`]. Implementations whose
    /// reader already serves bytes from memory (e.g. a [`Cursor`] over a
    /// [`Vec<u8>`]) can override to skip the extra layer.
    fn open_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        Ok(Box::new(BufReader::new(self.open_file(path)?)))
    }
}

/// The file system of the host, through [`std::fs`].
///
/// Paths are used as given, relative ones against the current working
/// directory. Errors from opening and reading name the path, which
/// [`std::fs`] leaves out.
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        let file = File::create(path)?;
        Ok(Box::new(file))
    }

    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        let file = File::open(path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        Ok(Box::new(file))
    }

    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
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

/// A file system that keeps every file in memory, keyed by the path as a
/// string.
///
/// There are no directories: [`create_dir_all`](FileSystem::create_dir_all)
/// always succeeds and [`exists`](FileSystem::exists) only reports files.
/// A path is stored as its lossy string form, so `/vpx/a.png` and
/// `Path::new("/vpx/a.png")` name the same file.
///
/// Clones share the same files; the store is behind an [`Arc`] and a
/// [`RwLock`], so a clone handed to another thread sees the same writes.
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
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

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
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    /// An empty file system.
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// A copy of the content of the file at `path`, `None` when there is
    /// no such file.
    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.get(path).cloned()
    }

    /// The paths of every file, in no particular order.
    pub fn list_files(&self) -> Vec<String> {
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.keys().cloned().collect()
    }

    /// Removes every file.
    pub fn clear(&self) {
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.clear();
    }

    /// Removes the file at `path`; a path without a file is not an error.
    pub fn delete_file(&self, path: &str) {
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.insert(self.path.clone(), self.buffer.clone());
        Ok(())
    }
}

impl Drop for MemoryFileWriter {
    fn drop(&mut self) {
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.get(&path_str).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path_str),
            )
        })
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let mut files = self
            .files
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.insert(path_str, data.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_string();
        let files = self
            .files
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        files.contains_key(&path_str)
    }

    // The in-memory writer is already Vec<u8>-backed and the reader is a Cursor
    // over a Vec<u8>; both serve bytes without syscalls, so an extra BufWriter
    // / BufReader layer would be pure overhead.
    fn create_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        self.create_file(path)
    }

    fn open_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        self.open_file(path)
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
