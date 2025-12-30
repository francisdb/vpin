use super::{GameItem, font::Font, font::FontJson, vertex2d::Vertex2D};
use crate::vpx::gameitem::select::{HasSharedAttributes, WriteSharedAttributes};
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, PartialEq, Dummy, Clone)]
pub enum DecalType {
    Text = 0,
    Image = 1,
}

#[derive(Debug, PartialEq, Dummy, Clone)]
pub enum SizingType {
    AutoSize = 0,
    AutoWidth = 1,
    ManualSize = 2,
}

impl From<&DecalType> for u32 {
    fn from(decal_type: &DecalType) -> u32 {
        match decal_type {
            DecalType::Text => 0,
            DecalType::Image => 1,
        }
    }
}

impl From<u32> for DecalType {
    fn from(value: u32) -> Self {
        match value {
            0 => DecalType::Text,
            1 => DecalType::Image,
            _ => panic!("Invalid value for DecalType: {value}, we expect 0, 1"),
        }
    }
}

/// A serializer for DecalType that writes it as lowercase
impl Serialize for DecalType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            DecalType::Text => "text",
            DecalType::Image => "image",
        };
        serializer.serialize_str(value)
    }
}

/// A deserializer for DecalType that reads it as lowercase
/// or number for backwards compatibility.
impl<'de> Deserialize<'de> for DecalType {
    fn deserialize<D>(deserializer: D) -> Result<DecalType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer);
        match value {
            Ok(Value::String(value)) => match value.as_str() {
                "text" => Ok(DecalType::Text),
                "image" => Ok(DecalType::Image),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid value for DecalType: {value}, we expect \"text\", \"image\""
                ))),
            },
            Ok(Value::Number(value)) => {
                let value = value.as_u64().unwrap();
                match value {
                    0 => Ok(DecalType::Text),
                    1 => Ok(DecalType::Image),
                    _ => Err(serde::de::Error::custom(format!(
                        "Invalid value for DecalType: {value}, we expect 0, 1"
                    ))),
                }
            }
            _ => Err(serde::de::Error::custom(format!(
                "Invalid value for DecalType: {value:?}, we expect a string or a number"
            ))),
        }
    }
}

impl From<&SizingType> for u32 {
    fn from(sizing_type: &SizingType) -> u32 {
        match sizing_type {
            SizingType::AutoSize => 0,
            SizingType::AutoWidth => 1,
            SizingType::ManualSize => 2,
        }
    }
}

impl From<u32> for SizingType {
    fn from(value: u32) -> Self {
        match value {
            0 => SizingType::AutoSize,
            1 => SizingType::AutoWidth,
            2 => SizingType::ManualSize,
            _ => panic!("Invalid value for SizingType: {value}, we expect 0, 1, 2"),
        }
    }
}

/// A serializer for SizingType that writes it as lowercase
impl Serialize for SizingType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            SizingType::AutoSize => "auto_size",
            SizingType::AutoWidth => "auto_width",
            SizingType::ManualSize => "manual_size",
        };
        serializer.serialize_str(value)
    }
}

/// A deserializer for SizingType that reads it as lowercase
/// or number for backwards compatibility.
impl<'de> Deserialize<'de> for SizingType {
    fn deserialize<D>(deserializer: D) -> Result<SizingType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer);
        match value {
            Ok(Value::String(value)) => match value.as_str() {
                "auto_size" => Ok(SizingType::AutoSize),
                "auto_width" => Ok(SizingType::AutoWidth),
                "manual_size" => Ok(SizingType::ManualSize),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid value for SizingType: {value}, we expect \"auto_size\", \"auto_width\", \"manual_size\""
                ))),
            },
            Ok(Value::Number(value)) => {
                let value = value.as_u64().unwrap();
                match value {
                    0 => Ok(SizingType::AutoSize),
                    1 => Ok(SizingType::AutoWidth),
                    2 => Ok(SizingType::ManualSize),
                    _ => Err(serde::de::Error::custom(format!(
                        "Invalid value for SizingType: {value}, we expect 0, 1, 2"
                    ))),
                }
            }
            _ => Err(serde::de::Error::custom(format!(
                "Invalid value for SizingType: {value:?}, we expect a string or a number"
            ))),
        }
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Decal {
    pub center: Vertex2D,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub image: String,
    pub surface: String,
    pub name: String,
    pub text: String,
    pub decal_type: DecalType,
    pub material: String,
    pub color: Color,
    pub sizing_type: SizingType,
    pub vertical_text: bool,
    pub backglass: bool,

    pub font: Font,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DecalJson {
    center: Vertex2D,
    width: f32,
    height: f32,
    rotation: f32,
    image: String,
    surface: String,
    name: String,
    text: String,
    decal_type: DecalType,
    material: String,
    color: Color,
    sizing_type: SizingType,
    vertical_text: bool,
    backglass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
    font: FontJson,
}

impl DecalJson {
    pub fn from_decal(decal: &Decal) -> Self {
        Self {
            center: decal.center,
            width: decal.width,
            height: decal.height,
            rotation: decal.rotation,
            image: decal.image.clone(),
            surface: decal.surface.clone(),
            name: decal.name.clone(),
            text: decal.text.clone(),
            decal_type: decal.decal_type.clone(),
            material: decal.material.clone(),
            color: decal.color,
            sizing_type: decal.sizing_type.clone(),
            vertical_text: decal.vertical_text,
            backglass: decal.backglass,
            part_group_name: decal.part_group_name.clone(),
            font: FontJson::from_font(&decal.font),
        }
    }

    pub fn to_decal(&self) -> Decal {
        Decal {
            center: self.center,
            width: self.width,
            height: self.height,
            rotation: self.rotation,
            image: self.image.clone(),
            surface: self.surface.clone(),
            name: self.name.clone(),
            text: self.text.clone(),
            decal_type: self.decal_type.clone(),
            material: self.material.clone(),
            color: self.color,
            sizing_type: self.sizing_type.clone(),
            vertical_text: self.vertical_text,
            backglass: self.backglass,
            font: self.font.to_font(),
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
        }
    }
}

impl Default for Decal {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            width: 100.0,
            height: 100.0,
            rotation: 0.0,
            image: Default::default(),
            surface: Default::default(),
            name: Default::default(),
            text: Default::default(),
            decal_type: DecalType::Image,
            material: Default::default(),
            color: Color::from_rgb(0x000000),
            sizing_type: SizingType::ManualSize,
            vertical_text: false,
            backglass: false,
            font: Font::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl Serialize for Decal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DecalJson::from_decal(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Decal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = DecalJson::deserialize(deserializer)?;
        Ok(json.to_decal())
    }
}

impl GameItem for Decal {
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasSharedAttributes for Decal {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_locked(&self) -> bool {
        self.is_locked
    }

    fn editor_layer(&self) -> Option<u32> {
        self.editor_layer
    }

    fn editor_layer_name(&self) -> Option<&str> {
        self.editor_layer_name.as_deref()
    }

    fn editor_layer_visibility(&self) -> Option<bool> {
        self.editor_layer_visibility
    }

    fn part_group_name(&self) -> Option<&str> {
        self.part_group_name.as_deref()
    }

    fn set_is_locked(&mut self, locked: bool) {
        self.is_locked = locked;
    }

    fn set_editor_layer(&mut self, layer: Option<u32>) {
        self.editor_layer = layer;
    }

    fn set_editor_layer_name(&mut self, name: Option<String>) {
        self.editor_layer_name = name;
    }

    fn set_editor_layer_visibility(&mut self, visibility: Option<bool>) {
        self.editor_layer_visibility = visibility;
    }

    fn set_part_group_name(&mut self, name: Option<String>) {
        self.part_group_name = name;
    }
}

impl BiffRead for Decal {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut decal = Decal::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    decal.center = Vertex2D::biff_read(reader);
                }
                "WDTH" => {
                    decal.width = reader.get_f32();
                }
                "HIGH" => {
                    decal.height = reader.get_f32();
                }
                "ROTA" => {
                    decal.rotation = reader.get_f32();
                }
                "IMAG" => {
                    decal.image = reader.get_string();
                }
                "SURF" => {
                    decal.surface = reader.get_string();
                }
                "NAME" => {
                    decal.name = reader.get_wide_string();
                }
                "TEXT" => {
                    decal.text = reader.get_string();
                }
                "TYPE" => {
                    decal.decal_type = reader.get_u32().into();
                }
                "MATR" => {
                    decal.material = reader.get_string();
                }
                "COLR" => {
                    decal.color = Color::biff_read(reader);
                }
                "SIZE" => {
                    decal.sizing_type = reader.get_u32().into();
                }
                "VERT" => {
                    decal.vertical_text = reader.get_bool();
                }
                "BGLS" => {
                    decal.backglass = reader.get_bool();
                }

                "FONT" => {
                    decal.font = Font::biff_read(reader);
                }
                _ => {
                    if !decal.read_shared_attribute(tag_str, reader) {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag();
                    }
                }
            }
        }
        decal
    }
}

impl BiffWrite for Decal {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("WDTH", self.width);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_f32("ROTA", self.rotation);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("TEXT", &self.text);
        writer.write_tagged_u32("TYPE", (&self.decal_type).into());
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_with("COLR", &self.color, Color::biff_write);
        writer.write_tagged_u32("SIZE", (&self.sizing_type).into());
        writer.write_tagged_bool("VERT", self.vertical_text);
        writer.write_tagged_bool("BGLS", self.backglass);

        self.write_shared_attributes(writer);

        writer.write_tagged_without_size("FONT", &self.font);

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use fake::{Fake, Faker};

    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let decal = Decal {
            center: Vertex2D::new(1.0, 2.0),
            width: 3.0,
            height: 4.0,
            rotation: 5.0,
            image: "image".to_owned(),
            surface: "surface".to_owned(),
            name: "name".to_owned(),
            text: "text".to_owned(),
            decal_type: Faker.fake(),
            material: "material".to_owned(),
            color: Faker.fake(),
            sizing_type: Faker.fake(),
            vertical_text: true,
            backglass: true,
            font: Font::default(),
            is_locked: true,
            editor_layer: Some(3),
            editor_layer_name: Some("editor_layer_name".to_owned()),
            editor_layer_visibility: Some(false),
            part_group_name: Some("part_group_name".to_owned()),
        };
        let mut writer = BiffWriter::new();
        Decal::biff_write(&decal, &mut writer);
        let decal_read = Decal::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(decal, decal_read);
    }

    #[test]
    fn test_write_read_json() {
        // values not equal to the defaults
        let decal: Decal = Faker.fake();
        let decal_json = DecalJson::from_decal(&decal);
        let json = serde_json::to_string(&decal_json).unwrap();
        let decal_read_json: DecalJson = serde_json::from_str(&json).unwrap();
        let mut decal_read = decal_read_json.to_decal();
        // json does not store the shared fields
        decal_read.is_locked = decal.is_locked;
        decal_read.editor_layer = decal.editor_layer;
        decal_read
            .editor_layer_name
            .clone_from(&decal.editor_layer_name);
        decal_read.editor_layer_visibility = decal.editor_layer_visibility;
        assert_eq!(decal, decal_read);
    }

    #[test]
    fn test_decal_type_json() {
        let decal_type = DecalType::Text;
        let json = serde_json::to_string(&decal_type).unwrap();
        assert_eq!(json, "\"text\"");
        let decal_type_read: DecalType = serde_json::from_str(&json).unwrap();
        assert_eq!(decal_type, decal_type_read);
        let json = serde_json::Value::from(1);
        let decal_type_read: DecalType = serde_json::from_value(json).unwrap();
        assert_eq!(DecalType::Image, decal_type_read);
    }

    #[test]
    #[should_panic]
    fn test_decal_type_json_fail() {
        let json: Value = serde_json::Value::from("foo");
        let _decal_type_read: DecalType = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_sizing_type_json() {
        let sizing_type = SizingType::ManualSize;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"manual_size\"");
        let sizing_type_read: SizingType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(1);
        let sizing_type_read: SizingType = serde_json::from_value(json).unwrap();
        assert_eq!(SizingType::AutoWidth, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"Invalid value for SizingType: foo, we expect \\\"auto_size\\\", \\\"auto_width\\\", \\\"manual_size\\\"\", line: 0, column: 0)"]
    fn test_sizing_type_json_fail() {
        let json: Value = serde_json::Value::from("foo");
        let _: SizingType = serde_json::from_value(json).unwrap();
    }
}
