use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum PlungerType {
    Modern = 1,
    Flat = 2,
    Custom = 3,
}

impl From<u32> for PlungerType {
    fn from(value: u32) -> Self {
        match value {
            1 => PlungerType::Modern,
            2 => PlungerType::Flat,
            3 => PlungerType::Custom,
            _ => panic!("Invalid PlungerType value {}", value),
        }
    }
}

impl From<&PlungerType> for u32 {
    fn from(value: &PlungerType) -> Self {
        match value {
            PlungerType::Modern => 1,
            PlungerType::Flat => 2,
            PlungerType::Custom => 3,
        }
    }
}

/// Serialize to lowercase string
impl Serialize for PlungerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PlungerType::Modern => serializer.serialize_str("modern"),
            PlungerType::Flat => serializer.serialize_str("flat"),
            PlungerType::Custom => serializer.serialize_str("custom"),
        }
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for PlungerType {
    fn deserialize<D>(deserializer: D) -> Result<PlungerType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PlungerTypeVisitor;

        impl<'de> serde::de::Visitor<'de> for PlungerTypeVisitor {
            type Value = PlungerType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<PlungerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    1 => Ok(PlungerType::Modern),
                    2 => Ok(PlungerType::Flat),
                    3 => Ok(PlungerType::Custom),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"1, 2, or 3",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<PlungerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "modern" => Ok(PlungerType::Modern),
                    "flat" => Ok(PlungerType::Flat),
                    "custom" => Ok(PlungerType::Custom),

                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["modern", "flat", "custom"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(PlungerTypeVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Plunger {
    pub center: Vertex2D,
    width: f32,
    height: f32,
    z_adjust: f32,
    stroke: f32,
    speed_pull: f32,
    speed_fire: f32,
    plunger_type: PlungerType,
    anim_frames: u32,
    material: String,
    image: String,
    mech_strength: f32,
    is_mech_plunger: bool,
    auto_plunger: bool,
    park_position: f32,
    scatter_velocity: f32,
    momentum_xfer: f32,
    is_timer_enabled: bool,
    timer_interval: u32,
    is_visible: bool,
    is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)
    surface: String,
    pub name: String,
    tip_shape: String,
    rod_diam: f32,
    ring_gap: f32,
    ring_diam: f32,
    ring_width: f32,
    spring_diam: f32,
    spring_gauge: f32,
    spring_loops: f32,
    spring_end_loops: f32,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

impl Default for Plunger {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            width: 25.0,
            height: 20.0,
            z_adjust: 0.0,
            stroke: 80.0,
            speed_pull: 0.5,
            speed_fire: 80.0,
            plunger_type: PlungerType::Modern,
            anim_frames: 1,
            material: String::default(),
            image: String::default(),
            mech_strength: 85.0,
            is_mech_plunger: false,
            auto_plunger: false,
            park_position: 0.5 / 3.0,
            scatter_velocity: 0.0,
            momentum_xfer: 1.0,
            is_timer_enabled: false,
            timer_interval: 0,
            is_visible: true,
            is_reflection_enabled: Some(true),
            surface: String::default(),
            name: String::default(),
            tip_shape: "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .84"
                .to_string(),
            rod_diam: 0.6,
            ring_gap: 2.0,
            ring_diam: 0.94,
            ring_width: 3.0,
            spring_diam: 0.77,
            spring_gauge: 1.38,
            spring_loops: 8.0,
            spring_end_loops: 2.5,
            is_locked: false,
            editor_layer: 0,
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PlungerJson {
    center: Vertex2D,
    width: f32,
    height: f32,
    z_adjust: f32,
    stroke: f32,
    speed_pull: f32,
    speed_fire: f32,
    plunger_type: PlungerType,
    anim_frames: u32,
    material: String,
    image: String,
    mech_strength: f32,
    is_mech_plunger: bool,
    auto_plunger: bool,
    park_position: f32,
    scatter_velocity: f32,
    momentum_xfer: f32,
    is_timer_enabled: bool,
    timer_interval: u32,
    is_visible: bool,
    is_reflection_enabled: Option<bool>,
    surface: String,
    name: String,
    tip_shape: String,
    rod_diam: f32,
    ring_gap: f32,
    ring_diam: f32,
    ring_width: f32,
    spring_diam: f32,
    spring_gauge: f32,
    spring_loops: f32,
    spring_end_loops: f32,
}

impl PlungerJson {
    pub fn from_plunger(plunger: &Plunger) -> Self {
        Self {
            center: plunger.center,
            width: plunger.width,
            height: plunger.height,
            z_adjust: plunger.z_adjust,
            stroke: plunger.stroke,
            speed_pull: plunger.speed_pull,
            speed_fire: plunger.speed_fire,
            plunger_type: plunger.plunger_type.clone(),
            anim_frames: plunger.anim_frames,
            material: plunger.material.clone(),
            image: plunger.image.clone(),
            mech_strength: plunger.mech_strength,
            is_mech_plunger: plunger.is_mech_plunger,
            auto_plunger: plunger.auto_plunger,
            park_position: plunger.park_position,
            scatter_velocity: plunger.scatter_velocity,
            momentum_xfer: plunger.momentum_xfer,
            is_timer_enabled: plunger.is_timer_enabled,
            timer_interval: plunger.timer_interval,
            is_visible: plunger.is_visible,
            is_reflection_enabled: plunger.is_reflection_enabled,
            surface: plunger.surface.clone(),
            name: plunger.name.clone(),
            tip_shape: plunger.tip_shape.clone(),
            rod_diam: plunger.rod_diam,
            ring_gap: plunger.ring_gap,
            ring_diam: plunger.ring_diam,
            ring_width: plunger.ring_width,
            spring_diam: plunger.spring_diam,
            spring_gauge: plunger.spring_gauge,
            spring_loops: plunger.spring_loops,
            spring_end_loops: plunger.spring_end_loops,
        }
    }

    pub fn to_plunger(&self) -> Plunger {
        Plunger {
            center: self.center,
            width: self.width,
            height: self.height,
            z_adjust: self.z_adjust,
            stroke: self.stroke,
            speed_pull: self.speed_pull,
            speed_fire: self.speed_fire,
            plunger_type: self.plunger_type.clone(),
            anim_frames: self.anim_frames,
            material: self.material.clone(),
            image: self.image.clone(),
            mech_strength: self.mech_strength,
            is_mech_plunger: self.is_mech_plunger,
            auto_plunger: self.auto_plunger,
            park_position: self.park_position,
            scatter_velocity: self.scatter_velocity,
            momentum_xfer: self.momentum_xfer,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            is_visible: self.is_visible,
            is_reflection_enabled: self.is_reflection_enabled,
            surface: self.surface.clone(),
            name: self.name.clone(),
            tip_shape: self.tip_shape.clone(),
            rod_diam: self.rod_diam,
            ring_gap: self.ring_gap,
            ring_diam: self.ring_diam,
            ring_width: self.ring_width,
            spring_diam: self.spring_diam,
            spring_gauge: self.spring_gauge,
            spring_loops: self.spring_loops,
            spring_end_loops: self.spring_end_loops,
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

impl Serialize for Plunger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PlungerJson::from_plunger(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Plunger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = PlungerJson::deserialize(deserializer)?;
        Ok(json.to_plunger())
    }
}

impl BiffRead for Plunger {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut plunger = Plunger::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    plunger.center = Vertex2D::biff_read(reader);
                }
                "WDTH" => {
                    plunger.width = reader.get_f32();
                }
                "HIGH" => {
                    plunger.height = reader.get_f32();
                }
                "ZADJ" => {
                    plunger.z_adjust = reader.get_f32();
                }
                "HPSL" => {
                    plunger.stroke = reader.get_f32();
                }
                "SPDP" => {
                    plunger.speed_pull = reader.get_f32();
                }
                "SPDF" => {
                    plunger.speed_fire = reader.get_f32();
                }
                "TYPE" => {
                    plunger.plunger_type = reader.get_u32().into();
                }
                "ANFR" => {
                    plunger.anim_frames = reader.get_u32();
                }
                "MATR" => {
                    plunger.material = reader.get_string();
                }
                "IMAG" => {
                    plunger.image = reader.get_string();
                }
                "MEST" => {
                    plunger.mech_strength = reader.get_f32();
                }
                "MECH" => {
                    plunger.is_mech_plunger = reader.get_bool();
                }
                "APLG" => {
                    plunger.auto_plunger = reader.get_bool();
                }
                "MPRK" => {
                    plunger.park_position = reader.get_f32();
                }
                "PSCV" => {
                    plunger.scatter_velocity = reader.get_f32();
                }
                "MOMX" => {
                    plunger.momentum_xfer = reader.get_f32();
                }
                "TMON" => {
                    plunger.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    plunger.timer_interval = reader.get_u32();
                }
                "VSBL" => {
                    plunger.is_visible = reader.get_bool();
                }
                "REEN" => {
                    plunger.is_reflection_enabled = Some(reader.get_bool());
                }
                "SURF" => {
                    plunger.surface = reader.get_string();
                }
                "NAME" => {
                    plunger.name = reader.get_wide_string();
                }
                "TIPS" => {
                    plunger.tip_shape = reader.get_string();
                }
                "RODD" => {
                    plunger.rod_diam = reader.get_f32();
                }
                "RNGG" => {
                    plunger.ring_gap = reader.get_f32();
                }
                "RNGD" => {
                    plunger.ring_diam = reader.get_f32();
                }
                "RNGW" => {
                    plunger.ring_width = reader.get_f32();
                }
                "SPRD" => {
                    plunger.spring_diam = reader.get_f32();
                }
                "SPRG" => {
                    plunger.spring_gauge = reader.get_f32();
                }
                "SPRL" => {
                    plunger.spring_loops = reader.get_f32();
                }
                "SPRE" => {
                    plunger.spring_end_loops = reader.get_f32();
                }

                // shared
                "LOCK" => {
                    plunger.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    plunger.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    plunger.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    plunger.editor_layer_visibility = Some(reader.get_bool());
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
        plunger
    }
}

impl BiffWrite for Plunger {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("WDTH", self.width);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_f32("ZADJ", self.z_adjust);
        writer.write_tagged_f32("HPSL", self.stroke);
        writer.write_tagged_f32("SPDP", self.speed_pull);
        writer.write_tagged_f32("SPDF", self.speed_fire);
        writer.write_tagged_u32("TYPE", (&self.plunger_type).into());
        writer.write_tagged_u32("ANFR", self.anim_frames);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_f32("MEST", self.mech_strength);
        writer.write_tagged_bool("MECH", self.is_mech_plunger);
        writer.write_tagged_bool("APLG", self.auto_plunger);
        writer.write_tagged_f32("MPRK", self.park_position);
        writer.write_tagged_f32("PSCV", self.scatter_velocity);
        writer.write_tagged_f32("MOMX", self.momentum_xfer);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        writer.write_tagged_bool("VSBL", self.is_visible);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("TIPS", &self.tip_shape);
        writer.write_tagged_f32("RODD", self.rod_diam);
        writer.write_tagged_f32("RNGG", self.ring_gap);
        writer.write_tagged_f32("RNGD", self.ring_diam);
        writer.write_tagged_f32("RNGW", self.ring_width);
        writer.write_tagged_f32("SPRD", self.spring_diam);
        writer.write_tagged_f32("SPRG", self.spring_gauge);
        writer.write_tagged_f32("SPRL", self.spring_loops);
        writer.write_tagged_f32("SPRE", self.spring_end_loops);
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
        let plunger = Plunger {
            center: Vertex2D::new(1.0, 2.0),
            width: 1.0,
            height: 1.0,
            z_adjust: 0.1,
            stroke: 2.0,
            speed_pull: 0.5,
            speed_fire: 3.0,
            plunger_type: Faker.fake(),
            anim_frames: 1,
            material: "test material".to_string(),
            image: "test image".to_string(),
            mech_strength: 85.0,
            is_mech_plunger: false,
            auto_plunger: false,
            park_position: 0.5 / 3.0,
            scatter_velocity: 0.0,
            momentum_xfer: 1.0,
            is_timer_enabled: false,
            timer_interval: 0,
            is_visible: true,
            is_reflection_enabled: Some(true),
            surface: "test surface".to_string(),
            name: "test plunger".to_string(),
            tip_shape: "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .83"
                .to_string(),
            rod_diam: 0.6,
            ring_gap: 2.0,
            ring_diam: 0.94,
            ring_width: 3.0,
            spring_diam: 0.77,
            spring_gauge: 1.38,
            spring_loops: 8.0,
            spring_end_loops: 2.5,
            is_locked: true,
            editor_layer: 0,
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(false),
        };
        let mut writer = BiffWriter::new();
        Plunger::biff_write(&plunger, &mut writer);
        let plunger_read = Plunger::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(plunger, plunger_read);
    }

    #[test]
    fn test_plunger_type_json() {
        let sizing_type = PlungerType::Modern;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"modern\"");
        let sizing_type_read: PlungerType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(3);
        let sizing_type_read: PlungerType = serde_json::from_value(json).unwrap();
        assert_eq!(PlungerType::Custom, sizing_type_read);
    }

    #[test]
    #[should_panic = " Error(\"unknown variant `foo`, expected one of `modern`, `flat`, `custom`\", line: 0, column: 0)"]
    fn test_plunger_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: PlungerType = serde_json::from_value(json).unwrap();
    }
}
