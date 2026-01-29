//! Utility functions for expanded VPX operations

use crate::filesystem::FileSystem;
use serde::de;
use std::io;
use std::path::Path;

/// Sanitize a filename using the sanitize-filename crate
// TODO the whole sanitize_filename effort is not cross-platform compatible
//   Eg a vpx extracted on linux could fail to be opened on Windows if the sound name
//   contains such characters.
//   This should probably be improved in the future
pub(super) fn sanitize_filename<S: AsRef<str>>(name: S) -> String {
    sanitize_filename::sanitize(name)
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
    let mut json_file = fs.open_file(path)?;
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
}
