use encoding_rs::mem::decode_latin1;
use log::warn;
use std::borrow::Cow;

/// A mesh vertex with position, normal and one set of texture
/// coordinates, mirroring vpinball's `Vertex3D_NoTex2` (`src/core/def.h`).
///
/// This is the vertex layout of a primitive mesh: vpinball writes the
/// struct as is, eight little endian `f32` in field order (32 bytes per
/// vertex), into the `M3CX` (compressed) or `M3DX` (older, uncompressed)
/// record. The mesh readers, the OBJ and glTF exporters and the wasm
/// build all exchange vertices in this form; see
/// [`VertexWrapper`](crate::vpx::gameitem::primitive::VertexWrapper) for
/// the byte-exact wrapper used when reading.
#[derive(Debug, PartialEq, Clone)]
pub struct Vertex3dNoTex2 {
    /// X coordinate of the position.
    pub x: f32,
    /// Y coordinate of the position.
    pub y: f32,
    /// Z coordinate of the position.
    pub z: f32,
    /// X component of the vertex normal.
    pub nx: f32,
    /// Y component of the vertex normal.
    pub ny: f32,
    /// Z component of the vertex normal.
    pub nz: f32,
    /// Horizontal texture coordinate (u), 0 to 1 across the texture.
    pub tu: f32,
    /// Vertical texture coordinate (v), 0 to 1 down the texture.
    pub tv: f32,
}

impl Vertex3dNoTex2 {
    pub(crate) fn to_vpx_bytes(&self) -> [u8; 32] {
        let mut b = [0u8; 32];
        let mut offset = 0;
        for &value in &[
            self.x, self.y, self.z, self.nx, self.ny, self.nz, self.tu, self.tv,
        ] {
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            offset += 4;
        }
        b
    }

    #[cfg(test)]
    pub(crate) fn as_vpx_bytes(&self) -> [u8; 32] {
        self.to_vpx_bytes()
    }

    #[cfg(test)]
    pub(crate) fn from_vpx_bytes(b: &[u8; 32]) -> Self {
        Vertex3dNoTex2 {
            x: f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            y: f32::from_le_bytes([b[4], b[5], b[6], b[7]]),
            z: f32::from_le_bytes([b[8], b[9], b[10], b[11]]),
            nx: f32::from_le_bytes([b[12], b[13], b[14], b[15]]),
            ny: f32::from_le_bytes([b[16], b[17], b[18], b[19]]),
            nz: f32::from_le_bytes([b[20], b[21], b[22], b[23]]),
            tu: f32::from_le_bytes([b[24], b[25], b[26], b[27]]),
            tv: f32::from_le_bytes([b[28], b[29], b[30], b[31]]),
        }
    }
}

/// The byte encoding a string had in the file, kept so that it can be
/// written back with the same bytes. See [`StringWithEncoding`].
#[derive(Debug, PartialEq, Clone)]
pub enum StringEncoding {
    /// One byte per character (ISO-8859-1), as older vpinball versions
    /// wrote strings. Picked when the bytes in the file are not valid
    /// UTF-8. Characters outside Latin-1 cannot be stored and are written
    /// as `?`.
    Latin1,
    /// UTF-8, as current vpinball writes strings. Picked when the bytes in
    /// the file are valid UTF-8, and the encoding of every string created
    /// from Rust.
    Utf8,
}

/// A string together with the encoding it had in the file.
///
/// vpinball wrote strings as Latin-1 before it switched to UTF-8, and a
/// table must round trip byte for byte, so a string read from a file
/// remembers which of the two it was and is written back the same way.
/// Used for the table script ([`GameData::code`]) and by the BIFF reader
/// for any string whose encoding must be preserved.
///
/// [`GameData::code`]: crate::vpx::gamedata::GameData::code
#[derive(Debug, PartialEq, Clone)]
pub struct StringWithEncoding {
    /// How the bytes were, or will be, stored in the file:
    /// [`StringEncoding::Latin1`] when the bytes read were not valid UTF-8,
    /// [`StringEncoding::Utf8`] otherwise and for strings created from
    /// Rust.
    pub encoding: StringEncoding,
    /// The decoded text.
    pub string: String,
}
impl StringWithEncoding {
    /// A UTF-8 string from anything that converts into a `String`.
    pub fn new(string: impl Into<String>) -> StringWithEncoding {
        StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string: string.into(),
        }
    }

    /// A UTF-8 string copied from `s`.
    pub fn from(s: &str) -> StringWithEncoding {
        StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string: s.to_owned(),
        }
    }

    /// An empty UTF-8 string, the script of a new table.
    pub fn empty() -> StringWithEncoding {
        StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string: String::new(),
        }
    }
}

impl From<&[u8]> for StringWithEncoding {
    fn from(data: &[u8]) -> Self {
        match String::from_utf8(data.to_vec()) {
            Ok(s) => StringWithEncoding {
                encoding: StringEncoding::Utf8,
                string: s.to_string(),
            },
            Err(_e) => StringWithEncoding {
                encoding: StringEncoding::Latin1,
                string: decode_latin1(data).to_string(),
            },
        }
    }
}

impl From<Vec<u8>> for StringWithEncoding {
    fn from(data: Vec<u8>) -> Self {
        // TODO how to avoid clone here?
        match String::from_utf8(data.clone()) {
            Ok(s) => StringWithEncoding {
                encoding: StringEncoding::Utf8,
                string: s.to_string(),
            },
            Err(_e) => StringWithEncoding {
                encoding: StringEncoding::Latin1,
                string: decode_latin1(data.as_ref()).to_string(),
            },
        }
    }
}

impl From<StringWithEncoding> for Vec<u8> {
    fn from(string_with_encoding: StringWithEncoding) -> Self {
        match string_with_encoding.encoding {
            StringEncoding::Utf8 => string_with_encoding.string.as_bytes().to_vec(),
            StringEncoding::Latin1 => encode_latin1_lossy(&string_with_encoding.string).to_vec(),
        }
    }
}

/// Encodes a string as Latin-1, one byte per character, with `?` for
/// every character Latin-1 cannot hold. Borrows when the string is ASCII.
///
/// The `encoding_rs` function of the same name is not lossy: it requires
/// Latin-1 input and otherwise panics under debug assertions or produces
/// unspecified bytes, so a table name or script with a character outside
/// Latin-1 needs this one.
pub(crate) fn encode_latin1_lossy(string: &str) -> Cow<'_, [u8]> {
    if string.is_ascii() {
        return Cow::Borrowed(string.as_bytes());
    }
    let mut replaced = 0usize;
    let bytes: Vec<u8> = string
        .chars()
        .map(|c| match u32::from(c) {
            code @ 0..=0xFF => code as u8,
            _ => {
                replaced += 1;
                b'?'
            }
        })
        .collect();
    if replaced > 0 {
        warn!("{replaced} character(s) of {string:?} cannot be stored as Latin-1, written as '?'");
    }
    Cow::Owned(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn latin1_encoding_is_lossy_and_borrows_ascii() {
        use super::encode_latin1_lossy;
        use std::borrow::Cow;
        assert!(matches!(
            encode_latin1_lossy("Metal"),
            Cow::Borrowed(b"Metal")
        ));
        assert_eq!(encode_latin1_lossy("Métal").as_ref(), b"M\xe9tal");
        assert_eq!(encode_latin1_lossy("Metal ✓ ok").as_ref(), b"Metal ? ok");
    }

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_string_with_encoding_latin1() {
        // a latin1 string that is not utf8 compatible
        let bytes = vec![0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89];
        let s: StringWithEncoding = bytes.clone().into();
        assert_eq!(s.encoding, StringEncoding::Latin1);
        println!("{:?}", s.string);

        let bytes_decoded: Vec<u8> = s.into();
        assert_eq!(bytes, bytes_decoded);
    }

    #[test]
    fn test_string_with_encoding_utf8() {
        // a latin1 string that is not utf8 compatible
        let bytes = "Hello World".as_bytes().to_vec();
        let s: StringWithEncoding = bytes.clone().into();
        assert_eq!(s.encoding, StringEncoding::Utf8);

        let bytes_decoded: Vec<u8> = s.into();
        assert_eq!(bytes, bytes_decoded);
    }

    #[test]
    fn test_vertex3d_no_tex2_serialization() {
        let vertex = Vertex3dNoTex2 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            nx: 0.0,
            ny: 1.0,
            nz: 0.0,
            tu: 0.5,
            tv: 0.5,
        };
        let bytes = vertex.as_vpx_bytes();
        let deserialized_vertex = Vertex3dNoTex2::from_vpx_bytes(&bytes);
        assert_eq!(vertex, deserialized_vertex);
    }
}
