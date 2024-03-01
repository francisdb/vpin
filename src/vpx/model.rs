use encoding_rs::mem::{decode_latin1, encode_latin1_lossy};

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
}
