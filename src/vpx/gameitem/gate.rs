use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum GateType {
    WireW = 1,
    WireRectangle = 2,
    Plate = 3,
    LongPlate = 4,
}

impl From<u32> for GateType {
    fn from(value: u32) -> Self {
        match value {
            1 => GateType::WireW,
            2 => GateType::WireRectangle,
            3 => GateType::Plate,
            4 => GateType::LongPlate,
            _ => panic!("Unknown GateType: {}", value),
        }
    }
}

impl From<GateType> for u32 {
    fn from(gate_type: GateType) -> Self {
        match gate_type {
            GateType::WireW => 1,
            GateType::WireRectangle => 2,
            GateType::Plate => 3,
            GateType::LongPlate => 4,
        }
    }
}

impl Serialize for GateType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            GateType::WireW => serializer.serialize_str("wire_w"),
            GateType::WireRectangle => serializer.serialize_str("wire_rectangle"),
            GateType::Plate => serializer.serialize_str("plate"),
            GateType::LongPlate => serializer.serialize_str("long_plate"),
        }
    }
}

impl<'de> Deserialize<'de> for GateType {
    fn deserialize<D>(deserializer: D) -> Result<GateType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let s = value.as_str();
        match s {
            "wire_w" => Ok(GateType::WireW),
            "wire_rectangle" => Ok(GateType::WireRectangle),
            "plate" => Ok(GateType::Plate),
            "long_plate" => Ok(GateType::LongPlate),
            _ => Err(serde::de::Error::unknown_variant(
                s,
                &["wire_w", "wire_rectangle", "plate", "long_plate"],
            )),
        }
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Gate {
    pub center: Vertex2D,                    // 1 VCEN
    pub length: f32,                         // 2 LGTH
    pub height: f32,                         // 3 HGTH
    pub rotation: f32,                       // 4 ROTA
    pub material: String,                    // 5 MATR
    pub is_timer_enabled: bool,              // 6 TMON
    pub show_bracket: bool,                  // 7 GSUP
    pub is_collidable: bool,                 // 8 GCOL
    pub timer_interval: u32,                 // 9 TMIN
    pub imgf: Option<String>,                // IMGF (was in use in 10.01)
    pub imgb: Option<String>,                // IMGB (was in use in 10.01)
    pub surface: String,                     // 10 SURF
    pub elasticity: f32,                     // 11 ELAS
    pub angle_max: f32,                      // 12 GAMA
    pub angle_min: f32,                      // 13 GAMI
    pub friction: f32,                       // 14 GFRC
    pub damping: Option<f32>,                // 15 AFRC (added in 10.?)
    pub gravity_factor: Option<f32>,         // 16 GGFC (added in 10.?)
    pub is_visible: bool,                    // 17 GVSB
    pub name: String,                        // 18 NAME
    pub two_way: bool,                       // 19 TWWA
    pub is_reflection_enabled: Option<bool>, // 20 REEN (was missing in 10.01)
    pub gate_type: Option<GateType>,         // 21 GATY (was missing in 10.01)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            center: Default::default(),
            length: 100.0,
            height: 50.0,
            rotation: -90.0,
            material: Default::default(),
            is_timer_enabled: false,
            show_bracket: true,
            is_collidable: true,
            timer_interval: Default::default(),
            imgf: None,
            imgb: None,
            surface: Default::default(),
            elasticity: 0.3,
            angle_max: std::f32::consts::PI / 2.0,
            angle_min: Default::default(),
            friction: 0.02,
            damping: None,        //0.985,
            gravity_factor: None, //0.25,
            is_visible: true,
            name: Default::default(),
            two_way: false,
            is_reflection_enabled: None, //true,
            gate_type: None,             //Some(GateType::Plate),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct GateJson {
    center: Vertex2D,
    length: f32,
    height: f32,
    rotation: f32,
    material: String,
    is_timer_enabled: bool,
    show_bracket: bool,
    is_collidable: bool,
    timer_interval: u32,
    imgf: Option<String>,
    imgb: Option<String>,
    surface: String,
    elasticity: f32,
    angle_max: f32,
    angle_min: f32,
    friction: f32,
    damping: Option<f32>,
    gravity_factor: Option<f32>,
    is_visible: bool,
    name: String,
    two_way: bool,
    is_reflection_enabled: Option<bool>,
    gate_type: Option<GateType>,
}

impl GateJson {
    pub fn from_gate(gate: &Gate) -> Self {
        Self {
            center: gate.center,
            length: gate.length,
            height: gate.height,
            rotation: gate.rotation,
            material: gate.material.clone(),
            is_timer_enabled: gate.is_timer_enabled,
            show_bracket: gate.show_bracket,
            is_collidable: gate.is_collidable,
            timer_interval: gate.timer_interval,
            imgf: gate.imgf.clone(),
            imgb: gate.imgb.clone(),
            surface: gate.surface.clone(),
            elasticity: gate.elasticity,
            angle_max: gate.angle_max,
            angle_min: gate.angle_min,
            friction: gate.friction,
            damping: gate.damping,
            gravity_factor: gate.gravity_factor,
            is_visible: gate.is_visible,
            name: gate.name.clone(),
            two_way: gate.two_way,
            is_reflection_enabled: gate.is_reflection_enabled,
            gate_type: gate.gate_type.clone(),
        }
    }
    pub fn to_gate(&self) -> Gate {
        Gate {
            center: self.center,
            length: self.length,
            height: self.height,
            rotation: self.rotation,
            material: self.material.clone(),
            is_timer_enabled: self.is_timer_enabled,
            show_bracket: self.show_bracket,
            is_collidable: self.is_collidable,
            timer_interval: self.timer_interval,
            imgf: self.imgf.clone(),
            imgb: self.imgb.clone(),
            surface: self.surface.clone(),
            elasticity: self.elasticity,
            angle_max: self.angle_max,
            angle_min: self.angle_min,
            friction: self.friction,
            damping: self.damping,
            gravity_factor: self.gravity_factor,
            is_visible: self.is_visible,
            name: self.name.clone(),
            two_way: self.two_way,
            is_reflection_enabled: self.is_reflection_enabled,
            gate_type: self.gate_type.clone(),
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

impl Serialize for Gate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        GateJson::from_gate(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Gate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let gate_json = GateJson::deserialize(deserializer)?;
        Ok(gate_json.to_gate())
    }
}

impl BiffRead for Gate {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut gate = Gate::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    gate.center = Vertex2D::biff_read(reader);
                }
                "LGTH" => {
                    gate.length = reader.get_f32();
                }
                "HGTH" => {
                    gate.height = reader.get_f32();
                }
                "ROTA" => {
                    gate.rotation = reader.get_f32();
                }
                "MATR" => {
                    gate.material = reader.get_string();
                }
                "TMON" => {
                    gate.is_timer_enabled = reader.get_bool();
                }
                "GSUP" => {
                    gate.show_bracket = reader.get_bool();
                }
                "GCOL" => {
                    gate.is_collidable = reader.get_bool();
                }
                "TMIN" => {
                    gate.timer_interval = reader.get_u32();
                }
                "IMGF" => {
                    gate.imgf = Some(reader.get_string());
                }
                "IMGB" => {
                    gate.imgb = Some(reader.get_string());
                }
                "SURF" => {
                    gate.surface = reader.get_string();
                }
                "ELAS" => {
                    gate.elasticity = reader.get_f32();
                }
                "GAMA" => {
                    gate.angle_max = reader.get_f32();
                }
                "GAMI" => {
                    gate.angle_min = reader.get_f32();
                }
                "GFRC" => {
                    gate.friction = reader.get_f32();
                }
                "AFRC" => {
                    gate.damping = Some(reader.get_f32());
                }
                "GGFC" => {
                    gate.gravity_factor = Some(reader.get_f32());
                }
                "GVSB" => {
                    gate.is_visible = reader.get_bool();
                }
                "NAME" => {
                    gate.name = reader.get_wide_string();
                }
                "TWWA" => {
                    gate.two_way = reader.get_bool();
                }
                "REEN" => {
                    gate.is_reflection_enabled = Some(reader.get_bool());
                }
                "GATY" => {
                    gate.gate_type = Some(reader.get_u32().into());
                }

                // shared
                "LOCK" => {
                    gate.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    gate.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    gate.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    gate.editor_layer_visibility = Some(reader.get_bool());
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
        gate
    }
}

impl BiffWrite for Gate {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("LGTH", self.length);
        writer.write_tagged_f32("HGTH", self.height);
        writer.write_tagged_f32("ROTA", self.rotation);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_bool("GSUP", self.show_bracket);
        writer.write_tagged_bool("GCOL", self.is_collidable);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        if let Some(imgf) = &self.imgf {
            writer.write_tagged_string("IMGF", imgf);
        }
        if let Some(imgb) = &self.imgb {
            writer.write_tagged_string("IMGB", imgb);
        }
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("GAMA", self.angle_max);
        writer.write_tagged_f32("GAMI", self.angle_min);
        writer.write_tagged_f32("GFRC", self.friction);
        if let Some(damping) = self.damping {
            writer.write_tagged_f32("AFRC", damping);
        }
        if let Some(gravity_factor) = self.gravity_factor {
            writer.write_tagged_f32("GGFC", gravity_factor);
        }
        writer.write_tagged_bool("GVSB", self.is_visible);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_bool("TWWA", self.two_way);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        };
        if let Some(gate_type) = &self.gate_type {
            writer.write_tagged_u32("GATY", gate_type.clone().into());
        };

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

    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let gate = Gate {
            center: Vertex2D::new(1.0, 2.0),
            length: 3.0,
            height: 4.0,
            rotation: 5.0,
            material: "material".to_string(),
            is_timer_enabled: true,
            show_bracket: false,
            is_collidable: false,
            timer_interval: 6,
            imgf: Some("imgf".to_string()),
            imgb: Some("imgb".to_string()),
            surface: "surface".to_string(),
            elasticity: 7.0,
            angle_max: 8.0,
            angle_min: 9.0,
            friction: 10.0,
            damping: Some(11.0),
            gravity_factor: Some(12.0),
            is_visible: false,
            name: "name".to_string(),
            two_way: true,
            is_reflection_enabled: Some(false),
            gate_type: Some(GateType::Plate),
            is_locked: true,
            editor_layer: 14,
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: Some(false),
        };
        let mut writer = BiffWriter::new();
        Gate::biff_write(&gate, &mut writer);
        let gate_read = Gate::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(gate, gate_read);
    }

    #[test]
    fn test_gate_type_json() {
        let gate_type = GateType::WireRectangle;
        let json = serde_json::to_string(&gate_type).unwrap();
        assert_eq!(json, "\"wire_rectangle\"");
        let gate_type_read: GateType = serde_json::from_str(&json).unwrap();
        assert_eq!(gate_type, gate_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `Unknown`, expected one of `wire_w`, `wire_rectangle`, `plate`, `long_plate`\", line: 0, column: 0)"]
    fn test_gate_type_json_panic() {
        let json = Value::from("Unknown");
        let _: GateType = serde_json::from_value(json).unwrap();
    }
}
