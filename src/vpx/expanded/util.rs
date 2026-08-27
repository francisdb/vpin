//! Utility functions for expanded VPX operations

use crate::filesystem::FileSystem;
use serde::de;
use std::borrow::Cow;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Sanitize a filename using the sanitize-filename crate
///
/// The name is first NFC-normalized so that generated file names are
/// deterministic regardless of the Unicode form used inside the VPX.
// TODO the whole sanitize_filename effort is not cross-platform compatible
//   Eg a vpx extracted on linux could fail to be opened on Windows if the sound name
//   contains such characters.
//   This should probably be improved in the future
pub(crate) fn sanitize_filename<S: AsRef<str>>(name: S) -> String {
    let normalized: String = name.as_ref().nfc().collect();
    sanitize_filename::sanitize(normalized)
}

/// A [`FileSystem`] wrapper that retries path lookups with NFC/NFD-normalized
/// variants when the exact path does not exist.
///
/// Storage layers like Safari/WebKit OPFS, macOS HFS+ and some network shares
/// change the Unicode normalization form of file names between writing and
/// listing. A name recorded in an index json (e.g. `gameitems.json`) then no
/// longer matches the stored file byte-wise even though both render
/// identically ("ö" as U+00F6 vs "o" + U+0308).
/// See <https://github.com/francisdb/vpin/issues/355>
pub(crate) struct NormalizingFileSystem<'a> {
    inner: &'a dyn FileSystem,
}

impl<'a> NormalizingFileSystem<'a> {
    pub(crate) fn new(inner: &'a dyn FileSystem) -> Self {
        Self { inner }
    }

    fn resolve<'p>(&self, path: &'p Path) -> Cow<'p, Path> {
        if self.inner.exists(path) {
            return Cow::Borrowed(path);
        }
        let Some(path_str) = path.to_str() else {
            return Cow::Borrowed(path);
        };
        if path_str.is_ascii() {
            return Cow::Borrowed(path);
        }
        let nfc: String = path_str.nfc().collect();
        if nfc != path_str {
            let candidate = PathBuf::from(nfc);
            if self.inner.exists(&candidate) {
                return Cow::Owned(candidate);
            }
        }
        let nfd: String = path_str.nfd().collect();
        if nfd != path_str {
            let candidate = PathBuf::from(nfd);
            if self.inner.exists(&candidate) {
                return Cow::Owned(candidate);
            }
        }
        Cow::Borrowed(path)
    }
}

impl FileSystem for NormalizingFileSystem<'_> {
    fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        self.inner.create_file(path)
    }

    fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        self.inner.open_file(&self.resolve(path))
    }

    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.inner.read_file(&self.resolve(path))
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        self.inner.write_file(path, data)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        self.inner.exists(&self.resolve(path))
    }

    fn create_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
        self.inner.create_buffered_file(path)
    }

    fn open_buffered_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
        self.inner.open_buffered_file(&self.resolve(path))
    }
}

/// Read and parse a JSON file from the filesystem
pub(super) fn read_json<P: AsRef<Path>, T>(json_path: P, fs: &dyn FileSystem) -> io::Result<T>
where
    T: de::DeserializeOwned,
{
    let path = json_path.as_ref();
    if !fs.exists(path) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("JSON file not found: {}", path.display()),
        ));
    }
    let mut json_file = fs.open_buffered_file(path)?;
    serde_json::from_reader(&mut json_file).map_err(|e| {
        io::Error::other(format!(
            "Failed to parse/read json {}: {}",
            path.display(),
            e
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_sanitize_filename() {
        let name = "font:name/with*invalid|chars?.ttf";
        let sanitized = sanitize_filename(name);
        assert_eq!(sanitized, "fontnamewithinvalidchars.ttf");
    }

    #[test]
    fn test_sanitize_filename_normalizes_to_nfc() {
        // "PfLöcher" with NFD "ö" ("o" + combining diaeresis U+0308)
        let nfd = "PfLo\u{0308}cher";
        // NFC "ö" (U+00F6)
        assert_eq!(sanitize_filename(nfd), "PfL\u{00F6}cher");
    }

    #[test]
    fn test_normalizing_filesystem_resolves_nfd_stored_file() {
        use crate::filesystem::MemoryFileSystem;
        let fs = MemoryFileSystem::new();
        // file stored under its NFD form, as a normalizing storage layer would list it
        let nfd_path = Path::new("gameitems/Primitive.PfLo\u{0308}cher.json");
        fs.write_file(nfd_path, b"{}").unwrap();

        let normalizing = NormalizingFileSystem::new(&fs);
        // lookup with the NFC form recorded in the index
        let nfc_path = Path::new("gameitems/Primitive.PfL\u{00F6}cher.json");
        assert!(normalizing.exists(nfc_path));
        assert_eq!(normalizing.read_file(nfc_path).unwrap(), b"{}");
    }

    #[test]
    fn test_normalizing_filesystem_resolves_nfc_stored_file() {
        use crate::filesystem::MemoryFileSystem;
        let fs = MemoryFileSystem::new();
        let nfc_path = Path::new("gameitems/Primitive.PfL\u{00F6}cher.json");
        fs.write_file(nfc_path, b"{}").unwrap();

        let normalizing = NormalizingFileSystem::new(&fs);
        let nfd_path = Path::new("gameitems/Primitive.PfLo\u{0308}cher.json");
        assert!(normalizing.exists(nfd_path));
        assert_eq!(normalizing.read_file(nfd_path).unwrap(), b"{}");
    }

    #[test]
    fn test_normalizing_filesystem_missing_file() {
        use crate::filesystem::MemoryFileSystem;
        let fs = MemoryFileSystem::new();
        let normalizing = NormalizingFileSystem::new(&fs);
        assert!(!normalizing.exists(Path::new("gameitems/Primitive.PfL\u{00F6}cher.json")));
        assert!(!normalizing.exists(Path::new("gameitems/Primitive.Missing.json")));
    }
}
