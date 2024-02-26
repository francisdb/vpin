use crate::vpx::biff::{self, BiffRead, BiffReader};
use serde::{Deserialize, Serialize};

use super::GameItem;

/**
 * FOr any items that have a type that we don't know about, we can use this
 */
#[derive(Debug, PartialEq)]
pub struct Generic {
    pub name: String,
    pub fields: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GenericJson {
    name: String,
    fields: Vec<(String, Vec<u8>)>,
}

impl Serialize for Generic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        GenericJson {
            name: self.name.clone(),
            fields: self.fields.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Generic {
    fn deserialize<D>(deserializer: D) -> Result<Generic, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let json = GenericJson::deserialize(deserializer)?;
        Ok(Generic {
            name: json.name,
            fields: json.fields,
        })
    }
}

impl GameItem for Generic {
    fn name(&self) -> &str {
        &self.name
    }
}

impl BiffRead for Generic {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut name = Default::default();
        let mut fields: Vec<(String, Vec<u8>)> = Vec::new();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "NAME" => {
                    name = reader.get_wide_string();
                }
                _ => {
                    fields.push((tag_str.to_string(), reader.get_record_data(false).to_vec()));
                }
            }
        }
        Self { name, fields }
    }
}
