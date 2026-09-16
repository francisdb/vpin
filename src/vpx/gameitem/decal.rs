use super::{GameItem, font::Font, font::FontJson, vertex2d::Vertex2D};
use crate::vpx::gameitem::select::WriteSharedAttributes;
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::{
    biff::{self, BiffError, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What a decal shows, mirroring vpinball's `DecalType`.
///
/// Values this library does not know are kept in [`DecalType::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum DecalType {
    /// `DecalText`: renders the decal text with its font.
    Text,
    /// `DecalImage`: renders the decal image.
    Image,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(#[cfg_attr(test, proptest(strategy = "2..=u32::MAX"))] u32),
}
impl From<u32> for DecalType {
    fn from(value: u32) -> Self {
        match value {
            0 => DecalType::Text,
            1 => DecalType::Image,
            other => {
                warn!("Unknown DecalType value {other}, keeping it as is");
                DecalType::Other(other)
            }
        }
    }
}
impl From<&DecalType> for u32 {
    fn from(value: &DecalType) -> Self {
        match value {
            DecalType::Text => 0,
            DecalType::Image => 1,
            DecalType::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`DecalType::Other`]
impl Serialize for DecalType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            DecalType::Text => serializer.serialize_str("text"),
            DecalType::Image => serializer.serialize_str("image"),
            DecalType::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for DecalType {
    fn deserialize<D>(deserializer: D) -> Result<DecalType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DecalTypeVisitor;
        impl serde::de::Visitor<'_> for DecalTypeVisitor {
            type Value = DecalType;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a DecalType as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<DecalType, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(DecalType::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<DecalType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "text" => Ok(DecalType::Text),
                    "image" => Ok(DecalType::Image),
                    _ => Err(serde::de::Error::unknown_variant(value, &["text", "image"])),
                }
            }
        }
        deserializer.deserialize_any(DecalTypeVisitor)
    }
}
#[cfg(test)]
mod decal_type_open_enum_tests {
    use super::DecalType;

    #[test]
    fn unknown_value_round_trips() {
        let value = DecalType::from(4_000_000_000);
        assert_eq!(value, DecalType::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: DecalType = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(serde_json::from_value::<DecalType>(serde_json::json!("no_such_variant")).is_err());
    }
}

/// How a decal is sized, mirroring vpinball's `SizingType`.
///
/// Values this library does not know are kept in [`SizingType::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum SizingType {
    /// `AutoSize`: width and height follow the content.
    AutoSize,
    /// `AutoWidth`: the width follows the content, the height is set manually.
    AutoWidth,
    /// `ManualSize`: width and height are set manually.
    ManualSize,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(#[cfg_attr(test, proptest(strategy = "3..=u32::MAX"))] u32),
}
impl From<u32> for SizingType {
    fn from(value: u32) -> Self {
        match value {
            0 => SizingType::AutoSize,
            1 => SizingType::AutoWidth,
            2 => SizingType::ManualSize,
            other => {
                warn!("Unknown SizingType value {other}, keeping it as is");
                SizingType::Other(other)
            }
        }
    }
}
impl From<&SizingType> for u32 {
    fn from(value: &SizingType) -> Self {
        match value {
            SizingType::AutoSize => 0,
            SizingType::AutoWidth => 1,
            SizingType::ManualSize => 2,
            SizingType::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`SizingType::Other`]
impl Serialize for SizingType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SizingType::AutoSize => serializer.serialize_str("auto_size"),
            SizingType::AutoWidth => serializer.serialize_str("auto_width"),
            SizingType::ManualSize => serializer.serialize_str("manual_size"),
            SizingType::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for SizingType {
    fn deserialize<D>(deserializer: D) -> Result<SizingType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SizingTypeVisitor;
        impl serde::de::Visitor<'_> for SizingTypeVisitor {
            type Value = SizingType;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a SizingType as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<SizingType, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(SizingType::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<SizingType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "auto_size" => Ok(SizingType::AutoSize),
                    "auto_width" => Ok(SizingType::AutoWidth),
                    "manual_size" => Ok(SizingType::ManualSize),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["auto_size", "auto_width", "manual_size"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(SizingTypeVisitor)
    }
}
#[cfg(test)]
mod sizing_type_open_enum_tests {
    use super::SizingType;

    #[test]
    fn unknown_value_round_trips() {
        let value = SizingType::from(4_000_000_000);
        assert_eq!(value, SizingType::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: SizingType = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<SizingType>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

/// A decal: a flat image or a line of text lying on the playfield, on a
/// surface, or on the desktop backdrop.
///
/// vpinball renders it as a single textured quad of
/// [`width`](Self::width) x [`height`](Self::height) VPU centered on
/// [`center`](Self::center), rotated by [`rotation`](Self::rotation) and
/// placed 0.2 VPU above the height of [`surface`](Self::surface). A text
/// decal is rasterized with its [`font`](Self::font) into a texture at
/// render setup. Decals have no physics and are prerendered when their
/// material is opaque.
///
/// The record is written by `Decal::Save` and read by `Decal::Load` in
/// vpinball's `src/parts/decal.cpp`.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Decal {
    /// Position of the decal center in table coordinates (VPU).
    ///
    /// For a backdrop decal ([`backglass`](Self::backglass)) the
    /// coordinates are in the 1000 x 750 backdrop editor space and are
    /// scaled to the render target.
    ///
    /// BIFF tag `VCEN`
    pub center: Vertex2D,
    /// Width of the decal quad in VPU.
    ///
    /// Used as is for `ManualSize`, for image decals with `AutoSize`
    /// (treated as manual) and for vertical text with `AutoWidth`; in the
    /// other cases vpinball derives the width from the text or the image
    /// aspect ratio, see [`sizing_type`](Self::sizing_type). Default:
    /// `100.0`.
    ///
    /// BIFF tag `WDTH`
    pub width: f32,
    /// Height of the decal quad in VPU.
    ///
    /// Used as is except for `AutoSize` text decals, where the height
    /// follows the font size (`font.size / 2545` VPU, multiplied by the
    /// character count for vertical text). Default: `100.0`.
    ///
    /// BIFF tag `HIGH`
    pub height: f32,
    /// Rotation of the quad around its center, in degrees. Default:
    /// `0.0`.
    ///
    /// BIFF tag `ROTA`
    pub rotation: f32,
    /// Name of the table image shown by an image decal
    /// ([`DecalType::Image`]); ignored for text decals.
    ///
    /// When empty, or the image is missing, the quad is rendered with the
    /// material only. Default: empty.
    ///
    /// BIFF tag `IMAG`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub image: String,
    /// The name of the surface (wall, ramp, or empty for playfield) that this decal sits on.
    /// Used to determine the Z height of the decal via `GetSurfaceHeight()`.
    /// The decal is rendered at surface_height + 0.2 units.
    ///
    /// BIFF tag: `SURF`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub surface: String,
    /// Name of the decal, its identifier in the editor and in scripts.
    /// Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Text drawn by a text decal ([`DecalType::Text`]); ignored for image
    /// decals.
    ///
    /// An empty text makes vpinball fall back to the manual size. Default:
    /// empty.
    ///
    /// BIFF tag `TEXT`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub text: String,
    /// Whether the decal shows its [`text`](Self::text) or its
    /// [`image`](Self::image), see [`DecalType`]. Default:
    /// [`DecalType::Image`].
    ///
    /// BIFF tag `TYPE`
    pub decal_type: DecalType,
    /// Name of the table material applied to the quad.
    ///
    /// vpinball uses the material's active opacity flag to decide whether
    /// the decal is prerendered with the static parts (opaque) or drawn in
    /// the dynamic pass. Default: empty (the default material).
    ///
    /// BIFF tag `MATR`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub material: String,
    /// Color of the text of a text decal (`FontColor` in script); ignored
    /// for image decals.
    ///
    /// At render setup vpinball nudges pure white to `RGB(254, 255, 255)`
    /// and pure black to `RGB(0, 0, 1)` so the text does not clash with the
    /// transparent color of the rasterized texture. Default: black.
    ///
    /// BIFF tag `COLR`
    pub color: Color,
    /// How [`width`](Self::width) and [`height`](Self::height) are derived,
    /// see [`SizingType`]. Image decals treat `AutoSize` as `ManualSize`.
    /// Default: [`SizingType::ManualSize`].
    ///
    /// BIFF tag `SIZE`
    pub sizing_type: SizingType,
    /// Draws the text of a text decal one character per line, from top to
    /// bottom, instead of horizontally. Default: `false`.
    ///
    /// BIFF tag `VERT`
    pub vertical_text: bool,
    /// Whether the decal is part of the desktop backdrop (the 2D backglass
    /// area of desktop mode) instead of the playfield.
    ///
    /// vpinball keeps this in `IEditable::m_desktopBackdrop`. A backdrop
    /// decal is positioned in the backdrop editor space, drawn without
    /// depth and only when
    /// [`GameData::render_decals`](crate::vpx::gamedata::GameData::render_decals)
    /// is set; backdrop items are skipped in cabinet and VR modes and in
    /// reflections. Default: `false`.
    ///
    /// BIFF tag `BGLS`
    pub backglass: bool,

    /// Font used to rasterize a text decal; ignored for image decals.
    ///
    /// The stored size is the point size x 10000; an auto sized text decal
    /// is `size / 2545` VPU high. Default: Arial Black, 14.25 pt, normal
    /// weight, no italic, underline or strikethrough.
    ///
    /// BIFF tag `FONT` (OLE font descriptor)
    pub font: Font,

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
    #[cfg_attr(
        test,
        proptest(strategy = "proptest::option::of(crate::vpx::test_support::latin1_string())")
    )]
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
    #[cfg_attr(
        test,
        proptest(strategy = "proptest::option::of(crate::vpx::test_support::latin1_string())")
    )]
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Decal);

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

impl BiffRead for Decal {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut decal = Decal::default();
        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    decal.center = Vertex2D::biff_read(reader)?;
                }
                "WDTH" => {
                    decal.width = reader.get_f32()?;
                }
                "HIGH" => {
                    decal.height = reader.get_f32()?;
                }
                "ROTA" => {
                    decal.rotation = reader.get_f32()?;
                }
                "IMAG" => {
                    decal.image = reader.get_string()?;
                }
                "SURF" => {
                    decal.surface = reader.get_string()?;
                }
                "NAME" => {
                    decal.name = reader.get_wide_string()?;
                }
                "TEXT" => {
                    decal.text = reader.get_string()?;
                }
                "TYPE" => {
                    decal.decal_type = reader.get_u32()?.into();
                }
                "MATR" => {
                    decal.material = reader.get_string()?;
                }
                "COLR" => {
                    decal.color = Color::biff_read(reader)?;
                }
                "SIZE" => {
                    decal.sizing_type = reader.get_u32()?.into();
                }
                "VERT" => {
                    decal.vertical_text = reader.get_bool()?;
                }
                "BGLS" => {
                    decal.backglass = reader.get_bool()?;
                }

                "FONT" => {
                    decal.font = Font::biff_read(reader)?;
                }
                _ => {
                    if !decal.read_shared_attribute(tag_str, reader)? {
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
        Ok(decal)
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
    use crate::vpx::test_support::debug;
    use proptest::prelude::*;

    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    proptest! {
        #[test]
        fn any_decal_round_trips_through_its_records(decal in any::<Decal>()) {
            let mut writer = BiffWriter::new();
            Decal::biff_write(&decal, &mut writer);
            let read = Decal::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
            prop_assert_eq!(debug(&decal), debug(&read));
        }
    }

    proptest! {
        #[test]
        fn any_decal_round_trips_through_json(
            decal in any::<Decal>().prop_filter("json cannot carry NaN or infinity", |decal| {
                [decal.center.x, decal.center.y, decal.width, decal.height, decal.rotation]
                    .iter()
                    .all(|value| value.is_finite())
            })
        ) {
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
            prop_assert_eq!(debug(&decal), debug(&decal_read));
        }
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
    fn test_sizing_type_json_fail() {
        let json: Value = serde_json::Value::from("foo");
        let err = serde_json::from_value::<SizingType>(json).unwrap_err();
        assert!(err.to_string().contains("unknown variant `foo`"), "{err}");
    }
}

#[cfg(test)]
mod json_error_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invalid_json_numbers_are_errors_not_panics() {
        for value in [json!(1.5), json!(-1), json!(true), json!(null)] {
            assert!(
                serde_json::from_value::<DecalType>(value.clone()).is_err(),
                "{value}"
            );
            assert!(
                serde_json::from_value::<SizingType>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}
