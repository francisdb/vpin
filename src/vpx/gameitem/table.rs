use crate::vpx::biff::{self, BiffRead, BiffReader};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Dummy)]
pub struct Table {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
struct TableJson {
    name: String,
}

impl Serialize for Table {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json = TableJson {
            name: self.name.clone(),
        };
        json.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Table {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = TableJson::deserialize(deserializer)?;
        Ok(Self { name: json.name })
    }
}

impl BiffRead for Table {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut name = Default::default();

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
                    println!(
                        "Unknown tag {} for {}",
                        tag_str,
                        std::any::type_name::<Self>()
                    );
                    reader.skip_tag();
                }
            }
        }
        Self { name }
    }
}
