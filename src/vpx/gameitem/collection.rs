use crate::vpx::biff::{self, BiffRead, BiffReader};
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Dummy)]
pub struct Collection {
    pub name: String,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: String,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct CollectionJson {
    name: String,
    is_locked: bool,
    editor_layer: u32,
    editor_layer_name: String,
    editor_layer_visibility: bool,
}

impl Serialize for Collection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CollectionJson {
            name: self.name.clone(),
            is_locked: self.is_locked,
            editor_layer: self.editor_layer,
            editor_layer_name: self.editor_layer_name.clone(),
            editor_layer_visibility: self.editor_layer_visibility,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Collection {
    fn deserialize<D>(deserializer: D) -> Result<Collection, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let json = CollectionJson::deserialize(deserializer)?;
        Ok(Collection {
            name: json.name,
            is_locked: json.is_locked,
            editor_layer: json.editor_layer,
            editor_layer_name: json.editor_layer_name,
            editor_layer_visibility: json.editor_layer_visibility,
        })
    }
}

impl BiffRead for Collection {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut name = Default::default();

        // these are shared between all items
        let mut is_locked: bool = false;
        let mut editor_layer: u32 = Default::default();
        let mut editor_layer_name: String = Default::default();
        let mut editor_layer_visibility: bool = true;

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
                // shared
                "LOCK" => {
                    is_locked = reader.get_bool();
                }
                "LAYR" => {
                    editor_layer = reader.get_u32();
                }
                "LANR" => {
                    editor_layer_name = reader.get_string();
                }
                "LVIS" => {
                    editor_layer_visibility = reader.get_bool();
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
        Self {
            name,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
        }
    }
}
