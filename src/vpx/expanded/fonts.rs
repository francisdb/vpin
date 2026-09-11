//! Font reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::font::{FontData, FontDataJson};
use log::info;
use std::io;
use std::path::Path;

use super::util::{read_json, sanitize_filename};
use super::{FONTS_DIR, Output, WriteError};

pub(super) fn write_fonts(fonts: &[FontData], out: &Output) -> Result<(), WriteError> {
    out.write_json(Path::new("fonts.json"), || {
        fonts
            .iter()
            .map(FontDataJson::from_font_data)
            .collect::<Vec<_>>()
    })?;

    fonts.iter().try_for_each(|font| {
        let sanitized_name = sanitize_filename(&font.name);
        let file_name = format!("{}.{}", sanitized_name, font.ext());
        out.write(&Path::new(FONTS_DIR).join(file_name), &font.data)
    })?;
    Ok(())
}

pub(super) fn read_fonts<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<FontData>> {
    let fonts_index_path = expanded_dir.as_ref().join("fonts.json");
    if !fs.exists(&fonts_index_path) {
        info!("No fonts.json found");
        return Ok(vec![]);
    }
    let fonts_json: Vec<FontDataJson> = read_json(fonts_index_path, fs)?;
    let fonts_index: Vec<FontData> = fonts_json
        .iter()
        .map(|font_data_json| font_data_json.to_font_data())
        .collect();
    let fonts_dir = expanded_dir.as_ref().join("fonts");
    let fonts: io::Result<Vec<FontData>> = fonts_index
        .into_iter()
        .map(|mut font| {
            let sanitized_name = sanitize_filename(&font.name);
            let file_name = format!("{}.{}", sanitized_name, font.ext());
            let font_path = fonts_dir.join(file_name);
            if fs.exists(&font_path) {
                let font_data = fs.read_file(&font_path)?;
                font.data = font_data;
                Ok(font)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Font file not found: {}", font_path.display()),
                ))
            }
        })
        .collect();
    fonts
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::filesystem::MemoryFileSystem;
    use crate::vpx::expanded::{ExpandOptions, Output};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_and_read_fonts() {
        let fs = MemoryFileSystem::new();
        let expanded_dir = Path::new("/expanded");

        let fonts = vec![
            FontData {
                name: "TestFont".to_string(),
                path: "c:\\test.ttf".to_string(),
                data: vec![0, 1, 2, 3],
            },
            FontData {
                name: "AnotherFont".to_string(),
                path: "c:\\AnotherFont.ttf".to_string(),
                data: vec![4, 5, 6, 7],
            },
        ];

        let options = ExpandOptions::default();
        let out = Output::new(expanded_dir, &fs, &options);
        write_fonts(&fonts, &out).unwrap();
        let read_fonts = read_fonts(&expanded_dir, &fs).unwrap();

        assert_eq!(fonts.len(), read_fonts.len());
        for (original, read) in fonts.iter().zip(read_fonts.iter()) {
            assert_eq!(original, read);
        }
    }
}
