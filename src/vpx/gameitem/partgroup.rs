use crate::vpx::biff;
use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::TimerData;
use crate::vpx::gameitem::vertex2d::Vertex2D;
use log::warn;
use serde::{Deserialize, Serialize};

/// A part group visibility mask, one bit per view/window.
///
/// Mirrors an earlier revision of vpinball's `PartGroup` visibility mask,
/// before it became the player mode mask (desktop, FSS, cabinet, MR, VR).
/// Not read from tables yet.
///
/// This is a bit mask, not an enumeration: any combination of the
/// constants below is a legitimate value, and bits this library does not
/// know are simply kept, so the table round-trips unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct VisibilityMask(pub u32);

impl VisibilityMask {
    /// Visible on the playfield window.
    pub const PLAYFIELD: VisibilityMask = VisibilityMask(1);
    /// Visible on the score view window.
    pub const SCOREVIEW: VisibilityMask = VisibilityMask(2);
    /// Visible on the backglass window.
    pub const BACKGLASS: VisibilityMask = VisibilityMask(4);
    /// Visible on the topper window.
    pub const TOPPER: VisibilityMask = VisibilityMask(8);
    /// Visible on the left apron window.
    pub const APRON_LEFT: VisibilityMask = VisibilityMask(16);
    /// Visible on the right apron window.
    pub const APRON_RIGHT: VisibilityMask = VisibilityMask(32);
    /// Visible in mixed reality (AR) mode.
    pub const MIXED_REALITY: VisibilityMask = VisibilityMask(64);
    /// Visible in virtual reality mode.
    pub const VIRTUAL_REALITY: VisibilityMask = VisibilityMask(128);

    /// Whether every bit of `mask` is set in `self`.
    pub fn contains(self, mask: VisibilityMask) -> bool {
        self.0 & mask.0 == mask.0
    }
}

impl From<u32> for VisibilityMask {
    fn from(value: u32) -> Self {
        VisibilityMask(value)
    }
}
impl From<&VisibilityMask> for u32 {
    fn from(value: &VisibilityMask) -> Self {
        value.0
    }
}
impl From<VisibilityMask> for u32 {
    fn from(value: VisibilityMask) -> Self {
        value.0
    }
}
#[cfg(test)]
mod visibility_mask_tests {
    use super::VisibilityMask;

    #[test]
    fn combinations_are_legitimate_values_and_round_trip() {
        // A combination is a normal mask value, not an unknown.
        let mask = VisibilityMask(VisibilityMask::PLAYFIELD.0 | VisibilityMask::SCOREVIEW.0);
        assert!(mask.contains(VisibilityMask::PLAYFIELD));
        assert!(mask.contains(VisibilityMask::SCOREVIEW));
        assert!(!mask.contains(VisibilityMask::BACKGLASS));
        assert_eq!(u32::from(mask), 3);
        assert_eq!(VisibilityMask::from(3), mask);
    }

    #[test]
    fn unknown_bits_round_trip() {
        let mask = VisibilityMask::from(4_000_000_000);
        assert_eq!(u32::from(&mask), 4_000_000_000);
    }
}

/// Coordinate space a part group is positioned in, mirroring vpinball's
/// `PartGroup::SpaceReference`.
///
/// Values this library does not know are kept in [`SpaceReference::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum SpaceReference {
    /// Relative to cabinet with playfield inclination and local coordinate system applied (usual local playfield coordinate system tailored for table design)
    Playfield,
    /// Relative to cabinet feet, with height adjustment (with height adjustment for lockbar to match cabinet lockbar height after scaling)
    Cabinet,
    /// Relative to room, scaled to fit cabinet size (without any height adjustment, for cabinet feet to touch ground)
    CabinetFeet,
    /// Base space, aligned to (offsetted) real world, without any scaling (to match real world room in AR/VR)
    Room,
    /// Inherit space reference from parent (note that root defaults to Playfield reference space)
    Inherit,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(u32),
}
impl From<u32> for SpaceReference {
    fn from(value: u32) -> Self {
        match value {
            0 => SpaceReference::Playfield,
            1 => SpaceReference::Cabinet,
            2 => SpaceReference::CabinetFeet,
            3 => SpaceReference::Room,
            4 => SpaceReference::Inherit,
            other => {
                warn!("Unknown SpaceReference value {other}, keeping it as is");
                SpaceReference::Other(other)
            }
        }
    }
}
impl From<&SpaceReference> for u32 {
    fn from(value: &SpaceReference) -> Self {
        match value {
            SpaceReference::Playfield => 0,
            SpaceReference::Cabinet => 1,
            SpaceReference::CabinetFeet => 2,
            SpaceReference::Room => 3,
            SpaceReference::Inherit => 4,
            SpaceReference::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`SpaceReference::Other`]
impl Serialize for SpaceReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SpaceReference::Playfield => serializer.serialize_str("playfield"),
            SpaceReference::Cabinet => serializer.serialize_str("cabinet"),
            SpaceReference::CabinetFeet => serializer.serialize_str("cabinet_feet"),
            SpaceReference::Room => serializer.serialize_str("room"),
            SpaceReference::Inherit => serializer.serialize_str("inherit"),
            SpaceReference::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for SpaceReference {
    fn deserialize<D>(deserializer: D) -> Result<SpaceReference, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SpaceReferenceVisitor;
        impl serde::de::Visitor<'_> for SpaceReferenceVisitor {
            type Value = SpaceReference;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a SpaceReference as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<SpaceReference, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(SpaceReference::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<SpaceReference, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "playfield" => Ok(SpaceReference::Playfield),
                    "cabinet" => Ok(SpaceReference::Cabinet),
                    "cabinet_feet" => Ok(SpaceReference::CabinetFeet),
                    "room" => Ok(SpaceReference::Room),
                    "inherit" => Ok(SpaceReference::Inherit),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["playfield", "cabinet", "cabinet_feet", "room", "inherit"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(SpaceReferenceVisitor)
    }
}
#[cfg(test)]
mod space_reference_open_enum_tests {
    use super::SpaceReference;

    #[test]
    fn unknown_value_round_trips() {
        let value = SpaceReference::from(4_000_000_000);
        assert_eq!(value, SpaceReference::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: SpaceReference = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<SpaceReference>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct PartGroup {
    pub name: String,
    /// In vpinball this is just v, but I wanted to unify the naming.
    pub center: Vertex2D,
    pub timer: TimerData,
    pub backglass: bool,
    pub visibility_mask: Option<u32>,
    pub space_reference: SpaceReference,
    pub player_mode_visibility_mask: Option<u32>,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    // Added in 10.8.1
    //pub part_group_name: Option<String>,
}

impl Default for PartGroup {
    fn default() -> Self {
        PartGroup {
            name: Default::default(),
            center: Vertex2D::default(),
            timer: TimerData::default(),
            backglass: false,
            visibility_mask: None,
            space_reference: SpaceReference::Inherit,
            player_mode_visibility_mask: None,
            is_locked: false,
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct PartGroupJson {
    name: String,
    center: Vertex2D,
    #[serde(flatten)]
    pub timer: TimerData,
    backglass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility_mask: Option<u32>,
    space_reference: SpaceReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    player_mode_visibility_mask: Option<u32>,
    is_locked: bool,
    editor_layer_name: Option<String>,
    editor_layer_visibility: Option<bool>,
    // part_group_name: Option<String>,
}

impl PartGroupJson {
    pub fn from_part_group(part_group: &PartGroup) -> Self {
        PartGroupJson {
            name: part_group.name.clone(),
            center: part_group.center,
            timer: part_group.timer.clone(),
            backglass: part_group.backglass,
            visibility_mask: part_group.visibility_mask,
            space_reference: part_group.space_reference.clone(),
            player_mode_visibility_mask: part_group.player_mode_visibility_mask,
            is_locked: part_group.is_locked,
            editor_layer_name: part_group.editor_layer_name.clone(),
            editor_layer_visibility: part_group.editor_layer_visibility,
        }
    }

    pub fn to_part_group(&self) -> PartGroup {
        PartGroup {
            name: self.name.clone(),
            center: self.center,
            timer: self.timer.clone(),
            backglass: self.backglass,
            visibility_mask: self.visibility_mask,
            space_reference: self.space_reference.clone(),
            player_mode_visibility_mask: self.player_mode_visibility_mask,
            is_locked: self.is_locked,
            editor_layer_name: self.editor_layer_name.clone(),
            editor_layer_visibility: self.editor_layer_visibility,
        }
    }
}

impl Serialize for PartGroup {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let part_group_json = PartGroupJson::from_part_group(self);
        part_group_json.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PartGroup {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let part_group_json = PartGroupJson::deserialize(deserializer)?;
        Ok(part_group_json.to_part_group())
    }
}

impl BiffRead for PartGroup {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut part_group = PartGroup::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "NAME" => part_group.name = reader.get_wide_string(),
                "VCEN" => part_group.center = Vertex2D::biff_read(reader),
                "BGLS" => {
                    part_group.backglass = reader.get_bool();
                }
                "VMSK" => {
                    part_group.visibility_mask = Some(reader.get_u32());
                }
                "SPRF" => {
                    part_group.space_reference = reader.get_u32().into();
                }
                "PMSK" => {
                    part_group.player_mode_visibility_mask = Some(reader.get_u32());
                }

                // shared
                "LOCK" => {
                    part_group.is_locked = reader.get_bool();
                }
                "LANR" => {
                    part_group.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    part_group.editor_layer_visibility = Some(reader.get_bool());
                }
                // There are some excludes for this field of which PartGroup is one
                // "GRUP" => {
                //     part_group.part_group_name = Some(reader.get_string());
                // }
                _ => {
                    if !part_group.timer.biff_read_tag(tag_str, reader) {
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
        part_group
    }
}

impl BiffWrite for PartGroup {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged("VCEN", &self.center);
        self.timer.biff_write(writer);
        writer.write_tagged_bool("BGLS", self.backglass);
        if let Some(vmsk) = self.visibility_mask {
            writer.write_tagged_u32("VMSK", vmsk);
        }
        if let Some(pmsk) = self.player_mode_visibility_mask {
            writer.write_tagged_u32("PMSK", pmsk);
        }
        writer.write_tagged_u32("SPRF", (&self.space_reference).into());

        // shared attributes, not using the trait as this one does not have a part_group_name
        writer.write_tagged_bool("LOCK", self.is_locked);
        if let Some(editor_layer_visibility) = self.editor_layer_visibility {
            writer.write_tagged_bool("LVIS", editor_layer_visibility);
        }
        if let Some(editor_layer_name) = &self.editor_layer_name {
            writer.write_tagged_string("LANR", editor_layer_name);
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let part_group = PartGroup {
            name: "Test".to_string(),
            center: Vertex2D::new(1.0, 2.0),
            timer: TimerData {
                is_enabled: true,
                interval: 1000,
            },
            backglass: true,
            visibility_mask: Some(VisibilityMask::PLAYFIELD.into()),
            space_reference: SpaceReference::Cabinet,
            player_mode_visibility_mask: Some(0x00FF),
            is_locked: true,
            editor_layer_name: Some("Layer 1".to_string()),
            editor_layer_visibility: Some(true),
        };

        let mut writer = BiffWriter::new();
        PartGroup::biff_write(&part_group, &mut writer);
        let gate_read = PartGroup::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(part_group, gate_read);
    }
}
