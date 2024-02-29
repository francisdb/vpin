use serde::{Deserialize, Serialize};

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

impl Serialize for StringWithEncoding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.string.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StringWithEncoding {
    fn deserialize<D>(deserializer: D) -> Result<StringWithEncoding, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        Ok(StringWithEncoding {
            encoding: StringEncoding::Utf8,
            string,
        })
    }
}
