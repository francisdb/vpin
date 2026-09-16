use super::vertex2d::Vertex2D;
use crate::vpx::gameitem::font::FontJson;
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use crate::vpx::{
    biff::{self, BiffError, BiffRead, BiffReader, BiffWrite},
    color::Color,
    gameitem::font::Font,
};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Horizontal text alignment of a text box, mirroring vpinball's `TextAlignment`.
///
/// Values this library does not know are kept in [`TextAlignment::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone, Default)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum TextAlignment {
    /// `TextAlignLeft`
    #[default]
    Left,
    /// `TextAlignCenter`
    Center,
    /// `TextAlignRight`
    Right,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(u32),
}
impl From<u32> for TextAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => TextAlignment::Left,
            1 => TextAlignment::Center,
            2 => TextAlignment::Right,
            other => {
                warn!("Unknown TextAlignment value {other}, keeping it as is");
                TextAlignment::Other(other)
            }
        }
    }
}
impl From<&TextAlignment> for u32 {
    fn from(value: &TextAlignment) -> Self {
        match value {
            TextAlignment::Left => 0,
            TextAlignment::Center => 1,
            TextAlignment::Right => 2,
            TextAlignment::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`TextAlignment::Other`]
impl Serialize for TextAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TextAlignment::Left => serializer.serialize_str("left"),
            TextAlignment::Center => serializer.serialize_str("center"),
            TextAlignment::Right => serializer.serialize_str("right"),
            TextAlignment::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for TextAlignment {
    fn deserialize<D>(deserializer: D) -> Result<TextAlignment, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TextAlignmentVisitor;
        impl serde::de::Visitor<'_> for TextAlignmentVisitor {
            type Value = TextAlignment;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a TextAlignment as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<TextAlignment, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(TextAlignment::from(value))
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
#[cfg(test)]
mod text_alignment_open_enum_tests {
    use super::TextAlignment;

    #[test]
    fn unknown_value_round_trips() {
        let value = TextAlignment::from(4_000_000_000);
        assert_eq!(value, TextAlignment::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: TextAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<TextAlignment>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

/// A text box: a 2D rectangle on the desktop backdrop that shows a text
/// the script can change (`Text`), or, when marked as a DMD, the dot matrix
/// display frame of the controller.
///
/// vpinball rasterizes the text with GDI into a texture the size of the
/// rectangle and draws it as a sprite over the backdrop. It is not part of
/// the 3D playfield and has no physics; text boxes are always backdrop
/// items and are skipped in cabinet and VR modes.
///
/// The record is written by `Textbox::Save` and read by `Textbox::Load` in
/// vpinball's `src/parts/textbox.cpp`.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct TextBox {
    /// One corner of the rectangle, in the 1000 x 750 backdrop editor
    /// space.
    ///
    /// vpinball takes the min and max of `ver1` and `ver2` for each axis,
    /// so the order of the two corners does not matter. Default: the
    /// position the box was created at.
    ///
    /// BIFF tag `VER1`
    pub ver1: Vertex2D,
    /// The opposite corner of the rectangle, see [`ver1`](Self::ver1).
    /// Default: `ver1 + (100, 50)`.
    ///
    /// BIFF tag `VER2`
    pub ver2: Vertex2D,
    /// Background color of the rectangle.
    ///
    /// With [`is_transparent`](Self::is_transparent) every pixel of exactly
    /// this color is made fully transparent. Default: black.
    ///
    /// BIFF tag `CLRB`
    pub back_color: Color,
    /// Color of the text. For a DMD text box it tints the dots of a
    /// luminance-only frame. Default: white.
    ///
    /// BIFF tag `CLRF`
    pub font_color: Color,
    /// Brightness multiplier applied when the sprite or the DMD frame is
    /// drawn (`IntensityScale` in script). Default: `1.0`.
    ///
    /// BIFF tag `INSC`
    pub intensity_scale: f32,
    /// Text shown in the box; the script can change it through `Text`.
    ///
    /// For 10.0 compatibility a text containing `DMD` (any case) turns the
    /// box into a DMD, like [`is_dmd`](Self::is_dmd). Default: empty.
    ///
    /// BIFF tag `TEXT`
    pub text: String,
    /// Name of the text box, its identifier in the editor and in scripts.
    /// Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Horizontal alignment of the text within the rectangle, see
    /// [`TextAlignment`]. vpinball's default for a new text box is
    /// [`TextAlignment::Right`].
    ///
    /// BIFF tag `ALGN`
    pub align: TextAlignment,
    /// Whether pixels matching [`back_color`](Self::back_color) are made
    /// transparent, so only the glyphs are drawn over the backdrop.
    /// Default: `false`.
    ///
    /// BIFF tag `TRNS`
    pub is_transparent: bool,
    /// Whether the box shows the controller's DMD frame instead of its
    /// text (compatibility style, the 10.8 flasher DMD modes replace it).
    ///
    /// `None` when the record is absent (file older than 10.2); vpinball
    /// then uses `false`, but a [`text`](Self::text) containing `DMD` still
    /// enables DMD mode. Default: `false`.
    ///
    /// BIFF tag `IDMD` (added in 10.2)
    pub is_dmd: Option<bool>,
    /// Font used to draw the text. Default: Arial Black, 14.25 pt, normal
    /// weight.
    ///
    /// BIFF tag `FONT` (OLE font descriptor)
    pub font: Font,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    /// Whether the item is locked in the editor to prevent accidental
    /// moving or editing. Editor-only; has no runtime effect.
    ///
    /// BIFF tag `LOCK`
    pub is_locked: bool,
    /// Legacy editor layer index (0-based, at most 11). Editor-only.
    ///
    /// Superseded by part groups in 10.8.1, see
    /// [`part_group_name`](Self::part_group_name); vpinball still writes
    /// it, as the index of the item's root group among the root groups, so
    /// older versions can open the file. `None` when the record is absent.
    ///
    /// BIFF tag `LAYR`
    pub editor_layer: Option<u32>,
    /// Name of the editor layer (10.7 named layers). Editor-only.
    ///
    /// Defaults to `"Layer_{editor_layer + 1}"`. Since 10.8.1 vpinball
    /// writes the name of the item's root part group here, for older
    /// versions, and on read maps it to a group when no `GRUP` record
    /// follows. `None` when the record is absent.
    ///
    /// BIFF tag `LANR`
    pub editor_layer_name: Option<String>,
    /// Whether the item is shown in the editor (the 10.7 layer visibility,
    /// stored per item). Editor-only; has no runtime effect. `None` when
    /// the record is absent.
    ///
    /// BIFF tag `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Name of the part group the item belongs to. Added in 10.8.1.
    ///
    /// Part groups replace the editor layers; see
    /// [`PartGroup`](crate::vpx::gameitem::partgroup::PartGroup). `None`
    /// when the record is absent (file older than 10.8.1, or an item that
    /// is not in a group).
    ///
    /// BIFF tag `GRUP`
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(TextBox);

#[derive(Serialize, Deserialize)]
struct TextBoxJson {
    ver1: Vertex2D,
    ver2: Vertex2D,
    back_color: Color,
    font_color: Color,
    intensity_scale: f32,
    text: String,
    #[serde(flatten)]
    pub timer: TimerData,
    name: String,
    align: TextAlignment,
    is_transparent: bool,
    is_dmd: Option<bool>,
    font: FontJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
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
            timer: textbox.timer.clone(),
            name: textbox.name.clone(),
            align: textbox.align.clone(),
            is_transparent: textbox.is_transparent,
            is_dmd: textbox.is_dmd,
            font: FontJson::from_font(&textbox.font),
            part_group_name: textbox.part_group_name.clone(),
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
            timer: self.timer.clone(),
            name: self.name,
            align: self.align,
            is_transparent: self.is_transparent,
            is_dmd: self.is_dmd,
            font: self.font.to_font(),
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name,
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
            timer: TimerData::default(),
            name: Default::default(),
            align: Default::default(),
            is_transparent: false,
            is_dmd: None,
            font: Font::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl BiffRead for TextBox {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut textbox = TextBox::default();

        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VER1" => {
                    textbox.ver1 = Vertex2D::biff_read(reader)?;
                }
                "VER2" => {
                    textbox.ver2 = Vertex2D::biff_read(reader)?;
                }
                "CLRB" => {
                    textbox.back_color = Color::biff_read(reader)?;
                }
                "CLRF" => {
                    textbox.font_color = Color::biff_read(reader)?;
                }
                "INSC" => {
                    textbox.intensity_scale = reader.get_f32()?;
                }
                "TEXT" => {
                    textbox.text = reader.get_string()?;
                }
                "NAME" => {
                    textbox.name = reader.get_wide_string()?;
                }
                "ALGN" => {
                    textbox.align = reader.get_u32()?.into();
                }
                "TRNS" => {
                    textbox.is_transparent = reader.get_bool()?;
                }
                "IDMD" => {
                    textbox.is_dmd = Some(reader.get_bool()?);
                }

                "FONT" => {
                    textbox.font = Font::biff_read(reader)?;
                }
                _ => {
                    if !textbox.timer.biff_read_tag(tag_str, reader)?
                        && !textbox.read_shared_attribute(tag_str, reader)?
                    {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag()?;
                    }
                }
            }
        }
        Ok(textbox)
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
        self.timer.biff_write(writer);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("ALGN", (&self.align).into());
        writer.write_tagged_bool("TRNS", self.is_transparent);
        if let Some(is_dmd) = self.is_dmd {
            writer.write_tagged_bool("IDMD", is_dmd);
        }

        self.write_shared_attributes(writer);

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
    use crate::vpx::gameitem::font::{CHARSET_ANSI, FontStyle};
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
            timer: TimerData {
                is_enabled: true,
                interval: 3,
            },
            name: "test timer".to_string(),
            align: Faker.fake(),
            is_transparent: false,
            is_dmd: Some(false),
            font: Font::new(
                CHARSET_ANSI,
                HashSet::from([FontStyle::Italic, FontStyle::Underline]),
                123,
                456,
                "test font".to_string(),
            ),
            is_locked: false,
            editor_layer: Some(1),
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(true),
            part_group_name: Some("test group".to_string()),
        };
        let mut writer = BiffWriter::new();
        TextBox::biff_write(&textbox, &mut writer);
        let textbox_read = TextBox::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
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
