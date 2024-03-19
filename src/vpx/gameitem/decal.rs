use crate::vpx::color::ColorJson;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{font::Font, font::FontJson, vertex2d::Vertex2D, GameItem};

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
    pub decal_type: u32,
    pub material: String,
    pub color: Color,
    pub sizing_type: u32,
    pub vertical_text: bool,
    pub backglass: bool,

    font: Font,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
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
    decal_type: u32,
    material: String,
    color: ColorJson,
    sizing_type: u32,
    vertical_text: bool,
    backglass: bool,
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
            decal_type: decal.decal_type,
            material: decal.material.clone(),
            color: ColorJson::from_color(&decal.color),
            sizing_type: decal.sizing_type,
            vertical_text: decal.vertical_text,
            backglass: decal.backglass,
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
            decal_type: self.decal_type,
            material: self.material.clone(),
            color: self.color.to_color(),
            sizing_type: self.sizing_type,
            vertical_text: self.vertical_text,
            backglass: self.backglass,
            font: self.font.to_font(),
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
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
            decal_type: Decal::DECAL_TYPE_IMAGE,
            material: Default::default(),
            color: Color::new_bgr(0x000000),
            sizing_type: Decal::SIZING_TYPE_MANUAL_SIZE,
            vertical_text: false,
            backglass: false,
            font: Font::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
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

impl Decal {
    pub const DECAL_TYPE_TEXT: u32 = 0;
    pub const DECAL_TYPE_IMAGE: u32 = 1;

    pub const SIZING_TYPE_AUTO_SIZE: u32 = 0;
    pub const SIZING_TYPE_AUTO_WIDTH: u32 = 1;
    pub const SIZING_TYPE_MANUAL_SIZE: u32 = 2;
}

impl GameItem for Decal {
    fn name(&self) -> &str {
        &self.name
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
                    decal.decal_type = reader.get_u32();
                }
                "MATR" => {
                    decal.material = reader.get_string();
                }
                "COLR" => {
                    decal.color = Color::biff_read_bgr(reader);
                }
                "SIZE" => {
                    decal.sizing_type = reader.get_u32();
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

                // shared
                "LOCK" => {
                    decal.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    decal.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    decal.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    decal.editor_layer_visibility = Some(reader.get_bool());
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
        writer.write_tagged_u32("TYPE", self.decal_type);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_with("COLR", &self.color, Color::biff_write_bgr);
        writer.write_tagged_u32("SIZE", self.sizing_type);
        writer.write_tagged_bool("VERT", self.vertical_text);
        writer.write_tagged_bool("BGLS", self.backglass);

        writer.write_tagged("FONT", &self.font);

        // shared
        writer.write_tagged_bool("LOCK", self.is_locked);
        writer.write_tagged_u32("LAYR", self.editor_layer);
        if let Some(editor_layer_name) = &self.editor_layer_name {
            writer.write_tagged_string("LANR", editor_layer_name);
        }
        if let Some(editor_layer_visibility) = self.editor_layer_visibility {
            writer.write_tagged_bool("LVIS", editor_layer_visibility);
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use fake::{Fake, Faker};

    use super::*;
    use pretty_assertions::assert_eq;

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
            decal_type: 1,
            material: "material".to_owned(),
            color: Color::new_bgr(0x010203),
            sizing_type: 2,
            vertical_text: true,
            backglass: true,
            font: Font::default(),
            is_locked: true,
            editor_layer: 3,
            editor_layer_name: Some("editor_layer_name".to_owned()),
            editor_layer_visibility: Some(false),
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
        decal_read.editor_layer_name = decal.editor_layer_name.clone();
        decal_read.editor_layer_visibility = decal.editor_layer_visibility;
        assert_eq!(decal, decal_read);
    }
}
