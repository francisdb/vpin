use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::dragpoint::DragPoint;

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum RampType {
    Flat = 0,
    FourWire = 1,
    TwoWire = 2,
    ThreeWireLeft = 3,
    ThreeWireRight = 4,
    OneWire = 5,
}

impl From<u32> for RampType {
    fn from(value: u32) -> Self {
        match value {
            0 => RampType::Flat,
            1 => RampType::FourWire,
            2 => RampType::TwoWire,
            3 => RampType::ThreeWireLeft,
            4 => RampType::ThreeWireRight,
            5 => RampType::OneWire,
            _ => panic!("Invalid RampType {}", value),
        }
    }
}

impl From<&RampType> for u32 {
    fn from(value: &RampType) -> Self {
        match value {
            RampType::Flat => 0,
            RampType::FourWire => 1,
            RampType::TwoWire => 2,
            RampType::ThreeWireLeft => 3,
            RampType::ThreeWireRight => 4,
            RampType::OneWire => 5,
        }
    }
}

/// Serializes RampType to lowercase string
impl Serialize for RampType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            RampType::Flat => serializer.serialize_str("flat"),
            RampType::FourWire => serializer.serialize_str("four_wire"),
            RampType::TwoWire => serializer.serialize_str("two_wire"),
            RampType::ThreeWireLeft => serializer.serialize_str("three_wire_left"),
            RampType::ThreeWireRight => serializer.serialize_str("three_wire_right"),
            RampType::OneWire => serializer.serialize_str("one_wire"),
        }
    }
}

/// Deserializes RampType from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for RampType {
    fn deserialize<D>(deserializer: D) -> Result<RampType, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RampTypeVisitor;

        impl<'de> serde::de::Visitor<'de> for RampTypeVisitor {
            type Value = RampType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<RampType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(RampType::Flat),
                    1 => Ok(RampType::FourWire),
                    2 => Ok(RampType::TwoWire),
                    3 => Ok(RampType::ThreeWireLeft),
                    4 => Ok(RampType::ThreeWireRight),
                    5 => Ok(RampType::OneWire),
                    _ => Err(serde::de::Error::unknown_variant(
                        &value.to_string(),
                        &["0", "1", "2", "3", "4", "5"],
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<RampType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "flat" => Ok(RampType::Flat),
                    "four_wire" => Ok(RampType::FourWire),
                    "two_wire" => Ok(RampType::TwoWire),
                    "three_wire_left" => Ok(RampType::ThreeWireLeft),
                    "three_wire_right" => Ok(RampType::ThreeWireRight),
                    "one_wire" => Ok(RampType::OneWire),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "flat",
                            "four_wire",
                            "two_wire",
                            "three_wire_left",
                            "three_wire_right",
                            "one_wire",
                        ],
                    )),
                }
            }
        }

        deserializer.deserialize_any(RampTypeVisitor)
    }
}

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum RampImageAlignment {
    World = 0,
    Wrap = 1,
}

impl From<u32> for RampImageAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => RampImageAlignment::World,
            1 => RampImageAlignment::Wrap,
            _ => panic!("Invalid RampImageAlignment {}", value),
        }
    }
}

impl From<&RampImageAlignment> for u32 {
    fn from(value: &RampImageAlignment) -> Self {
        match value {
            RampImageAlignment::World => 0,
            RampImageAlignment::Wrap => 1,
        }
    }
}

/// Serializes RampImageAlignment to lowercase string
impl Serialize for RampImageAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            RampImageAlignment::World => serializer.serialize_str("world"),
            RampImageAlignment::Wrap => serializer.serialize_str("wrap"),
        }
    }
}

/// Deserializes RampImageAlignment from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for RampImageAlignment {
    fn deserialize<D>(deserializer: D) -> Result<RampImageAlignment, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RampImageAlignmentVisitor;

        impl<'de> serde::de::Visitor<'de> for RampImageAlignmentVisitor {
            type Value = RampImageAlignment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(RampImageAlignment::World),
                    1 => Ok(RampImageAlignment::Wrap),
                    _ => Err(serde::de::Error::unknown_variant(
                        &value.to_string(),
                        &["0", "1"],
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "world" => Ok(RampImageAlignment::World),
                    "wrap" => Ok(RampImageAlignment::Wrap),
                    _ => Err(serde::de::Error::unknown_variant(value, &["world", "wrap"])),
                }
            }
        }

        deserializer.deserialize_any(RampImageAlignmentVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Ramp {
    pub height_bottom: f32,                  // 1
    pub height_top: f32,                     // 2
    pub width_bottom: f32,                   // 3
    pub width_top: f32,                      // 4
    pub material: String,                    // 5
    pub is_timer_enabled: bool,              // 6
    pub timer_interval: u32,                 // 7
    pub ramp_type: RampType,                 // TYPE 8
    pub name: String,                        // 9
    pub image: String,                       // 10
    pub image_alignment: RampImageAlignment, // 11
    pub image_walls: bool,                   // 12
    pub left_wall_height: f32,               // 13
    pub right_wall_height: f32,              // 14
    pub left_wall_height_visible: f32,       // 15
    pub right_wall_height_visible: f32,      // 16
    pub hit_event: Option<bool>,             // HTEV 17 (added in 10.?)
    pub threshold: Option<f32>,              // THRS 18 (added in 10.?)
    pub elasticity: f32,                     // 19
    pub friction: f32,                       // 20
    pub scatter: f32,                        // 21
    pub is_collidable: bool,                 // 22
    pub is_visible: bool,                    // 23
    pub depth_bias: f32,                     // 24
    pub wire_diameter: f32,                  // 25
    pub wire_distance_x: f32,                // 26
    pub wire_distance_y: f32,                // 27
    pub is_reflection_enabled: Option<bool>, // 28 REEN (was missing in 10.01)
    pub physics_material: Option<String>,    // MAPH 29 (added in 10.?)
    pub overwrite_physics: Option<bool>,     // OVPH 30 (added in 10.?)

    drag_points: Vec<DragPoint>,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RampJson {
    height_bottom: f32,
    height_top: f32,
    width_bottom: f32,
    width_top: f32,
    material: String,
    is_timer_enabled: bool,
    timer_interval: u32,
    ramp_type: RampType,
    name: String,
    image: String,
    image_alignment: RampImageAlignment,
    image_walls: bool,
    left_wall_height: f32,
    right_wall_height: f32,
    left_wall_height_visible: f32,
    right_wall_height_visible: f32,
    hit_event: Option<bool>,
    threshold: Option<f32>,
    elasticity: f32,
    friction: f32,
    scatter: f32,
    is_collidable: bool,
    is_visible: bool,
    depth_bias: f32,
    wire_diameter: f32,
    wire_distance_x: f32,
    wire_distance_y: f32,
    is_reflection_enabled: Option<bool>,
    physics_material: Option<String>,
    overwrite_physics: Option<bool>, // true;
    drag_points: Vec<DragPoint>,
}

impl RampJson {
    fn from_ramp(ramp: &Ramp) -> Self {
        Self {
            height_bottom: ramp.height_bottom,
            height_top: ramp.height_top,
            width_bottom: ramp.width_bottom,
            width_top: ramp.width_top,
            material: ramp.material.clone(),
            is_timer_enabled: ramp.is_timer_enabled,
            timer_interval: ramp.timer_interval,
            ramp_type: ramp.ramp_type.clone(),
            name: ramp.name.clone(),
            image: ramp.image.clone(),
            image_alignment: ramp.image_alignment.clone(),
            image_walls: ramp.image_walls,
            left_wall_height: ramp.left_wall_height,
            right_wall_height: ramp.right_wall_height,
            left_wall_height_visible: ramp.left_wall_height_visible,
            right_wall_height_visible: ramp.right_wall_height_visible,
            hit_event: ramp.hit_event,
            threshold: ramp.threshold,
            elasticity: ramp.elasticity,
            friction: ramp.friction,
            scatter: ramp.scatter,
            is_collidable: ramp.is_collidable,
            is_visible: ramp.is_visible,
            depth_bias: ramp.depth_bias,
            wire_diameter: ramp.wire_diameter,
            wire_distance_x: ramp.wire_distance_x,
            wire_distance_y: ramp.wire_distance_y,
            is_reflection_enabled: ramp.is_reflection_enabled,
            physics_material: ramp.physics_material.clone(),
            overwrite_physics: ramp.overwrite_physics,
            drag_points: ramp.drag_points.clone(),
        }
    }

    fn to_ramp(&self) -> Ramp {
        Ramp {
            height_bottom: self.height_bottom,
            height_top: self.height_top,
            width_bottom: self.width_bottom,
            width_top: self.width_top,
            material: self.material.clone(),
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            ramp_type: self.ramp_type.clone(),
            name: self.name.clone(),
            image: self.image.clone(),
            image_alignment: self.image_alignment.clone(),
            image_walls: self.image_walls,
            left_wall_height: self.left_wall_height,
            right_wall_height: self.right_wall_height,
            left_wall_height_visible: self.left_wall_height_visible,
            right_wall_height_visible: self.right_wall_height_visible,
            hit_event: self.hit_event,
            threshold: self.threshold,
            elasticity: self.elasticity,
            friction: self.friction,
            scatter: self.scatter,
            is_collidable: self.is_collidable,
            is_visible: self.is_visible,
            depth_bias: self.depth_bias,
            wire_diameter: self.wire_diameter,
            wire_distance_x: self.wire_distance_x,
            wire_distance_y: self.wire_distance_y,
            is_reflection_enabled: self.is_reflection_enabled,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            drag_points: self.drag_points.clone(),
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

impl Serialize for Ramp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RampJson::from_ramp(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Ramp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ramp_json = RampJson::deserialize(deserializer)?;
        Ok(ramp_json.to_ramp())
    }
}

impl Default for Ramp {
    fn default() -> Self {
        Self {
            height_bottom: 0.0,
            height_top: 50.0,
            width_bottom: 75.0,
            width_top: 60.0,
            material: Default::default(),
            is_timer_enabled: Default::default(),
            timer_interval: Default::default(),
            ramp_type: RampType::Flat,
            name: Default::default(),
            image: Default::default(),
            image_alignment: RampImageAlignment::World,
            image_walls: true,
            left_wall_height: 62.0,
            right_wall_height: 62.0,
            left_wall_height_visible: 30.0,
            right_wall_height_visible: 30.0,
            hit_event: None,
            threshold: None,
            elasticity: Default::default(),
            friction: Default::default(),
            scatter: Default::default(),
            is_collidable: true,
            is_visible: true,
            depth_bias: 0.0,
            wire_diameter: 8.0,
            wire_distance_x: 38.0,
            wire_distance_y: 88.0,
            is_reflection_enabled: None, // true,
            physics_material: None,
            overwrite_physics: None, // true;
            drag_points: Default::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

impl BiffRead for Ramp {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut ramp = Ramp::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "HTBT" => {
                    ramp.height_bottom = reader.get_f32();
                }
                "HTTP" => {
                    ramp.height_top = reader.get_f32();
                }
                "WDBT" => {
                    ramp.width_bottom = reader.get_f32();
                }
                "WDTP" => {
                    ramp.width_top = reader.get_f32();
                }
                "MATR" => {
                    ramp.material = reader.get_string();
                }
                "TMON" => {
                    ramp.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    ramp.timer_interval = reader.get_u32();
                }
                "TYPE" => {
                    ramp.ramp_type = reader.get_u32().into();
                }
                "NAME" => {
                    ramp.name = reader.get_wide_string();
                }
                "IMAG" => {
                    ramp.image = reader.get_string();
                }
                "ALGN" => {
                    ramp.image_alignment = reader.get_u32().into();
                }
                "IMGW" => {
                    ramp.image_walls = reader.get_bool();
                }
                "WLHL" => {
                    ramp.left_wall_height = reader.get_f32();
                }
                "WLHR" => {
                    ramp.right_wall_height = reader.get_f32();
                }
                "WVHL" => {
                    ramp.left_wall_height_visible = reader.get_f32();
                }
                "WVHR" => {
                    ramp.right_wall_height_visible = reader.get_f32();
                }
                "HTEV" => {
                    ramp.hit_event = Some(reader.get_bool());
                }
                "THRS" => {
                    ramp.threshold = Some(reader.get_f32());
                }
                "ELAS" => {
                    ramp.elasticity = reader.get_f32();
                }
                "RFCT" => {
                    ramp.friction = reader.get_f32();
                }
                "RSCT" => {
                    ramp.scatter = reader.get_f32();
                }
                "CLDR" => {
                    ramp.is_collidable = reader.get_bool();
                }
                "RVIS" => {
                    ramp.is_visible = reader.get_bool();
                }
                "RADB" => {
                    ramp.depth_bias = reader.get_f32();
                }
                "RADI" => {
                    ramp.wire_diameter = reader.get_f32();
                }
                "RADX" => {
                    ramp.wire_distance_x = reader.get_f32();
                }
                "RADY" => {
                    ramp.wire_distance_y = reader.get_f32();
                }
                "REEN" => {
                    ramp.is_reflection_enabled = Some(reader.get_bool());
                }
                "MAPH" => {
                    ramp.physics_material = Some(reader.get_string());
                }
                "OVPH" => {
                    ramp.overwrite_physics = Some(reader.get_bool());
                }
                "PNTS" => {
                    // this is just a tag with no data
                }
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    ramp.drag_points.push(point);
                }

                // shared
                "LOCK" => {
                    ramp.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    ramp.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    ramp.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    ramp.editor_layer_visibility = Some(reader.get_bool());
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
        ramp
    }
}

impl BiffWrite for Ramp {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_f32("HTBT", self.height_bottom);
        writer.write_tagged_f32("HTTP", self.height_top);
        writer.write_tagged_f32("WDBT", self.width_bottom);
        writer.write_tagged_f32("WDTP", self.width_top);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        writer.write_tagged_u32("TYPE", (&self.ramp_type).into());
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_u32("ALGN", (&self.image_alignment).into());
        writer.write_tagged_bool("IMGW", self.image_walls);
        writer.write_tagged_f32("WLHL", self.left_wall_height);
        writer.write_tagged_f32("WLHR", self.right_wall_height);
        writer.write_tagged_f32("WVHL", self.left_wall_height_visible);
        writer.write_tagged_f32("WVHR", self.right_wall_height_visible);
        if let Some(hit_event) = self.hit_event {
            writer.write_tagged_bool("HTEV", hit_event);
        }
        if let Some(threshold) = self.threshold {
            writer.write_tagged_f32("THRS", threshold);
        }
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("RFCT", self.friction);
        writer.write_tagged_f32("RSCT", self.scatter);
        writer.write_tagged_bool("CLDR", self.is_collidable);
        writer.write_tagged_bool("RVIS", self.is_visible);
        writer.write_tagged_f32("RADB", self.depth_bias);
        writer.write_tagged_f32("RADI", self.wire_diameter);
        writer.write_tagged_f32("RADX", self.wire_distance_x);
        writer.write_tagged_f32("RADY", self.wire_distance_y);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        if let Some(physics_material) = &self.physics_material {
            writer.write_tagged_string("MAPH", physics_material);
        }
        if let Some(overwrite_physics) = self.overwrite_physics {
            writer.write_tagged_bool("OVPH", overwrite_physics);
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
        writer.write_marker_tag("PNTS");
        for point in &self.drag_points {
            writer.write_tagged("DPNT", point)
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;

    use super::*;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::thread_rng();
        let ramp = Ramp {
            height_bottom: 1.0,
            height_top: 2.0,
            width_bottom: 3.0,
            width_top: 4.0,
            material: "material".to_string(),
            is_timer_enabled: rng.gen(),
            timer_interval: 5,
            ramp_type: Faker.fake(),
            name: "name".to_string(),
            image: "image".to_string(),
            image_alignment: Faker.fake(),
            image_walls: rng.gen(),
            left_wall_height: 8.0,
            right_wall_height: 9.0,
            left_wall_height_visible: 10.0,
            right_wall_height_visible: 11.0,
            hit_event: rng.gen(),
            threshold: rng.gen(),
            elasticity: 13.0,
            friction: 14.0,
            scatter: 15.0,
            is_collidable: rng.gen(),
            is_visible: rng.gen(),
            depth_bias: 16.0,
            wire_diameter: 17.0,
            wire_distance_x: 18.0,
            wire_distance_y: 19.0,
            is_reflection_enabled: rng.gen(),
            physics_material: Some("physics_material".to_string()),
            overwrite_physics: rng.gen(),
            drag_points: vec![DragPoint::default()],
            is_locked: true,
            editor_layer: 22,
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: Some(true),
        };
        let mut writer = BiffWriter::new();
        Ramp::biff_write(&ramp, &mut writer);
        let ramp_read = Ramp::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(ramp, ramp_read);
    }

    #[test]
    fn test_ramp_type_json() {
        let sizing_type = RampType::FourWire;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"four_wire\"");
        let sizing_type_read: RampType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RampType = serde_json::from_value(json).unwrap();
        assert_eq!(RampType::Flat, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `flat`, `four_wire`, `two_wire`, `three_wire_left`, `three_wire_right`, `one_wire`\", line: 0, column: 0)"]
    fn test_shadow_mode_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: RampType = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_image_alignment_json() {
        let sizing_type = RampImageAlignment::Wrap;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"wrap\"");
        let sizing_type_read: RampImageAlignment = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RampImageAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(RampImageAlignment::World, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected `world` or `wrap`\", line: 0, column: 0)"]
    fn test_image_alignment_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: RampImageAlignment = serde_json::from_value(json).unwrap();
    }
}
