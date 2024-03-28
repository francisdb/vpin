use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::dragpoint::DragPoint;

#[derive(Debug, PartialEq, Dummy)]
pub struct Rubber {
    pub height: f32,
    pub hit_height: Option<f32>, // HTHI (added in 10.?)
    pub thickness: i32,
    pub hit_event: bool,
    pub material: String,
    pub is_timer_enabled: bool,
    pub timer_interval: i32,
    pub name: String,
    pub image: String,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    pub scatter: f32,
    pub is_collidable: bool,
    pub is_visible: bool,
    pub radb: Option<f32>, // RADB (was used in 10.01)
    pub static_rendering: bool,
    pub show_in_editor: bool,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)
    pub physics_material: Option<String>,    // MAPH (added in 10.?)
    pub overwrite_physics: Option<bool>,     // OVPH (added in 10.?)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,

    drag_points: Vec<DragPoint>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RubberJson {
    height: f32,
    hit_height: Option<f32>,
    thickness: i32,
    hit_event: bool,
    material: String,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    image: String,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter: f32,
    is_collidable: bool,
    is_visible: bool,
    radb: Option<f32>,
    static_rendering: bool,
    show_in_editor: bool,
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    is_reflection_enabled: Option<bool>,
    physics_material: Option<String>,
    overwrite_physics: Option<bool>,
    drag_points: Vec<DragPoint>,
}

impl RubberJson {
    fn from_rubber(rubber: &Rubber) -> Self {
        RubberJson {
            height: rubber.height,
            hit_height: rubber.hit_height,
            thickness: rubber.thickness,
            hit_event: rubber.hit_event,
            material: rubber.material.clone(),
            is_timer_enabled: rubber.is_timer_enabled,
            timer_interval: rubber.timer_interval,
            name: rubber.name.clone(),
            image: rubber.image.clone(),
            elasticity: rubber.elasticity,
            elasticity_falloff: rubber.elasticity_falloff,
            friction: rubber.friction,
            scatter: rubber.scatter,
            is_collidable: rubber.is_collidable,
            is_visible: rubber.is_visible,
            radb: rubber.radb,
            static_rendering: rubber.static_rendering,
            show_in_editor: rubber.show_in_editor,
            rot_x: rubber.rot_x,
            rot_y: rubber.rot_y,
            rot_z: rubber.rot_z,
            is_reflection_enabled: rubber.is_reflection_enabled,
            physics_material: rubber.physics_material.clone(),
            overwrite_physics: rubber.overwrite_physics,
            drag_points: rubber.drag_points.clone(),
        }
    }

    fn to_rubber(&self) -> Rubber {
        Rubber {
            height: self.height,
            hit_height: self.hit_height,
            thickness: self.thickness,
            hit_event: self.hit_event,
            material: self.material.clone(),
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            name: self.name.clone(),
            image: self.image.clone(),
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            scatter: self.scatter,
            is_collidable: self.is_collidable,
            is_visible: self.is_visible,
            radb: self.radb,
            static_rendering: self.static_rendering,
            show_in_editor: self.show_in_editor,
            rot_x: self.rot_x,
            rot_y: self.rot_y,
            rot_z: self.rot_z,
            is_reflection_enabled: self.is_reflection_enabled,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            drag_points: self.drag_points.clone(),
        }
    }
}

impl Default for Rubber {
    fn default() -> Self {
        let height: f32 = 25.0;
        let hit_height: Option<f32> = None; //25.0;
        let thickness: i32 = 8;
        let hit_event: bool = false;
        let material: String = Default::default();
        let is_timer_enabled: bool = false;
        let timer_interval: i32 = Default::default();
        let name: String = Default::default();
        let image: String = Default::default();
        let elasticity: f32 = Default::default();
        let elasticity_falloff: f32 = Default::default();
        let friction: f32 = Default::default();
        let scatter: f32 = Default::default();
        let is_collidable: bool = true;
        let is_visible: bool = true;
        let radb: Option<f32> = None; //0.0;
        let static_rendering: bool = true;
        let show_in_editor: bool = true;
        let rot_x: f32 = 0.0;
        let rot_y: f32 = 0.0;
        let rot_z: f32 = 0.0;
        let is_reflection_enabled: Option<bool> = None; //true;
        let physics_material: Option<String> = None;
        let overwrite_physics: Option<bool> = None; //false;

        // these are shared between all items
        let is_locked: bool = false;
        let editor_layer: u32 = Default::default();
        let editor_layer_name: Option<String> = None;
        let editor_layer_visibility: Option<bool> = None;

        let points: Vec<DragPoint> = Default::default();
        Rubber {
            height,
            hit_height,
            thickness,
            hit_event,
            material,
            is_timer_enabled,
            timer_interval,
            name,
            image,
            elasticity,
            elasticity_falloff,
            friction,
            scatter,
            is_collidable,
            is_visible,
            radb,
            static_rendering,
            show_in_editor,
            rot_x,
            rot_y,
            rot_z,
            is_reflection_enabled,
            physics_material,
            overwrite_physics,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
            drag_points: points,
        }
    }
}

impl Serialize for Rubber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RubberJson::from_rubber(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Rubber {
    fn deserialize<D>(deserializer: D) -> Result<Rubber, D::Error>
    where
        D: Deserializer<'de>,
    {
        let rubber_json = RubberJson::deserialize(deserializer)?;
        Ok(rubber_json.to_rubber())
    }
}

impl BiffRead for Rubber {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut rubber = Rubber::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "HTTP" => {
                    rubber.height = reader.get_f32();
                }
                "HTHI" => {
                    rubber.hit_height = Some(reader.get_f32());
                }
                "WDTP" => {
                    rubber.thickness = reader.get_i32();
                }
                "HTEV" => {
                    rubber.hit_event = reader.get_bool();
                }
                "MATR" => {
                    rubber.material = reader.get_string();
                }
                "TMON" => {
                    rubber.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    rubber.timer_interval = reader.get_i32();
                }
                "NAME" => {
                    rubber.name = reader.get_wide_string();
                }
                "IMAG" => {
                    rubber.image = reader.get_string();
                }
                "ELAS" => {
                    rubber.elasticity = reader.get_f32();
                }
                "ELFO" => {
                    rubber.elasticity_falloff = reader.get_f32();
                }
                "RFCT" => {
                    rubber.friction = reader.get_f32();
                }
                "RSCT" => {
                    rubber.scatter = reader.get_f32();
                }
                "CLDR" => {
                    rubber.is_collidable = reader.get_bool();
                }
                "RVIS" => {
                    rubber.is_visible = reader.get_bool();
                }
                "RADB" => {
                    rubber.radb = Some(reader.get_f32());
                }
                "ESTR" => {
                    rubber.static_rendering = reader.get_bool();
                }
                "ESIE" => {
                    rubber.show_in_editor = reader.get_bool();
                }
                "ROTX" => {
                    rubber.rot_x = reader.get_f32();
                }
                "ROTY" => {
                    rubber.rot_y = reader.get_f32();
                }
                "ROTZ" => {
                    rubber.rot_z = reader.get_f32();
                }
                "REEN" => {
                    rubber.is_reflection_enabled = Some(reader.get_bool());
                }
                "MAPH" => {
                    rubber.physics_material = Some(reader.get_string());
                }
                "OVPH" => {
                    rubber.overwrite_physics = Some(reader.get_bool());
                }

                // shared
                "LOCK" => {
                    rubber.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    rubber.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    rubber.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    rubber.editor_layer_visibility = Some(reader.get_bool());
                }

                "PNTS" => {
                    // this is just a tag with no data
                }
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    rubber.drag_points.push(point);
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
        rubber
    }
}

impl BiffWrite for Rubber {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_f32("HTTP", self.height);
        if let Some(hthi) = self.hit_height {
            writer.write_tagged_f32("HTHI", hthi);
        }
        writer.write_tagged_i32("WDTP", self.thickness);
        writer.write_tagged_bool("HTEV", self.hit_event);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("RFCT", self.friction);
        writer.write_tagged_f32("RSCT", self.scatter);
        writer.write_tagged_bool("CLDR", self.is_collidable);
        writer.write_tagged_bool("RVIS", self.is_visible);
        if let Some(radb) = self.radb {
            writer.write_tagged_f32("RADB", radb);
        }
        writer.write_tagged_bool("ESTR", self.static_rendering);
        writer.write_tagged_bool("ESIE", self.show_in_editor);
        writer.write_tagged_f32("ROTX", self.rot_x);
        writer.write_tagged_f32("ROTY", self.rot_y);
        writer.write_tagged_f32("ROTZ", self.rot_z);
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
            writer.write_tagged("DPNT", point);
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
        // values not equal to the defaults
        let rubber: Rubber = Rubber {
            height: 1.0,
            hit_height: Some(2.0),
            thickness: 3,
            hit_event: rng.gen(),
            material: "material".to_string(),
            is_timer_enabled: rng.gen(),
            timer_interval: 4,
            name: "name".to_string(),
            image: "image".to_string(),
            elasticity: 5.0,
            elasticity_falloff: 6.0,
            friction: 7.0,
            scatter: 8.0,
            is_collidable: rng.gen(),
            is_visible: rng.gen(),
            radb: Some(9.0),
            static_rendering: rng.gen(),
            show_in_editor: rng.gen(),
            rot_x: 9.0,
            rot_y: 10.0,
            rot_z: 11.0,
            is_reflection_enabled: rng.gen(),
            physics_material: Some("physics_material".to_string()),
            overwrite_physics: rng.gen(),
            is_locked: rng.gen(),
            editor_layer: 12,
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: rng.gen(),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Rubber::biff_write(&rubber, &mut writer);
        let rubber_read = Rubber::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(rubber, rubber_read);
    }
}
