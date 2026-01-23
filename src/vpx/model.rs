use encoding_rs::mem::{decode_latin1, encode_latin1_lossy};

/// The enum used inside vpinball to represent a vertex in the vpx format (Vertex3D_NoTex2).
/// <https://github.com/vpinball/vpinball/blob/9bb99ca92ff7e7eb37c9fb42dd4dcc206b814132/def.h#L165C7-L181>
///
/// This struct is used for serializing and deserializing in the vpinball C++ code
#[derive(Debug, PartialEq, Clone)]
pub struct Vertex3dNoTex2 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub tu: f32,
    pub tv: f32,
}

impl Vertex3dNoTex2 {
    #[cfg(test)]
    pub(crate) fn as_vpx_bytes(&self) -> [u8; 32] {
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

#[derive(Debug, PartialEq, Clone)]
pub enum StringEncoding {
    Latin1,
    Utf8,
}

/// Because we want to have a exact copy after reading/writing a vpx file we need to
/// keep old latin1 encoding if we read that from a file.
#[derive(Debug, PartialEq, Clone)]
pub struct StringWithEncoding {
    pub encoding: StringEncoding,
    pub string: String,
}
impl StringWithEncoding {
    pub fn new(string: String) -> StringWithEncoding {
        StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string,
        }
    }

    pub fn from(s: &str) -> StringWithEncoding {
        StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string: s.to_owned(),
        }
    }

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

#[cfg(test)]
mod tests {
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
