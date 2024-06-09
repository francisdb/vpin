use crate::vpx::gameitem::font::FontJson;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
    gameitem::font::Font,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Clone, Dummy, Default)]
enum TextAlignment {
    #[default]
    Left = 0,
    Center = 1,
    Right = 2,
}

impl From<u32> for TextAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => TextAlignment::Left,
            1 => TextAlignment::Center,
            2 => TextAlignment::Right,
            _ => panic!("Invalid value for TextAlignment: {}", value),
        }
    }
}

impl From<&TextAlignment> for u32 {
    fn from(value: &TextAlignment) -> Self {
        match value {
            TextAlignment::Left => 0,
            TextAlignment::Center => 1,
            TextAlignment::Right => 2,
        }
    }
}

/// Serialize to lowercase string
impl Serialize for TextAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TextAlignment::Left => serializer.serialize_str("left"),
            TextAlignment::Center => serializer.serialize_str("center"),
            TextAlignment::Right => serializer.serialize_str("right"),
        }
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for TextAlignment {
    fn deserialize<D>(deserializer: D) -> Result<TextAlignment, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TextAlignmentVisitor;

        impl<'de> serde::de::Visitor<'de> for TextAlignmentVisitor {
            type Value = TextAlignment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<TextAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(TextAlignment::Left),
                    1 => Ok(TextAlignment::Center),
                    2 => Ok(TextAlignment::Right),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"0, 1, or 2",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<TextAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "left" => Ok(TextAlignment::Left),
                    "center" => Ok(TextAlignment::Center),
                    "right" => Ok(TextAlignment::Right),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["left", "center", "right"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(TextAlignmentVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct TextBox {
    ver1: Vertex2D,         // VER1
    ver2: Vertex2D,         // VER2
    back_color: Color,      // CLRB
    font_color: Color,      // CLRF
    intensity_scale: f32,   // INSC
    text: String,           // TEXT
    is_timer_enabled: bool, // TMON
    timer_interval: i32,    // TMIN
    pub name: String,       // NAME
    align: TextAlignment,   // ALGN
    is_transparent: bool,   // TRNS
    is_dmd: Option<bool>,   // IDMD added in 10.2?
    font: Font,             // FONT

    // these are shared between all items
    pub is_locked: bool,
    // LOCK
    pub editor_layer: u32,
    // LAYR
    pub editor_layer_name: Option<String>,
    // LANR default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>, // LVIS
}

#[derive(Serialize, Deserialize)]
struct TextBoxJson {
    ver1: Vertex2D,
    ver2: Vertex2D,
    back_color: Color,
    font_color: Color,
    intensity_scale: f32,
    text: String,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    align: TextAlignment,
    is_transparent: bool,
    is_dmd: Option<bool>,
    font: FontJson,
}

impl TextBoxJson {
    fn from_textbox(textbox: &TextBox) -> Self {
        Self {
            ver1: textbox.ver1,
            ver2: textbox.ver2,
            back_color: textbox.back_color,
            font_color: textbox.font_color,
            intensity_scale: textbox.intensity_scale,
            text: textbox.text.clone(),
            is_timer_enabled: textbox.is_timer_enabled,
            timer_interval: textbox.timer_interval,
            name: textbox.name.clone(),
            align: textbox.align.clone(),
            is_transparent: textbox.is_transparent,
            is_dmd: textbox.is_dmd,
            font: FontJson::from_font(&textbox.font),
        }
    }

    fn into_textbox(self) -> TextBox {
        TextBox {
            ver1: self.ver1,
            ver2: self.ver2,
            back_color: self.back_color,
            font_color: self.font_color,
            intensity_scale: self.intensity_scale,
            text: self.text,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            name: self.name,
            align: self.align,
            is_transparent: self.is_transparent,
            is_dmd: self.is_dmd,
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

impl Serialize for TextBox {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TextBoxJson::from_textbox(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextBox {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let textbox_json = TextBoxJson::deserialize(deserializer)?;
        Ok(textbox_json.into_textbox())
    }
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            ver1: Vertex2D::default(),
            ver2: Vertex2D::default(),
            back_color: Color::BLACK,
            font_color: Color::WHITE,
            intensity_scale: 1.0,
            text: Default::default(),
            is_timer_enabled: false,
            timer_interval: Default::default(),
            name: Default::default(),
            align: Default::default(),
            is_transparent: false,
            is_dmd: None,
            font: Font::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

impl BiffRead for TextBox {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut textbox = TextBox::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VER1" => {
                    textbox.ver1 = Vertex2D::biff_read(reader);
                }
                "VER2" => {
                    textbox.ver2 = Vertex2D::biff_read(reader);
                }
                "CLRB" => {
                    textbox.back_color = Color::biff_read(reader);
                }
                "CLRF" => {
                    textbox.font_color = Color::biff_read(reader);
                }
                "INSC" => {
                    textbox.intensity_scale = reader.get_f32();
                }
                "TEXT" => {
                    textbox.text = reader.get_string();
                }
                "TMON" => {
                    textbox.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    textbox.timer_interval = reader.get_i32();
                }
                "NAME" => {
                    textbox.name = reader.get_wide_string();
                }
                "ALGN" => {
                    textbox.align = reader.get_u32().into();
                }
                "TRNS" => {
                    textbox.is_transparent = reader.get_bool();
                }
                "IDMD" => {
                    textbox.is_dmd = Some(reader.get_bool());
                }

                "FONT" => {
                    textbox.font = Font::biff_read(reader);
                }
                // shared
                "LOCK" => {
                    textbox.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    textbox.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    textbox.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    textbox.editor_layer_visibility = Some(reader.get_bool());
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
        textbox
    }
}

impl BiffWrite for TextBox {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VER1", &self.ver1);
        writer.write_tagged("VER2", &self.ver2);
        writer.write_tagged_with("CLRB", &self.back_color, Color::biff_write);
        writer.write_tagged_with("CLRF", &self.font_color, Color::biff_write);
        writer.write_tagged_f32("INSC", self.intensity_scale);
        writer.write_tagged_string("TEXT", &self.text);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("ALGN", (&self.align).into());
        writer.write_tagged_bool("TRNS", self.is_transparent);
        if let Some(is_dmd) = self.is_dmd {
            writer.write_tagged_bool("IDMD", is_dmd);
        }

        // shared
        writer.write_tagged_bool("LOCK", self.is_locked);
        writer.write_tagged_u32("LAYR", self.editor_layer);
        if let Some(editor_layer_name) = &self.editor_layer_name {
            writer.write_tagged_string("LANR", editor_layer_name);
        }
        if let Some(editor_layer_visibility) = self.editor_layer_visibility {
            writer.write_tagged_bool("LVIS", editor_layer_visibility);
        }

        writer.write_tagged_without_size("FONT", &self.font);

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use fake::{Fake, Faker};
    use std::collections::HashSet;

    use super::*;
    use crate::vpx::gameitem::font::{FontStyle, CHARSET_ANSI};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read() {
        let textbox = TextBox {
            ver1: Vertex2D::new(1.0, 2.0),
            ver2: Vertex2D::new(3.0, 4.0),
            back_color: Faker.fake(),
            font_color: Faker.fake(),
            intensity_scale: 1.0,
            text: "test text".to_string(),
            is_timer_enabled: true,
            timer_interval: 3,
            name: "test timer".to_string(),
            align: Faker.fake(),
            is_transparent: false,
            is_dmd: Some(false),
            font: Font::new(
                CHARSET_ANSI,
                HashSet::from([FontStyle::Bold, FontStyle::Underline]),
                123,
                456,
                "test font".to_string(),
            ),
            is_locked: false,
            editor_layer: 1,
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(true),
        };
        let mut writer = BiffWriter::new();
        TextBox::biff_write(&textbox, &mut writer);
        let textbox_read = TextBox::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(textbox, textbox_read);
    }

    #[test]
    fn test_text_alignment_json() {
        let sizing_type = TextAlignment::Center;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"center\"");
        let sizing_type_read: TextAlignment = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(2);
        let sizing_type_read: TextAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(TextAlignment::Right, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `left`, `center`, `right`\", line: 0, column: 0)"]
    fn test_text_alignment_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: TextAlignment = serde_json::from_value(json).unwrap();
    }
}
