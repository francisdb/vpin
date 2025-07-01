use crate::vpx::biff;
use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::vertex2d::Vertex2D;
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, Clone, PartialEq)]
pub enum VisibilityMask {
    Playfield = 0x0001,
    Scoreview = 0x0002,
    Backglass = 0x0004,
    Topper = 0x0008,
    ApronLeft = 0x0010,
    ApronRight = 0x0020,
    MixedReality = 0x0040,
    VirtualReality = 0x0080,
}

impl From<u32> for VisibilityMask {
    fn from(value: u32) -> Self {
        match value {
            0x0001 => VisibilityMask::Playfield,
            0x0002 => VisibilityMask::Scoreview,
            0x0004 => VisibilityMask::Backglass,
            0x0008 => VisibilityMask::Topper,
            0x0010 => VisibilityMask::ApronLeft,
            0x0020 => VisibilityMask::ApronRight,
            0x0040 => VisibilityMask::MixedReality,
            0x0080 => VisibilityMask::VirtualReality,
            _ => panic!("Unknown visibility mask value: {value}"),
        }
    }
}

impl From<VisibilityMask> for u32 {
    fn from(value: VisibilityMask) -> Self {
        match value {
            VisibilityMask::Playfield => 0x0001,
            VisibilityMask::Scoreview => 0x0002,
            VisibilityMask::Backglass => 0x0004,
            VisibilityMask::Topper => 0x0008,
            VisibilityMask::ApronLeft => 0x0010,
            VisibilityMask::ApronRight => 0x0020,
            VisibilityMask::MixedReality => 0x0040,
            VisibilityMask::VirtualReality => 0x0080,
        }
    }
}

#[derive(Debug, Dummy, Clone, PartialEq)]
pub enum SpaceReference {
    /// Inherit space reference from parent (note that root defaults to Playfield reference space)
    Inherit,
    /// Base space, aligned to (offsetted) real world, without any scaling (to match real world room in AR/VR)
    Room,
    /// Relative to room, scaled to fit cabinet size (without any height adjustment, for cabinet feet to touch ground)
    CabinetFeet,
    /// Relative to cabinet feet, with height adjustment (with height adjustment for lockbar ro match cabinet lockbar height after scaling)
    Cabinet,
    /// Relative to cabinet with playfield inclination and local coordinate system applied (usual local playfield coordinate system tailored for table design)
    Playfield,
}

impl From<u32> for SpaceReference {
    fn from(value: u32) -> Self {
        match value {
            0 => SpaceReference::Inherit,
            1 => SpaceReference::Room,
            2 => SpaceReference::CabinetFeet,
            3 => SpaceReference::Cabinet,
            4 => SpaceReference::Playfield,
            _ => panic!("Unknown space reference value: {value}"),
        }
    }
}

impl From<&SpaceReference> for u32 {
    fn from(value: &SpaceReference) -> Self {
        match value {
            SpaceReference::Inherit => 0,
            SpaceReference::Room => 1,
            SpaceReference::CabinetFeet => 2,
            SpaceReference::Cabinet => 3,
            SpaceReference::Playfield => 4,
        }
    }
}

impl Serialize for SpaceReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            SpaceReference::Inherit => "inherit",
            SpaceReference::Room => "room",
            SpaceReference::CabinetFeet => "cabinet_feet",
            SpaceReference::Cabinet => "cabinet",
            SpaceReference::Playfield => "playfield",
        };
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for SpaceReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "inherit" => Ok(SpaceReference::Inherit),
            "room" => Ok(SpaceReference::Room),
            "cabinet_feet" => Ok(SpaceReference::CabinetFeet),
            "cabinet" => Ok(SpaceReference::Cabinet),
            "playfield" => Ok(SpaceReference::Playfield),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown space reference value: {value}"
            ))),
        }
    }
}

#[derive(Debug, Dummy, PartialEq)]
pub struct PartGroup {
    //     // Standard properties
    //     TimerDataRoot m_tdr;
    pub name: String,
    /// In vpinball this is just v, but I wanted to unify the naming.
    pub center: Vertex2D,
    /// In vpinball this is part of TimerDataRoot
    pub is_timer_enabled: bool,
    /// In vpinball this is part of TimerDataRoot
    pub timer_interval: i32,
    pub backglass: bool,
    pub visibility_mask: u32,
    pub space_reference: SpaceReference,

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
            is_timer_enabled: false,
            timer_interval: 0,
            backglass: false,
            visibility_mask: VisibilityMask::Playfield.into(),
            space_reference: SpaceReference::Inherit,
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
    is_timer_enabled: bool,
    timer_interval: i32,
    backglass: bool,
    visibility_mask: u32,
    space_reference: SpaceReference,
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
            is_timer_enabled: part_group.is_timer_enabled,
            timer_interval: part_group.timer_interval,
            backglass: part_group.backglass,
            visibility_mask: part_group.visibility_mask,
            space_reference: part_group.space_reference.clone(),
            is_locked: part_group.is_locked,
            editor_layer_name: part_group.editor_layer_name.clone(),
            editor_layer_visibility: part_group.editor_layer_visibility,
        }
    }

    pub fn to_part_group(&self) -> PartGroup {
        PartGroup {
            name: self.name.clone(),
            center: self.center,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            backglass: self.backglass,
            visibility_mask: self.visibility_mask,
            space_reference: self.space_reference.clone(),
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
                "TMON" => {
                    part_group.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    part_group.timer_interval = reader.get_i32();
                }
                "BGLS" => {
                    part_group.backglass = reader.get_bool();
                }
                "VMSK" => {
                    part_group.visibility_mask = reader.get_u32();
                }
                "SPRF" => {
                    part_group.space_reference = reader.get_u32().into();
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
                    println!(
                        "Unknown tag {} for {}",
                        tag_str,
                        std::any::type_name::<Self>()
                    );
                    reader.skip_tag();
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
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_bool("BGLS", self.backglass);
        writer.write_tagged_u32("VMSK", self.visibility_mask);
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
            is_timer_enabled: true,
            timer_interval: 1000,
            backglass: true,
            visibility_mask: VisibilityMask::Playfield.into(),
            space_reference: SpaceReference::Cabinet,
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
