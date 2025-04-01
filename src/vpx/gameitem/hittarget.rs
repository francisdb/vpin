use super::vertex3d::Vertex3D;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::{HasSharedAttributes, WriteSharedAttributes};
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum TargetType {
    DropTargetBeveled = 1,
    DropTargetSimple = 2,
    HitTargetRound = 3,
    HitTargetRectangle = 4,
    HitFatTargetRectangle = 5,
    HitFatTargetSquare = 6,
    DropTargetFlatSimple = 7,
    HitFatTargetSlim = 8,
    HitTargetSlim = 9,
}

impl From<u32> for TargetType {
    fn from(value: u32) -> Self {
        match value {
            1 => TargetType::DropTargetBeveled,
            2 => TargetType::DropTargetSimple,
            3 => TargetType::HitTargetRound,
            4 => TargetType::HitTargetRectangle,
            5 => TargetType::HitFatTargetRectangle,
            6 => TargetType::HitFatTargetSquare,
            7 => TargetType::DropTargetFlatSimple,
            8 => TargetType::HitFatTargetSlim,
            9 => TargetType::HitTargetSlim,
            _ => panic!("Invalid TargetType value {}", value),
        }
    }
}

impl From<&TargetType> for u32 {
    fn from(value: &TargetType) -> Self {
        match value {
            TargetType::DropTargetBeveled => 1,
            TargetType::DropTargetSimple => 2,
            TargetType::HitTargetRound => 3,
            TargetType::HitTargetRectangle => 4,
            TargetType::HitFatTargetRectangle => 5,
            TargetType::HitFatTargetSquare => 6,
            TargetType::DropTargetFlatSimple => 7,
            TargetType::HitFatTargetSlim => 8,
            TargetType::HitTargetSlim => 9,
        }
    }
}

/// Serialize TargetType as lowercase string
impl Serialize for TargetType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TargetType::DropTargetBeveled => serializer.serialize_str("drop_target_beveled"),
            TargetType::DropTargetSimple => serializer.serialize_str("drop_target_simple"),
            TargetType::HitTargetRound => serializer.serialize_str("hit_target_round"),
            TargetType::HitTargetRectangle => serializer.serialize_str("hit_target_rectangle"),
            TargetType::HitFatTargetRectangle => {
                serializer.serialize_str("hit_fat_target_rectangle")
            }
            TargetType::HitFatTargetSquare => serializer.serialize_str("hit_fat_target_square"),
            TargetType::DropTargetFlatSimple => serializer.serialize_str("drop_target_flat_simple"),
            TargetType::HitFatTargetSlim => serializer.serialize_str("hit_fat_target_slim"),
            TargetType::HitTargetSlim => serializer.serialize_str("hit_target_slim"),
        }
    }
}

/// Deserialize TargetType from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for TargetType {
    fn deserialize<D>(deserializer: D) -> Result<TargetType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TargetTypeVisitor;

        impl serde::de::Visitor<'_> for TargetTypeVisitor {
            type Value = TargetType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<TargetType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    1 => Ok(TargetType::DropTargetBeveled),
                    2 => Ok(TargetType::DropTargetSimple),
                    3 => Ok(TargetType::HitTargetRound),
                    4 => Ok(TargetType::HitTargetRectangle),
                    5 => Ok(TargetType::HitFatTargetRectangle),
                    6 => Ok(TargetType::HitFatTargetSquare),
                    7 => Ok(TargetType::DropTargetFlatSimple),
                    8 => Ok(TargetType::HitFatTargetSlim),
                    9 => Ok(TargetType::HitTargetSlim),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number between 1 and 9",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<TargetType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "drop_target_beveled" => Ok(TargetType::DropTargetBeveled),
                    "drop_target_simple" => Ok(TargetType::DropTargetSimple),
                    "hit_target_round" => Ok(TargetType::HitTargetRound),
                    "hit_target_rectangle" => Ok(TargetType::HitTargetRectangle),
                    "hit_fat_target_rectangle" => Ok(TargetType::HitFatTargetRectangle),
                    "hit_fat_target_square" => Ok(TargetType::HitFatTargetSquare),
                    "drop_target_flat_simple" => Ok(TargetType::DropTargetFlatSimple),
                    "hit_fat_target_slim" => Ok(TargetType::HitFatTargetSlim),
                    "hit_target_slim" => Ok(TargetType::HitTargetSlim),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "drop_target_beveled",
                            "drop_target_simple",
                            "hit_target_round",
                            "hit_target_rectangle",
                            "hit_fat_target_rectangle",
                            "hit_fat_target_square",
                            "drop_target_flat_simple",
                            "hit_fat_target_slim",
                            "hit_target_slim",
                        ],
                    )),
                }
            }
        }

        deserializer.deserialize_any(TargetTypeVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct HitTarget {
    pub position: Vertex3D,
    pub size: Vertex3D,
    pub rot_z: f32,
    pub image: String,
    pub target_type: TargetType,
    pub name: String,
    pub material: String,
    pub is_visible: bool,
    pub is_legacy: bool,
    pub use_hit_event: bool,
    pub threshold: f32,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    pub scatter: f32,
    pub is_collidable: bool,
    pub disable_lighting_top_old: Option<f32>, // DILI (removed in 10.8)
    pub disable_lighting_top: Option<f32>,     // DILT (added in 10.8)
    pub disable_lighting_below: Option<f32>,   // DILB (added in 10.?)
    pub depth_bias: f32,
    pub is_reflection_enabled: bool,
    pub is_dropped: bool,
    pub drop_speed: f32,
    pub is_timer_enabled: bool,
    pub timer_interval: i32,
    pub raise_delay: Option<u32>,
    // RADE (added in 10.?)
    pub physics_material: Option<String>,
    // MAPH (added in 10.?)
    pub overwrite_physics: Option<bool>, // OVPH (added in 10.?)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}

impl Default for HitTarget {
    fn default() -> Self {
        let position: Vertex3D = Default::default();
        let size = Vertex3D::new(32.0, 32.0, 32.0);
        let rot_z: f32 = 0.0;
        let image: String = Default::default();
        let target_type: TargetType = TargetType::DropTargetSimple;
        let name: String = Default::default();
        let material: String = Default::default();
        let is_visible: bool = true;
        let is_legacy: bool = false;
        let use_hit_event: bool = true;
        let threshold: f32 = 2.0;
        let elasticity: f32 = 0.0;
        let elasticity_falloff: f32 = 0.0;
        let friction: f32 = 0.0;
        let scatter: f32 = 0.0;
        let is_collidable: bool = true;
        let disable_lighting_top_old: Option<f32> = None; //0.0;
        let disable_lighting_top: Option<f32> = None; // 0.0;
        let disable_lighting_below: Option<f32> = None; //0.0;
        let depth_bias: f32 = 0.0;
        let is_reflection_enabled: bool = true;
        let is_dropped: bool = false;
        let drop_speed: f32 = 0.5;
        let is_timer_enabled: bool = false;
        let timer_interval: i32 = 0;
        let raise_delay: Option<u32> = None; //100;
        let physics_material: Option<String> = None;
        let overwrite_physics: Option<bool> = None; //false;

        // these are shared between all items
        let is_locked: bool = false;
        let editor_layer: u32 = Default::default();
        let editor_layer_name: Option<String> = None;
        let editor_layer_visibility: Option<bool> = None;
        let part_group_name: Option<String> = None;
        HitTarget {
            position,
            size,
            rot_z,
            image,
            target_type,
            name,
            material,
            is_visible,
            is_legacy,
            use_hit_event,
            threshold,
            elasticity,
            elasticity_falloff,
            friction,
            scatter,
            is_collidable,
            disable_lighting_top_old,
            disable_lighting_top,
            disable_lighting_below,
            depth_bias,
            is_reflection_enabled,
            is_dropped,
            drop_speed,
            is_timer_enabled,
            timer_interval,
            raise_delay,
            physics_material,
            overwrite_physics,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
            part_group_name,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct HitTargetJson {
    position: Vertex3D,
    size: Vertex3D,
    rot_z: f32,
    image: String,
    target_type: TargetType,
    name: String,
    material: String,
    is_visible: bool,
    is_legacy: bool,
    use_hit_event: bool,
    threshold: f32,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter: f32,
    is_collidable: bool,
    disable_lighting_top_old: Option<f32>,
    disable_lighting_top: Option<f32>,
    disable_lighting_below: Option<f32>,
    depth_bias: f32,
    is_reflection_enabled: bool,
    is_dropped: bool,
    drop_speed: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    raise_delay: Option<u32>,
    physics_material: Option<String>,
    overwrite_physics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl HitTargetJson {
    fn from_hit_target(hit_target: &HitTarget) -> Self {
        Self {
            position: hit_target.position,
            size: hit_target.size,
            rot_z: hit_target.rot_z,
            image: hit_target.image.clone(),
            target_type: hit_target.target_type.clone(),
            name: hit_target.name.clone(),
            material: hit_target.material.clone(),
            is_visible: hit_target.is_visible,
            is_legacy: hit_target.is_legacy,
            use_hit_event: hit_target.use_hit_event,
            threshold: hit_target.threshold,
            elasticity: hit_target.elasticity,
            elasticity_falloff: hit_target.elasticity_falloff,
            friction: hit_target.friction,
            scatter: hit_target.scatter,
            is_collidable: hit_target.is_collidable,
            disable_lighting_top_old: hit_target.disable_lighting_top_old,
            disable_lighting_top: hit_target.disable_lighting_top,
            disable_lighting_below: hit_target.disable_lighting_below,
            depth_bias: hit_target.depth_bias,
            is_reflection_enabled: hit_target.is_reflection_enabled,
            is_dropped: hit_target.is_dropped,
            drop_speed: hit_target.drop_speed,
            is_timer_enabled: hit_target.is_timer_enabled,
            timer_interval: hit_target.timer_interval,
            raise_delay: hit_target.raise_delay,
            physics_material: hit_target.physics_material.clone(),
            overwrite_physics: hit_target.overwrite_physics,
            part_group_name: hit_target.part_group_name.clone(),
        }
    }

    fn to_hit_target(&self) -> HitTarget {
        HitTarget {
            position: self.position,
            size: self.size,
            rot_z: self.rot_z,
            image: self.image.clone(),
            target_type: self.target_type.clone(),
            name: self.name.clone(),
            material: self.material.clone(),
            is_visible: self.is_visible,
            is_legacy: self.is_legacy,
            use_hit_event: self.use_hit_event,
            threshold: self.threshold,
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            scatter: self.scatter,
            is_collidable: self.is_collidable,
            disable_lighting_top_old: self.disable_lighting_top_old,
            disable_lighting_top: self.disable_lighting_top,
            disable_lighting_below: self.disable_lighting_below,
            depth_bias: self.depth_bias,
            is_reflection_enabled: self.is_reflection_enabled,
            is_dropped: self.is_dropped,
            drop_speed: self.drop_speed,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            raise_delay: self.raise_delay,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            part_group_name: self.part_group_name.clone(),
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

impl Serialize for HitTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        HitTargetJson::from_hit_target(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HitTarget {
    fn deserialize<D>(deserializer: D) -> Result<HitTarget, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let json = HitTargetJson::deserialize(deserializer)?;
        Ok(json.to_hit_target())
    }
}

impl HasSharedAttributes for HitTarget {
    fn is_locked(&self) -> bool {
        self.is_locked
    }
    fn editor_layer(&self) -> u32 {
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
}

impl BiffRead for HitTarget {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut position: Vertex3D = Default::default();
        let mut size = Vertex3D::new(32.0, 32.0, 32.0);
        let mut rot_z: f32 = 0.0;
        let mut image: String = Default::default();
        let mut target_type: TargetType = TargetType::DropTargetSimple;
        let mut name: String = Default::default();
        let mut material: String = Default::default();
        let mut is_visible: bool = true;
        let mut is_legacy: bool = false;
        let mut use_hit_event: bool = true;
        let mut threshold: f32 = 2.0;
        let mut elasticity: f32 = 0.0;
        let mut elasticity_falloff: f32 = 0.0;
        let mut friction: f32 = 0.0;
        let mut scatter: f32 = 0.0;
        let mut is_collidable: bool = true;
        let mut disable_lighting_top_old: Option<f32> = None; //0.0;
        let mut disable_lighting_top: Option<f32> = None; //0.0;
        let mut disable_lighting_below: Option<f32> = None; //0.0;
        let mut depth_bias: f32 = 0.0;
        let mut is_reflection_enabled: bool = true;
        let mut is_dropped: bool = false;
        let mut drop_speed: f32 = 0.5;
        let mut is_timer_enabled: bool = false;
        let mut timer_interval: i32 = 0;
        let mut raise_delay: Option<u32> = None; //100;
        let mut physics_material: Option<String> = None;
        let mut overwrite_physics: Option<bool> = None; //false;

        // these are shared between all items
        let mut is_locked: bool = false;
        let mut editor_layer: u32 = Default::default();
        let mut editor_layer_name: Option<String> = None;
        let mut editor_layer_visibility: Option<bool> = None;
        let mut part_group_name: Option<String> = None;

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VPOS" => {
                    position = Vertex3D::biff_read(reader);
                }
                "VSIZ" => {
                    size = Vertex3D::biff_read(reader);
                }
                "ROTZ" => {
                    rot_z = reader.get_f32();
                }
                "IMAG" => {
                    image = reader.get_string();
                }
                "TRTY" => {
                    target_type = reader.get_u32().into();
                }
                "NAME" => {
                    name = reader.get_wide_string();
                }
                "MATR" => {
                    material = reader.get_string();
                }
                "TVIS" => {
                    is_visible = reader.get_bool();
                }
                "LEMO" => {
                    is_legacy = reader.get_bool();
                }
                "HTEV" => {
                    use_hit_event = reader.get_bool();
                }
                "THRS" => {
                    threshold = reader.get_f32();
                }
                "ELAS" => {
                    elasticity = reader.get_f32();
                }
                "ELFO" => {
                    elasticity_falloff = reader.get_f32();
                }
                "RFCT" => {
                    friction = reader.get_f32();
                }
                "RSCT" => {
                    scatter = reader.get_f32();
                }
                "CLDR" => {
                    is_collidable = reader.get_bool();
                }
                "DILI" => {
                    disable_lighting_top_old = Some(reader.get_f32());
                }
                "DILT" => {
                    disable_lighting_top = Some(reader.get_f32());
                }
                "DILB" => {
                    disable_lighting_below = Some(reader.get_f32());
                }
                "REEN" => {
                    is_reflection_enabled = reader.get_bool();
                }
                "PIDB" => {
                    depth_bias = reader.get_f32();
                }
                "ISDR" => {
                    is_dropped = reader.get_bool();
                }
                "DRSP" => {
                    drop_speed = reader.get_f32();
                }
                "TMON" => {
                    is_timer_enabled = reader.get_bool();
                }
                "TMIN" => timer_interval = reader.get_i32(),
                "RADE" => raise_delay = Some(reader.get_u32()),
                "MAPH" => physics_material = Some(reader.get_string()),
                "OVPH" => overwrite_physics = Some(reader.get_bool()),

                // shared
                "LOCK" => {
                    is_locked = reader.get_bool();
                }
                "LAYR" => {
                    editor_layer = reader.get_u32();
                }
                "LANR" => {
                    editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    editor_layer_visibility = Some(reader.get_bool());
                }
                "GRUP" => {
                    part_group_name = Some(reader.get_string());
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
            position,
            size,
            rot_z,
            image,
            target_type,
            name,
            material,
            is_visible,
            is_legacy,
            use_hit_event,
            threshold,
            elasticity,
            elasticity_falloff,
            friction,
            scatter,
            is_collidable,
            disable_lighting_top_old,
            disable_lighting_top,
            disable_lighting_below,
            is_reflection_enabled,
            depth_bias,
            is_dropped,
            drop_speed,
            is_timer_enabled,
            timer_interval,
            raise_delay,
            physics_material,
            overwrite_physics,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
            part_group_name,
        }
    }
}

impl BiffWrite for HitTarget {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VPOS", &self.position);
        writer.write_tagged("VSIZ", &self.size);
        writer.write_tagged_f32("ROTZ", self.rot_z);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_u32("TRTY", (&self.target_type).into());
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("TVIS", self.is_visible);
        writer.write_tagged_bool("LEMO", self.is_legacy);
        writer.write_tagged_bool("HTEV", self.use_hit_event);
        writer.write_tagged_f32("THRS", self.threshold);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("RFCT", self.friction);
        writer.write_tagged_f32("RSCT", self.scatter);
        writer.write_tagged_bool("CLDR", self.is_collidable);
        if let Some(dili) = self.disable_lighting_top_old {
            writer.write_tagged_f32("DILI", dili);
        }
        if let Some(disable_lighting_top) = self.disable_lighting_top {
            writer.write_tagged_f32("DILT", disable_lighting_top);
        }
        if let Some(disable_lighting_below) = self.disable_lighting_below {
            writer.write_tagged_f32("DILB", disable_lighting_below);
        }
        writer.write_tagged_bool("REEN", self.is_reflection_enabled);
        writer.write_tagged_f32("PIDB", self.depth_bias);
        writer.write_tagged_bool("ISDR", self.is_dropped);
        writer.write_tagged_f32("DRSP", self.drop_speed);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        if let Some(raise_delay) = self.raise_delay {
            writer.write_tagged_u32("RADE", raise_delay);
        }
        if let Some(physics_material) = &self.physics_material {
            writer.write_tagged_string("MAPH", physics_material);
        }
        if let Some(overwrite_physics) = self.overwrite_physics {
            writer.write_tagged_bool("OVPH", overwrite_physics);
        }
        self.write_shared_attributes(writer);

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use fake::{Fake, Faker};

    use super::*;
    use crate::vpx::gameitem::tests::RandomOption;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        // values not equal to the defaults
        let hittarget = HitTarget {
            position: Vertex3D::new(rng.random(), rng.random(), rng.random()),
            size: Vertex3D::new(rng.random(), rng.random(), rng.random()),
            rot_z: rng.random(),
            image: "test image".to_string(),
            target_type: Faker.fake(),
            name: "test name".to_string(),
            material: "test material".to_string(),
            is_visible: rng.random(),
            is_legacy: rng.random(),
            use_hit_event: rng.random(),
            threshold: rng.random(),
            elasticity: rng.random(),
            elasticity_falloff: rng.random(),
            friction: rng.random(),
            scatter: rng.random(),
            is_collidable: rng.random(),
            disable_lighting_top_old: Some(rng.random()),
            disable_lighting_top: Some(rng.random()),
            disable_lighting_below: rng.random_option(),
            is_reflection_enabled: rng.random(),
            depth_bias: rng.random(),
            is_dropped: rng.random(),
            drop_speed: rng.random(),
            is_timer_enabled: rng.random(),
            timer_interval: rng.random(),
            raise_delay: rng.random_option(),
            physics_material: Some("test physics material".to_string()),
            overwrite_physics: rng.random_option(),
            is_locked: rng.random(),
            editor_layer: rng.random(),
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: rng.random_option(),
            part_group_name: Some("test group name".to_string()),
        };
        let mut writer = BiffWriter::new();
        HitTarget::biff_write(&hittarget, &mut writer);
        let hittarget_read = HitTarget::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(hittarget, hittarget_read);
    }

    #[test]
    fn test_target_type_json() {
        let sizing_type = TargetType::HitFatTargetRectangle;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"hit_fat_target_rectangle\"");
        let sizing_type_read: TargetType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(1);
        let sizing_type_read: TargetType = serde_json::from_value(json).unwrap();
        assert_eq!(TargetType::DropTargetBeveled, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `drop_target_beveled`, `drop_target_simple`, `hit_target_round`, `hit_target_rectangle`, `hit_fat_target_rectangle`, `hit_fat_target_square`, `drop_target_flat_simple`, `hit_fat_target_slim`, `hit_target_slim`\", line: 0, column: 0)"]
    fn test_target_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: TargetType = serde_json::from_value(json).unwrap();
    }

    #[test]
    #[should_panic = "Error(\"invalid value: integer `0`, expected a number between 1 and 9\", line: 0, column: 0)"]
    fn test_target_type_json_fail_number() {
        let json = serde_json::Value::from(0);
        let _: TargetType = serde_json::from_value(json).unwrap();
    }
}
