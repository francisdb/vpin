use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite, BiffWriter};
use serde::{Deserialize, Serialize};

use super::GameItem;

/// Game item of a type this library does not know about.
///
/// All records are kept as raw bytes in their original order so the item can
/// be written back unchanged. The `NAME` record is decoded into `name`; its
/// entry in `fields` keeps the position but carries no data.
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
                    fields.push((tag_str.to_string(), Vec::new()));
                }
                _ => {
                    fields.push((tag_str.to_string(), reader.get_record_data(false).to_vec()));
                }
            }
        }
        Self { name, fields }
    }
}

impl BiffWrite for Generic {
    fn biff_write(&self, writer: &mut BiffWriter) {
        let has_name_field = self.fields.iter().any(|(tag, _)| tag == "NAME");
        if !has_name_field && !self.name.is_empty() {
            writer.write_tagged_wide_string("NAME", &self.name);
        }
        for (tag, data) in &self.fields {
            if tag == "NAME" {
                writer.write_tagged_wide_string("NAME", &self.name);
            } else {
                writer.write_tagged_data(tag, data);
            }
        }
        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_read_write_preserves_record_order() {
        let mut writer = BiffWriter::new();
        writer.write_tagged_f32("ABCD", 1.5);
        writer.write_tagged_wide_string("NAME", "unknown item");
        writer.write_tagged_data("EFGH", &[1, 2, 3]);
        writer.close(true);
        let bytes = writer.get_data().to_vec();

        let generic = Generic::biff_read(&mut BiffReader::new(&bytes));
        assert_eq!(generic.name, "unknown item");
        assert_eq!(
            generic.fields,
            vec![
                ("ABCD".to_string(), 1.5f32.to_le_bytes().to_vec()),
                ("NAME".to_string(), vec![]),
                ("EFGH".to_string(), vec![1, 2, 3]),
            ]
        );

        let mut writer = BiffWriter::new();
        generic.biff_write(&mut writer);
        assert_eq!(writer.get_data(), &bytes[..]);
    }

    #[test]
    fn test_write_read_without_name_record() {
        let generic = Generic {
            name: "renamed".to_string(),
            fields: vec![("ABCD".to_string(), vec![9, 8, 7, 6])],
        };
        let mut writer = BiffWriter::new();
        generic.biff_write(&mut writer);
        let read = Generic::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(read.name, "renamed");
        assert_eq!(
            read.fields,
            vec![
                ("NAME".to_string(), vec![]),
                ("ABCD".to_string(), vec![9, 8, 7, 6]),
            ]
        );
    }
}
