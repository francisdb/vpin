use crate::vpx::biff;
use crate::vpx::biff::{BiffError, BiffRead, BiffReader, BiffWrite};
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

/// A part group: a named node of the table's part hierarchy. Added in
/// 10.8.1.
///
/// Part groups replace the editor layers of earlier versions (`LAYR`,
/// `LANR`). Every other item names the group it belongs to in its `GRUP`
/// record (the `part_group_name` field of the item), and a group can itself
/// be nested in another group. Besides grouping, a group carries a
/// [`player_mode_visibility_mask`](Self::player_mode_visibility_mask) that
/// selects the player modes its parts are rendered in, and a
/// [`space_reference`](Self::space_reference) that selects the coordinate
/// space its parts are placed in; both are combined along the chain of
/// parent groups. Parts in a group that does not resolve to the playfield
/// space get no physics colliders.
///
/// For files written before 10.8.1 vpinball creates a group per layer
/// name on load. When saving, it still writes `LAYR` and `LANR` for the
/// root group of each item so older versions can open the file.
///
/// The record is written by `PartGroup::Save` and read by
/// `PartGroup::Load` in vpinball's `src/parts/PartGroup.cpp`.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct PartGroup {
    /// Name of the group; items reference it through their `GRUP` record.
    /// Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Position of the group in table coordinates (VPU), used by the editor
    /// to select and move the group. Called `m_v` in vpinball; named
    /// `center` here to match the other items.
    ///
    /// BIFF tag `VCEN`
    pub center: Vertex2D,
    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,
    /// Whether the group is part of the desktop backdrop
    /// (`m_backglass`).
    ///
    /// Part of the part group record from its introduction in the 10.8.1
    /// development builds (March 2025); upstream vpinball dropped it in
    /// March 2026 and keeps a per-item backdrop flag instead. Default:
    /// `false`.
    ///
    /// BIFF tag `BGLS`
    pub backglass: bool,
    /// View visibility mask, one bit per window, see [`VisibilityMask`].
    ///
    /// Only written by vpinball 10.8.1 development builds between March
    /// and September 2025; it was removed as not implemented. `None` when
    /// the record is absent, which is the case for every other file; the
    /// value is written back only when present.
    ///
    /// BIFF tag `VMSK`
    pub visibility_mask: Option<u32>,
    /// Coordinate space the parts of the group are placed in, see
    /// [`SpaceReference`].
    ///
    /// [`SpaceReference::Inherit`] takes the parent group's space; a root
    /// group without an explicit space resolves to
    /// [`SpaceReference::Playfield`]. Parts whose resolved space is not the
    /// playfield are decoration (cabinet, room) and get no physics
    /// colliders. vpinball's default for a new group is `Playfield`; this
    /// library's [`Default`] uses `Inherit`.
    ///
    /// BIFF tag `SPRF`
    pub space_reference: SpaceReference,
    /// Player mode visibility mask: the player modes in which the parts of
    /// the group are rendered.
    ///
    /// Bits: `0x0001` desktop, `0x0002` full single screen, `0x0004`
    /// cabinet, `0x0008` mixed reality, `0x0010` virtual reality;
    /// `0xFFFF` is all modes. The renderer ANDs the masks of the group and
    /// its parents and skips a part when the result has no bit in common
    /// with the current mode. `None` when the record is absent. Default:
    /// `0xFFFF`.
    ///
    /// BIFF tag `PMSK`
    pub player_mode_visibility_mask: Option<u32>,

    // these are shared between all items
    /// Whether the group is locked in the editor to prevent accidental
    /// moving or editing. Editor-only; has no runtime effect.
    ///
    /// BIFF tag `LOCK`
    pub is_locked: bool,
    /// Name of the editor layer (10.7 named layers), written by vpinball
    /// as the name of the group's root group so older versions can open
    /// the file. Editor-only. `None` when the record is absent.
    ///
    /// BIFF tag `LANR`
    pub editor_layer_name: Option<String>,
    /// Whether the group is shown in the editor (the 10.7 layer
    /// visibility, stored per item). Editor-only; has no runtime effect.
    /// `None` when the record is absent.
    ///
    /// BIFF tag `LVIS`
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
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut part_group = PartGroup::default();

        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "NAME" => part_group.name = reader.get_wide_string()?,
                "VCEN" => part_group.center = Vertex2D::biff_read(reader)?,
                "BGLS" => {
                    part_group.backglass = reader.get_bool()?;
                }
                "VMSK" => {
                    part_group.visibility_mask = Some(reader.get_u32()?);
                }
                "SPRF" => {
                    part_group.space_reference = reader.get_u32()?.into();
                }
                "PMSK" => {
                    part_group.player_mode_visibility_mask = Some(reader.get_u32()?);
                }

                // shared
                "LOCK" => {
                    part_group.is_locked = reader.get_bool()?;
                }
                "LANR" => {
                    part_group.editor_layer_name = Some(reader.get_string()?);
                }
                "LVIS" => {
                    part_group.editor_layer_visibility = Some(reader.get_bool()?);
                }
                // There are some excludes for this field of which PartGroup is one
                // "GRUP" => {
                //     part_group.part_group_name = Some(reader.get_string()?);
                // }
                _ => {
                    if !part_group.timer.biff_read_tag(tag_str, reader)? {
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
        Ok(part_group)
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
        let gate_read = PartGroup::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(part_group, gate_read);
    }

    #[test]
    fn json_round_trip_keeps_every_field() {
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
        let json = serde_json::to_value(&part_group).unwrap();
        let back: PartGroup = serde_json::from_value(json).unwrap();
        assert_eq!(part_group, back);
    }

    #[test]
    fn json_omits_absent_mask_records_and_reads_them_back_as_absent() {
        // the short-lived VMSK record and the player mode mask are only
        // written when the file had them; the json must not invent them.
        // Unlike the other items, a part group keeps its editor attributes
        // in its own json, where an absent record is written as null
        let part_group = PartGroup::default();
        assert_eq!(part_group.visibility_mask, None);
        assert_eq!(part_group.player_mode_visibility_mask, None);
        let json = serde_json::to_value(&part_group).unwrap();
        let object = json.as_object().unwrap();
        assert!(!object.contains_key("visibility_mask"));
        assert!(!object.contains_key("player_mode_visibility_mask"));
        assert!(object["editor_layer_name"].is_null());
        assert!(object["editor_layer_visibility"].is_null());
        let back: PartGroup = serde_json::from_value(json).unwrap();
        assert_eq!(part_group, back);
    }
}
