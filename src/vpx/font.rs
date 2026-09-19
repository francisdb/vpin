use super::biff::{self, BiffReader, BiffWriter};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;

// TODO comment here a vpx file that contains font data

/// A font file embedded in the table, mirroring vpinball's `PinFont`.
///
/// vpinball stores every font as its own `Font<n>` stream in the table
/// storage, written as a `PinBinary`: the records `NAME`, `PATH`, `SIZE`
/// and `DATA` followed by `ENDB` (`PinBinary::Save` and `PinBinary::Load`
/// in `src/parts/pinbinary.cpp`). On load vpinball writes the data to a
/// temporary `.ttf` file and registers it with Windows
/// (`PinFont::Register`, `AddFontResource`) so that textboxes and decals
/// can use the font by one of its [`face_names`](FontData::face_names).
#[derive(PartialEq)]
pub struct FontData {
    /// Name of the font binary (`PinBinary::m_name`): the file name,
    /// without extension, of the file it was imported from.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Path of the file the font was imported from (`PinBinary::m_path`),
    /// kept for re-importing; this library uses its extension when
    /// extracting the font.
    ///
    /// BIFF tag `PATH`
    pub path: String,
    /// The bytes of the font file, unchanged.
    ///
    /// Stored as a `SIZE` record holding the length followed by a `DATA`
    /// record holding the raw bytes; vpinball needs `SIZE` first to
    /// allocate the buffer.
    ///
    /// BIFF tags `SIZE` and `DATA`
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontDataJson {
    name: String,
    path: String,
}

impl FontDataJson {
    pub fn from_font_data(font_data: &FontData) -> Self {
        Self {
            name: font_data.name.clone(),
            path: font_data.path.clone(),
        }
    }
    pub fn to_font_data(&self) -> FontData {
        FontData {
            name: self.name.clone(),
            path: self.path.clone(),
            data: vec![],
        }
    }
}

impl fmt::Debug for FontData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("FontData")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("data", &format!("<{} bytes>", self.data.len()))
            .finish()
    }
}

impl FontData {
    pub(crate) fn ext(&self) -> String {
        // TODO we might want to also check the jpeg fsPath
        match self.path.split('.').next_back() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }
}

/// Read a font from the bytes of a `FontN` stream.
///
/// Fails with [`io::ErrorKind::InvalidData`] when the stream is truncated or
/// structurally invalid instead of panicking.
pub fn read(input: &[u8]) -> io::Result<FontData> {
    let mut reader = BiffReader::new(input);
    let mut name: String = "".to_string();
    let mut path: String = "".to_string();
    let mut size_opt: Option<u32> = None;
    let mut data: Vec<u8> = vec![];
    while let Some(tag) = reader.next(biff::WARN)? {
        let tag_str = tag.as_str();
        match tag_str {
            "NAME" => {
                name = reader.get_string()?;
            }
            "PATH" => {
                path = reader.get_string()?;
            }
            "SIZE" => {
                size_opt = Some((reader.get_u32()?).to_owned());
            }
            "DATA" => match size_opt {
                Some(size) => {
                    let d = reader.get_data(size as usize)?;
                    d.clone_into(&mut data);
                }
                None => return Err(reader.err("DATA tag without SIZE tag").into()),
            },
            _ => {
                warn!("Skipping font tag: {tag_str}");
                reader.skip_tag()?;
            }
        }
    }
    Ok(FontData { name, path, data })
}

/// Writes a font as the bytes of a `Font<n>` stream.
pub fn write(font_data: &FontData) -> Vec<u8> {
    let mut writer = BiffWriter::with_capacity(
        font_data.data.len() + font_data.name.len() + font_data.path.len() + 64,
    );
    writer.write_tagged_string("NAME", &font_data.name);
    writer.write_tagged_string("PATH", &font_data.path);
    writer.write_tagged_u32("SIZE", crate::vpx::biff::record_len(font_data.data.len()));
    writer.write_tagged_data("DATA", &font_data.data);
    writer.close(true);
    writer.into_data()
}

#[test]
fn read_write() {
    use pretty_assertions::assert_eq;

    let font = FontData {
        name: "test_name".to_string(),
        path: "/tmp/test".to_string(),
        data: vec![1, 2, 3, 4],
    };
    let bytes = write(&font);
    let font_read = read(&bytes).unwrap();

    assert_eq!(font, font_read);
}

impl FontData {
    /// The family and full names in the font file, which are the names a
    /// textbox or decal refers to. Empty when the data is not a TrueType or
    /// OpenType font.
    pub fn face_names(&self) -> Vec<String> {
        super::ttf::face_names(&self.data)
    }
}

#[cfg(test)]
mod corrupt_input_tests {
    use super::*;

    use crate::vpx::biff::BiffWriter;

    #[test]
    fn truncated_font_fails_without_panicking() {
        let font = FontData {
            name: "font".to_string(),
            path: "font.ttf".to_string(),
            data: vec![1, 2, 3, 4, 5, 6],
        };
        let bytes = write(&font);
        assert!(read(&bytes).is_ok());
        for len in 0..bytes.len() {
            assert!(read(&bytes[..len]).is_err(), "truncated to {len}");
        }
    }

    #[test]
    fn data_without_size_fails() {
        let mut writer = BiffWriter::new();
        writer.write_tagged_data("DATA", &[1, 2, 3]);
        writer.close(true);
        let err = read(writer.get_data()).unwrap_err();
        assert!(err.to_string().contains("SIZE"), "{err}");
    }
}
