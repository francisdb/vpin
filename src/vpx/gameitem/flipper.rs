use super::{GameItem, vertex2d::Vertex2D};
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::{HasSharedAttributes, WriteSharedAttributes};
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Clone, Dummy)]
pub struct Flipper {
    pub center: Vertex2D,
    base_radius: f32,
    end_radius: f32,
    flipper_radius_max: f32,
    return_: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    override_physics: u32,
    mass: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    surface: String,
    material: String,
    pub name: String,
    rubber_material: String,
    rubber_thickness_int: u32,     // RTHK deprecated
    rubber_thickness: Option<f32>, // RTHF (added in 10.?)
    rubber_height_int: u32,        // RHGT deprecated
    rubber_height: Option<f32>,    // RHGF (added in 10.?)
    rubber_width_int: u32,         // RWDT deprecated
    rubber_width: Option<f32>,     // RHGF (added in 10.?)
    strength: f32,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    ramp_up: f32,
    scatter: Option<f32>,
    // SCTR (added in 10.?)
    torque_damping: Option<f32>,
    // TODA (added in 10.?)
    torque_damping_angle: Option<f32>,
    // TDAA (added in 10.?)
    flipper_radius_min: f32,
    is_visible: bool,
    is_enabled: bool,
    height: f32,
    image: Option<String>,               // IMAG (was missing in 10.01)
    is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FlipperJson {
    center: Vertex2D,
    base_radius: f32,
    end_radius: f32,
    flipper_radius_max: f32,
    return_: f32,
    start_angle: f32,
    end_angle: f32,
    override_physics: u32,
    mass: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    surface: String,
    material: String,
    name: String,
    rubber_material: String,
    rubber_thickness_int: u32,
    rubber_thickness: Option<f32>,
    rubber_height_int: u32,
    rubber_height: Option<f32>,
    rubber_width_int: u32,
    rubber_width: Option<f32>,
    strength: f32,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    ramp_up: f32,
    scatter: Option<f32>,
    torque_damping: Option<f32>,
    torque_damping_angle: Option<f32>,
    flipper_radius_min: f32,
    is_visible: bool,
    is_enabled: bool,
    height: f32,
    image: Option<String>,
    is_reflection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl FlipperJson {
    pub fn from_flipper(flipper: &Flipper) -> Self {
        Self {
            center: flipper.center,
            base_radius: flipper.base_radius,
            end_radius: flipper.end_radius,
            flipper_radius_max: flipper.flipper_radius_max,
            return_: flipper.return_,
            start_angle: flipper.start_angle,
            end_angle: flipper.end_angle,
            override_physics: flipper.override_physics,
            mass: flipper.mass,
            is_timer_enabled: flipper.is_timer_enabled,
            timer_interval: flipper.timer_interval,
            surface: flipper.surface.clone(),
            material: flipper.material.clone(),
            name: flipper.name.clone(),
            rubber_material: flipper.rubber_material.clone(),
            rubber_thickness_int: flipper.rubber_thickness_int,
            rubber_thickness: flipper.rubber_thickness,
            rubber_height_int: flipper.rubber_height_int,
            rubber_height: flipper.rubber_height,
            rubber_width_int: flipper.rubber_width_int,
            rubber_width: flipper.rubber_width,
            strength: flipper.strength,
            elasticity: flipper.elasticity,
            elasticity_falloff: flipper.elasticity_falloff,
            friction: flipper.friction,
            ramp_up: flipper.ramp_up,
            scatter: flipper.scatter,
            torque_damping: flipper.torque_damping,
            torque_damping_angle: flipper.torque_damping_angle,
            flipper_radius_min: flipper.flipper_radius_min,
            is_visible: flipper.is_visible,
            is_enabled: flipper.is_enabled,
            height: flipper.height,
            image: flipper.image.clone(),
            is_reflection_enabled: flipper.is_reflection_enabled,
            part_group_name: flipper.part_group_name.clone(),
        }
    }

    pub fn to_flipper(&self) -> Flipper {
        Flipper {
            center: self.center,
            base_radius: self.base_radius,
            end_radius: self.end_radius,
            flipper_radius_max: self.flipper_radius_max,
            return_: self.return_,
            start_angle: self.start_angle,
            end_angle: self.end_angle,
            override_physics: self.override_physics,
            mass: self.mass,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            surface: self.surface.clone(),
            material: self.material.clone(),
            name: self.name.clone(),
            rubber_material: self.rubber_material.clone(),
            rubber_thickness_int: self.rubber_thickness_int,
            rubber_thickness: self.rubber_thickness,
            rubber_height_int: self.rubber_height_int,
            rubber_height: self.rubber_height,
            rubber_width_int: self.rubber_width_int,
            rubber_width: self.rubber_width,
            strength: self.strength,
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            ramp_up: self.ramp_up,
            scatter: self.scatter,
            torque_damping: self.torque_damping,
            torque_damping_angle: self.torque_damping_angle,
            flipper_radius_min: self.flipper_radius_min,
            is_visible: self.is_visible,
            is_enabled: self.is_enabled,
            height: self.height,
            image: self.image.clone(),
            is_reflection_enabled: self.is_reflection_enabled,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
        }
    }
}

impl Serialize for Flipper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FlipperJson::from_flipper(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Flipper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let flipper_json = FlipperJson::deserialize(deserializer)?;
        Ok(flipper_json.to_flipper())
    }
}

impl GameItem for Flipper {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Default for Flipper {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            base_radius: 21.5,
            end_radius: 13.0,
            flipper_radius_max: 130.0,
            return_: 0.058,
            start_angle: 121.0,
            end_angle: 70.0,
            override_physics: 0,
            mass: 1.0,
            is_timer_enabled: false,
            timer_interval: 0,
            surface: String::default(),
            material: String::default(),
            name: String::default(),
            rubber_material: String::default(),
            rubber_thickness_int: 0,
            rubber_thickness: None,
            rubber_height_int: 0,
            rubber_height: None,
            rubber_width_int: 0,
            rubber_width: None,
            strength: 2200.0,
            elasticity: 0.8,
            elasticity_falloff: 0.43,
            friction: 0.6,
            ramp_up: 3.0,
            scatter: None,
            torque_damping: None,
            torque_damping_angle: None,
            flipper_radius_min: 0.0,
            is_visible: true,
            is_enabled: true,
            height: 50.0,
            image: None,
            is_reflection_enabled: None, // true,
            is_locked: false,
            editor_layer: 0,
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl HasSharedAttributes for Flipper {
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

impl BiffRead for Flipper {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut flipper = Flipper::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    flipper.center = Vertex2D::biff_read(reader);
                }
                "BASR" => {
                    flipper.base_radius = reader.get_f32();
                }
                "ENDR" => {
                    flipper.end_radius = reader.get_f32();
                }
                "FLPR" => {
                    flipper.flipper_radius_max = reader.get_f32();
                }
                "FRTN" => {
                    flipper.return_ = reader.get_f32();
                }
                "ANGS" => {
                    flipper.start_angle = reader.get_f32();
                }
                "ANGE" => {
                    flipper.end_angle = reader.get_f32();
                }
                "OVRP" => {
                    flipper.override_physics = reader.get_u32();
                }
                "FORC" => {
                    flipper.mass = reader.get_f32();
                }
                "TMON" => {
                    flipper.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    flipper.timer_interval = reader.get_i32();
                }
                "SURF" => {
                    flipper.surface = reader.get_string();
                }
                "MATR" => {
                    flipper.material = reader.get_string();
                }
                "NAME" => {
                    flipper.name = reader.get_wide_string();
                }
                "RUMA" => {
                    flipper.rubber_material = reader.get_string();
                }
                "RTHK" => {
                    flipper.rubber_thickness_int = reader.get_u32();
                }
                "RTHF" => {
                    flipper.rubber_thickness = Some(reader.get_f32());
                }
                "RHGT" => {
                    flipper.rubber_height_int = reader.get_u32();
                }
                "RHGF" => {
                    flipper.rubber_height = Some(reader.get_f32());
                }
                "RWDT" => {
                    flipper.rubber_width_int = reader.get_u32();
                }
                "RWDF" => {
                    flipper.rubber_width = Some(reader.get_f32());
                }
                "STRG" => {
                    flipper.strength = reader.get_f32();
                }
                "ELAS" => {
                    flipper.elasticity = reader.get_f32();
                }
                "ELFO" => {
                    flipper.elasticity_falloff = reader.get_f32();
                }
                "FRIC" => {
                    flipper.friction = reader.get_f32();
                }
                "RPUP" => {
                    flipper.ramp_up = reader.get_f32();
                }
                "SCTR" => {
                    flipper.scatter = Some(reader.get_f32());
                }
                "TODA" => {
                    flipper.torque_damping = Some(reader.get_f32());
                }
                "TDAA" => {
                    flipper.torque_damping_angle = Some(reader.get_f32());
                }
                "VSBL" => {
                    flipper.is_visible = reader.get_bool();
                }
                "ENBL" => {
                    flipper.is_enabled = reader.get_bool();
                }
                "FRMN" => {
                    flipper.flipper_radius_min = reader.get_f32();
                }
                "FHGT" => {
                    flipper.height = reader.get_f32();
                }
                "IMAG" => {
                    flipper.image = Some(reader.get_string());
                }
                "REEN" => {
                    flipper.is_reflection_enabled = Some(reader.get_bool());
                }

                // shared
                "LOCK" => {
                    flipper.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    flipper.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    flipper.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    flipper.editor_layer_visibility = Some(reader.get_bool());
                }
                "GRUP" => {
                    flipper.part_group_name = Some(reader.get_string());
                }
                _ => {
                    warn!(
                        "Unknown tag {} for {}",
                        tag_str,
                        std::any::type_name::<Self>()
                    );
                    reader.skip_tag();
                }
            }
        }
        flipper
    }
}

impl BiffWrite for Flipper {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("BASR", self.base_radius);
        writer.write_tagged_f32("ENDR", self.end_radius);
        writer.write_tagged_f32("FLPR", self.flipper_radius_max);
        writer.write_tagged_f32("FRTN", self.return_);
        writer.write_tagged_f32("ANGS", self.start_angle);
        writer.write_tagged_f32("ANGE", self.end_angle);
        writer.write_tagged_u32("OVRP", self.override_physics);
        writer.write_tagged_f32("FORC", self.mass);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("RUMA", &self.rubber_material);
        writer.write_tagged_u32("RTHK", self.rubber_thickness_int);
        if let Some(rubber_thickness) = self.rubber_thickness {
            writer.write_tagged_f32("RTHF", rubber_thickness);
        }
        writer.write_tagged_u32("RHGT", self.rubber_height_int);
        if let Some(rubber_height) = self.rubber_height {
            writer.write_tagged_f32("RHGF", rubber_height);
        }
        writer.write_tagged_u32("RWDT", self.rubber_width_int);
        if let Some(rubber_width) = self.rubber_width {
            writer.write_tagged_f32("RWDF", rubber_width);
        }
        writer.write_tagged_f32("STRG", self.strength);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("FRIC", self.friction);
        writer.write_tagged_f32("RPUP", self.ramp_up);
        if let Some(sctr) = self.scatter {
            writer.write_tagged_f32("SCTR", sctr);
        }
        if let Some(toda) = self.torque_damping {
            writer.write_tagged_f32("TODA", toda);
        }
        if let Some(tdaa) = self.torque_damping_angle {
            writer.write_tagged_f32("TDAA", tdaa);
        }
        writer.write_tagged_bool("VSBL", self.is_visible);
        writer.write_tagged_bool("ENBL", self.is_enabled);
        writer.write_tagged_f32("FRMN", self.flipper_radius_min);
        writer.write_tagged_f32("FHGT", self.height);
        if let Some(image) = &self.image {
            writer.write_tagged_string("IMAG", image);
        }
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }

        self.write_shared_attributes(writer);

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
        let flipper = Flipper {
            center: Vertex2D::new(0.0, 0.0),
            base_radius: 21.5,
            end_radius: 13.0,
            flipper_radius_max: 130.0,
            return_: 0.058,
            start_angle: 121.0,
            end_angle: 70.0,
            override_physics: 0,
            mass: 1.0,
            is_timer_enabled: false,
            timer_interval: 0,
            surface: String::from("test surface"),
            material: String::from("test material"),
            name: String::from("test name"),
            rubber_material: String::from("test rubber material"),
            rubber_thickness_int: 0,
            rubber_thickness: Some(7.0),
            rubber_height_int: 0,
            rubber_height: Some(19.0),
            rubber_width_int: 0,
            rubber_width: Some(24.0),
            strength: 2200.0,
            elasticity: 0.8,
            elasticity_falloff: 0.43,
            friction: 0.6,
            ramp_up: 3.0,
            scatter: Some(0.0),
            torque_damping: Some(0.75),
            torque_damping_angle: Some(6.0),
            flipper_radius_min: 0.0,
            is_visible: true,
            is_enabled: true,
            height: 50.0,
            image: Some(String::from("test image")),
            is_reflection_enabled: Some(true),
            is_locked: false,
            editor_layer: 123,
            editor_layer_name: Some(String::from("test editor layer name")),
            editor_layer_visibility: Some(true),
            part_group_name: Some(String::from("test part group name")),
        };
        let mut writer = BiffWriter::new();
        Flipper::biff_write(&flipper, &mut writer);
        let flipper_read = Flipper::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(flipper, flipper_read);
    }
}
