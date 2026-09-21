use super::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite, BiffWriter};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;

/// A file embedded in the table unchanged, mirroring vpinball's `PinBinary`.
///
/// vpinball stores two things this way, as the records `NAME`, `PATH`,
/// `SIZE` and `DATA` followed by `ENDB` (`PinBinary::Save` and
/// `PinBinary::Load` in `src/parts/pinbinary.cpp`):
///
/// - The original file of an image, in [`ImageData::jpeg`]. It is written
///   as a nested record inside the image's `JPEG` record (`Texture::Save`
///   in `src/renderer/Texture.cpp`). Despite that tag it holds any format
///   vpinball can decode (JPEG, PNG, WEBP, EXR, HDR, ...).
/// - Every embedded font, in [`VPX::fonts`]. vpinball's `PinFont` is a
///   `PinBinary` subclass that is stored as its own `Font<n>` stream in the
///   table storage. On load vpinball writes the data to a temporary `.ttf`
///   file and registers it with Windows (`PinFont::Register`,
///   `AddFontResource`) so that textboxes and decals can use the font by
///   one of its [`face_names`](PinBinary::face_names).
///
/// [`ImageData::jpeg`]: crate::vpx::image::ImageData::jpeg
/// [`VPX::fonts`]: crate::vpx::VPX::fonts
#[derive(PartialEq, Clone)]
pub struct PinBinary {
    /// Name of the binary (`PinBinary::m_name`).
    ///
    /// For an image, in files written by vpinball, this is the same as
    /// [`ImageData::name`](crate::vpx::image::ImageData::name). For a font
    /// it is the file name, without extension, of the file it was imported
    /// from.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Lowercased copy of the name that old vpinball versions wrote inside
    /// the binary record of an image. Current vpinball neither reads nor
    /// writes it; this library keeps it so such files round-trip unchanged.
    /// `None` when the record is absent, which is the case for every
    /// current file.
    ///
    /// BIFF tag `INME`
    pub internal_name: Option<String>,
    /// Path of the file the binary was imported from
    /// (`PinBinary::m_path`), kept for re-importing.
    ///
    /// For an image, in files written by vpinball, this is the same as
    /// [`ImageData::path`](crate::vpx::image::ImageData::path). For a font
    /// this library uses its extension when extracting the font.
    ///
    /// BIFF tag `PATH`
    pub path: String,
    /// The bytes of the original file, unchanged, in whatever format it
    /// was imported in.
    ///
    /// Stored as a `SIZE` record holding the length followed by a `DATA`
    /// record holding the raw bytes; vpinball needs `SIZE` first to allocate
    /// the buffer.
    ///
    /// BIFF tags `SIZE` and `DATA`
    pub data: Vec<u8>,
}

impl fmt::Debug for PinBinary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("PinBinary")
            .field("name", &self.name)
            .field("internal_name", &self.internal_name)
            .field("path", &self.path)
            .field("data", &format!("<{} bytes>", self.data.len()))
            .finish()
    }
}

impl PinBinary {
    /// The part of [`PinBinary::path`] after its last dot, used as the file
    /// extension when a font is written out as a file.
    ///
    /// A path without a dot is returned whole.
    pub(crate) fn ext(&self) -> String {
        match self.path.split('.').next_back() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }

    /// The family and full names in the font file, which are the names a
    /// textbox or decal refers to. Empty when the data is not a TrueType or
    /// OpenType font, which is the case for the binary of an image.
    pub fn face_names(&self) -> Vec<String> {
        super::ttf::face_names(&self.data)
    }
}

/// The entry of a font in `fonts.json` of the expanded format; the data is
/// a file of its own.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PinBinaryJson {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    internal_name: Option<String>,
    path: String,
}

impl PinBinaryJson {
    pub fn from_pin_binary(binary: &PinBinary) -> Self {
        Self {
            name: binary.name.clone(),
            internal_name: binary.internal_name.clone(),
            path: binary.path.clone(),
        }
    }
    pub fn to_pin_binary(&self) -> PinBinary {
        PinBinary {
            name: self.name.clone(),
            internal_name: self.internal_name.clone(),
            path: self.path.clone(),
            data: vec![], // populated later
        }
    }
}

impl BiffRead for PinBinary {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut name: String = "".to_string();
        let mut internal_name: Option<String> = None;
        let mut path: String = "".to_string();
        let mut size_opt: Option<u32> = None;
        let mut data: Vec<u8> = vec![];
        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "NAME" => name = reader.get_string()?,
                "INME" => internal_name = Some(reader.get_string()?),
                "PATH" => path = reader.get_string()?,
                "SIZE" => size_opt = Some(reader.get_u32()?),
                "DATA" => match size_opt {
                    Some(size) => data = reader.get_data(size as usize)?.to_vec(),
                    None => return Err(reader.err("DATA tag without SIZE tag")),
                },
                _ => {
                    warn!("Skipping binary tag: {tag_str}");
                    reader.skip_tag()?;
                }
            }
        }
        Ok(PinBinary {
            name,
            internal_name,
            path,
            data,
        })
    }
}

impl BiffWrite for PinBinary {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_tagged_string("NAME", &self.name);
        if let Some(inme) = &self.internal_name {
            writer.write_tagged_string("INME", inme);
        }
        writer.write_tagged_string("PATH", &self.path);
        writer.write_tagged_u32("SIZE", biff::record_len(self.data.len()));
        writer.write_tagged_data("DATA", &self.data);
        writer.close(true);
    }
}

/// Read a binary from the bytes of its stream, for a font a `Font<n>`
/// stream.
///
/// Fails with [`io::ErrorKind::InvalidData`] when the stream is truncated or
/// structurally invalid instead of panicking.
pub fn read(input: &[u8]) -> io::Result<PinBinary> {
    let mut reader = BiffReader::new(input);
    Ok(PinBinary::biff_read(&mut reader)?)
}

/// Writes a binary as the bytes of its stream, for a font a `Font<n>`
/// stream.
pub fn write(binary: &PinBinary) -> Vec<u8> {
    let mut writer =
        BiffWriter::with_capacity(binary.data.len() + binary.name.len() + binary.path.len() + 64);
    binary.biff_write(&mut writer);
    writer.into_data()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::vpx::test_support::latin1_string;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    /// A binary as the stream can hold it: the strings are stored as
    /// Latin-1.
    fn any_pin_binary() -> impl Strategy<Value = PinBinary> {
        (
            latin1_string(),
            proptest::option::of(latin1_string()),
            latin1_string(),
            proptest::collection::vec(any::<u8>(), 0..64),
        )
            .prop_map(|(name, internal_name, path, data)| PinBinary {
                name,
                internal_name,
                path,
                data,
            })
    }

    proptest! {
        #[test]
        fn any_binary_round_trips_through_its_stream(binary in any_pin_binary()) {
            let bytes = write(&binary);
            let read = read(&bytes).unwrap();
            prop_assert_eq!(binary, read);
        }
    }

    #[test]
    fn read_write() {
        let font = PinBinary {
            name: "test_name".to_string(),
            internal_name: None,
            path: "/tmp/test".to_string(),
            data: vec![1, 2, 3, 4],
        };
        let bytes = write(&font);
        let font_read = read(&bytes).unwrap();

        assert_eq!(font, font_read);
    }

    #[test]
    fn read_write_with_internal_name() {
        let binary = PinBinary {
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            path: "path_value".to_string(),
            data: vec![1, 2, 3],
        };
        let bytes = write(&binary);
        let binary_read = read(&bytes).unwrap();

        assert_eq!(binary, binary_read);
    }

    #[test]
    fn records_are_written_in_vpinball_order() {
        let binary = PinBinary {
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            path: "path_value".to_string(),
            data: vec![1, 2, 3],
        };
        let bytes = write(&binary);
        let mut reader = BiffReader::new(&bytes);
        let mut tags = Vec::new();
        while let Some(tag) = reader.next(false).unwrap() {
            tags.push(tag.as_str().to_string());
            reader.skip_tag().unwrap();
        }
        assert_eq!(tags, ["NAME", "INME", "PATH", "SIZE", "DATA"]);
    }

    #[test]
    fn json_without_internal_name_reads_as_none() {
        let json: PinBinaryJson =
            serde_json::from_str(r#"{"name": "font", "path": "font.ttf"}"#).unwrap();
        assert_eq!(
            json.to_pin_binary(),
            PinBinary {
                name: "font".to_string(),
                internal_name: None,
                path: "font.ttf".to_string(),
                data: vec![],
            }
        );
        assert_eq!(
            serde_json::to_string(&json).unwrap(),
            r#"{"name":"font","path":"font.ttf"}"#
        );
    }
}

#[cfg(test)]
mod corrupt_input_tests {
    use super::*;

    #[test]
    fn truncated_binary_fails_without_panicking() {
        let font = PinBinary {
            name: "font".to_string(),
            internal_name: None,
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
