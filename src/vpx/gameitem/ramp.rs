use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::dragpoint::DragPoint;

#[derive(Debug, PartialEq, Dummy)]
pub struct Ramp {
    pub height_bottom: f32,                  // 1
    pub height_top: f32,                     // 2
    pub width_bottom: f32,                   // 3
    pub width_top: f32,                      // 4
    pub material: String,                    // 5
    pub is_timer_enabled: bool,              // 6
    pub timer_interval: u32,                 // 7
    pub ramp_type: u32,                      // 8
    pub name: String,                        // 9
    pub image: String,                       // 10
    pub image_alignment: u32,                // 11
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
    ramp_type: u32,
    name: String,
    image: String,
    image_alignment: u32,
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

    // these are shared between all items
    is_locked: bool,
    editor_layer: u32,
    editor_layer_name: Option<String>,
    editor_layer_visibility: Option<bool>,
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
            ramp_type: ramp.ramp_type,
            name: ramp.name.clone(),
            image: ramp.image.clone(),
            image_alignment: ramp.image_alignment,
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
            is_locked: ramp.is_locked,
            editor_layer: ramp.editor_layer,
            editor_layer_name: ramp.editor_layer_name.clone(),
            editor_layer_visibility: ramp.editor_layer_visibility,
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
            ramp_type: self.ramp_type,
            name: self.name.clone(),
            image: self.image.clone(),
            image_alignment: self.image_alignment,
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
            is_locked: self.is_locked,
            editor_layer: self.editor_layer,
            editor_layer_name: self.editor_layer_name.clone(),
            editor_layer_visibility: self.editor_layer_visibility,
        }
    }
}

impl Ramp {
    pub const RAMP_IMAGE_ALIGNMENT_MODE_WORLD: u32 = 0;
    pub const RAMP_IMAGE_ALIGNMENT_MODE_WRAP: u32 = 1;

    pub const RAMP_TYPE_FLAT: u32 = 0;
    pub const RAMP_TYPE_4_WIRE: u32 = 1;
    pub const RAMP_TYPE_2_WIRE: u32 = 2;
    pub const RAMP_TYPE_3_WIRE_LEFT: u32 = 3;
    pub const RAMP_TYPE_3_WIRE_RIGHT: u32 = 4;
    pub const RAMP_TYPE_1_WIRE: u32 = 5;
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
            ramp_type: Ramp::RAMP_TYPE_FLAT,
            name: Default::default(),
            image: Default::default(),
            image_alignment: Ramp::RAMP_IMAGE_ALIGNMENT_MODE_WORLD,
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
                    ramp.ramp_type = reader.get_u32();
                }
                "NAME" => {
                    ramp.name = reader.get_wide_string();
                }
                "IMAG" => {
                    ramp.image = reader.get_string();
                }
                "ALGN" => {
                    ramp.image_alignment = reader.get_u32();
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
                "RAMP" => {
                    ramp.ramp_type = reader.get_u32();
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
        writer.write_tagged_u32("TYPE", self.ramp_type);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_u32("ALGN", self.image_alignment);
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
            ramp_type: 6,
            name: "name".to_string(),
            image: "image".to_string(),
            image_alignment: 7,
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
}
