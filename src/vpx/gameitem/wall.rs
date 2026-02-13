use super::dragpoint::DragPoint;
use crate::impl_shared_attributes;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite, BiffWriter};
use crate::vpx::gameitem::select::{TimerDataRoot, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/**
 * Surface
 */
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Wall {
    pub name: String,
    pub hit_event: bool,
    pub is_droppable: bool,
    pub is_flipbook: bool,
    pub is_bottom_solid: bool,
    pub is_collidable: bool,
    is_timer_enabled: bool,
    timer_interval: i32,
    pub threshold: f32,
    pub image: String,
    pub side_image: String,
    pub side_material: String,
    pub top_material: String,
    pub slingshot_material: String,
    pub height_bottom: f32,
    pub height_top: f32,
    /// Whether to display the top image texture in the VPinball editor preview.
    /// This does NOT affect runtime rendering - textures are always rendered if set.
    /// See: https://github.com/vpinball/vpinball/blob/master/src/parts/surface.h
    pub display_texture: bool,
    pub slingshot_force: f32,
    pub slingshot_threshold: f32,
    pub elasticity: f32,
    pub elasticity_falloff: Option<f32>, // added in ?
    pub friction: f32,
    pub scatter: f32,
    pub is_top_bottom_visible: bool,
    pub slingshot_animation: bool,
    pub is_side_visible: bool,

    /// Legacy field for disabling lighting on top surface.
    /// Replaced by `disable_lighting_top` in VPX 10.8.
    ///
    /// BIFF tag: `DILI` (removed in 10.8)
    pub disable_lighting_top_old: Option<f32>,

    /// Controls how much lighting is disabled on the top surface.
    /// Range: 0.0 to 1.0
    /// - 0.0 = full lighting (normal rendering)
    /// - 1.0 = lighting fully disabled (appears darker/unlit)
    ///
    /// BIFF tag: `DILT` (added in 10.8)
    pub disable_lighting_top: Option<f32>,

    /// Controls light transmission through this surface from lights below.
    /// Despite the confusing name "disable_lighting_below", this value controls
    /// how much light from below is BLOCKED, not transmitted.
    ///
    /// Range: 0.0 to 1.0
    /// - 0.0 = light passes through fully (fully transmissive/transparent to light)
    /// - 1.0 = light is fully blocked (opaque, no light transmission)
    ///
    /// VPinball shader uses: `lerp(light_from_below, 0, disable_lighting_below)`
    /// This ADDS light from below to the surface color, it doesn't make it see-through.
    ///
    /// BIFF tag: `DILB` (added in 10.?)
    pub disable_lighting_below: Option<f32>,

    pub is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)
    pub physics_material: Option<String>,    // MAPH (added in 10.?)
    pub overwrite_physics: Option<bool>,     // OVPH (added in 10.?)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    pub part_group_name: Option<String>,

    pub drag_points: Vec<DragPoint>,
}
impl_shared_attributes!(Wall);

#[derive(Serialize, Deserialize)]
struct WallJson {
    hit_event: bool,
    is_droppable: bool,
    is_flipbook: bool,
    is_bottom_solid: bool,
    is_collidable: bool,
    is_timer_enabled: bool,
    timer_interval: i32,
    threshold: f32,
    image: String,
    side_image: String,
    side_material: String,
    top_material: String,
    slingshot_material: String,
    height_bottom: f32,
    height_top: f32,
    name: String,
    display_texture: bool,
    slingshot_force: f32,
    slingshot_threshold: f32,
    elasticity: f32,
    elasticity_falloff: Option<f32>,
    friction: f32,
    scatter: f32,
    is_top_bottom_visible: bool,
    slingshot_animation: bool,
    is_side_visible: bool,
    disable_lighting_top_old: Option<f32>,
    disable_lighting_top: Option<f32>,
    disable_lighting_below: Option<f32>,
    is_reflection_enabled: Option<bool>,
    physics_material: Option<String>,
    overwrite_physics: Option<bool>,
    drag_points: Vec<DragPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl WallJson {
    pub fn from_wall(wall: &Wall) -> Self {
        Self {
            hit_event: wall.hit_event,
            is_droppable: wall.is_droppable,
            is_flipbook: wall.is_flipbook,
            is_bottom_solid: wall.is_bottom_solid,
            is_collidable: wall.is_collidable,
            is_timer_enabled: wall.is_timer_enabled,
            timer_interval: wall.timer_interval,
            threshold: wall.threshold,
            image: wall.image.clone(),
            side_image: wall.side_image.clone(),
            side_material: wall.side_material.clone(),
            top_material: wall.top_material.clone(),
            slingshot_material: wall.slingshot_material.clone(),
            height_bottom: wall.height_bottom,
            height_top: wall.height_top,
            name: wall.name.clone(),
            display_texture: wall.display_texture,
            slingshot_force: wall.slingshot_force,
            slingshot_threshold: wall.slingshot_threshold,
            elasticity: wall.elasticity,
            elasticity_falloff: wall.elasticity_falloff,
            friction: wall.friction,
            scatter: wall.scatter,
            is_top_bottom_visible: wall.is_top_bottom_visible,
            slingshot_animation: wall.slingshot_animation,
            is_side_visible: wall.is_side_visible,
            disable_lighting_top_old: wall.disable_lighting_top_old,
            disable_lighting_top: wall.disable_lighting_top,
            disable_lighting_below: wall.disable_lighting_below,
            is_reflection_enabled: wall.is_reflection_enabled,
            physics_material: wall.physics_material.clone(),
            overwrite_physics: wall.overwrite_physics,
            drag_points: wall.drag_points.clone(),
            part_group_name: wall.part_group_name.clone(),
        }
    }

    pub fn to_wall(&self) -> Wall {
        Wall {
            hit_event: self.hit_event,
            is_droppable: self.is_droppable,
            is_flipbook: self.is_flipbook,
            is_bottom_solid: self.is_bottom_solid,
            is_collidable: self.is_collidable,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            threshold: self.threshold,
            image: self.image.clone(),
            side_image: self.side_image.clone(),
            side_material: self.side_material.clone(),
            top_material: self.top_material.clone(),
            slingshot_material: self.slingshot_material.clone(),
            height_bottom: self.height_bottom,
            height_top: self.height_top,
            name: self.name.clone(),
            display_texture: self.display_texture,
            slingshot_force: self.slingshot_force,
            slingshot_threshold: self.slingshot_threshold,
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            scatter: self.scatter,
            is_top_bottom_visible: self.is_top_bottom_visible,
            slingshot_animation: self.slingshot_animation,
            is_side_visible: self.is_side_visible,
            disable_lighting_top_old: self.disable_lighting_top_old,
            disable_lighting_top: self.disable_lighting_top,
            disable_lighting_below: self.disable_lighting_below,
            is_reflection_enabled: self.is_reflection_enabled,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
            drag_points: self.drag_points.clone(),
        }
    }
}

impl Serialize for Wall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        WallJson::from_wall(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Wall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = WallJson::deserialize(deserializer)?;
        Ok(json.to_wall())
    }
}

impl Default for Wall {
    fn default() -> Self {
        Self {
            hit_event: false,
            is_droppable: false,
            is_flipbook: false,
            is_bottom_solid: false,
            is_collidable: true,
            is_timer_enabled: false,
            timer_interval: 0,
            threshold: 2.0,
            image: Default::default(),
            side_image: Default::default(),
            side_material: Default::default(),
            top_material: Default::default(),
            slingshot_material: Default::default(),
            height_bottom: Default::default(),
            height_top: 50.0,
            name: Default::default(),
            display_texture: false,
            slingshot_force: 80.0,
            slingshot_threshold: 0.0,
            elasticity: 0.3,
            elasticity_falloff: None,
            friction: 0.3,
            scatter: Default::default(),
            is_top_bottom_visible: true,
            slingshot_animation: true,
            is_side_visible: true,
            disable_lighting_top_old: None, //0.0;
            disable_lighting_top: Default::default(),
            disable_lighting_below: None,
            is_reflection_enabled: None, //true,
            physics_material: None,
            overwrite_physics: None, //true;
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
            drag_points: Default::default(),
        }
    }
}

impl TimerDataRoot for Wall {
    fn is_timer_enabled(&self) -> bool {
        self.is_timer_enabled
    }
    fn timer_interval(&self) -> i32 {
        self.timer_interval
    }
}

impl BiffRead for Wall {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut wall = Wall::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "HTEV" => {
                    wall.hit_event = reader.get_bool();
                }
                "DROP" => {
                    wall.is_droppable = reader.get_bool();
                }
                "FLIP" => {
                    wall.is_flipbook = reader.get_bool();
                }
                "BOTS" => {
                    wall.is_bottom_solid = reader.get_bool();
                }
                "COLL" => {
                    wall.is_collidable = reader.get_bool();
                }
                "THRS" => {
                    wall.threshold = reader.get_f32();
                }
                "IMGF" => {
                    wall.image = reader.get_string();
                }
                "IMGS" => {
                    wall.side_image = reader.get_string();
                }
                "MATR" => {
                    wall.side_material = reader.get_string();
                }
                "MATP" => {
                    wall.top_material = reader.get_string();
                }
                "MATL" => {
                    wall.slingshot_material = reader.get_string();
                }
                "HTBT" => {
                    wall.height_bottom = reader.get_f32();
                }
                "NAME" => {
                    wall.name = reader.get_wide_string();
                }
                "DTEX" => {
                    wall.display_texture = reader.get_bool();
                }
                "SLFO" => {
                    wall.slingshot_force = reader.get_f32();
                }
                "SLTH" => {
                    wall.slingshot_threshold = reader.get_f32();
                }
                "SLAN" => {
                    wall.slingshot_animation = reader.get_bool();
                }
                "ELAS" => {
                    wall.elasticity = reader.get_f32();
                }
                "ELFO" => {
                    wall.elasticity_falloff = Some(reader.get_f32());
                }
                "FRIC" => {
                    wall.friction = reader.get_f32();
                }
                "SCAT" => {
                    wall.scatter = reader.get_f32();
                }
                "TBVI" => {
                    wall.is_top_bottom_visible = reader.get_bool();
                }
                "OVPH" => {
                    wall.overwrite_physics = Some(reader.get_bool());
                }
                "DLTO" => {
                    wall.disable_lighting_top = Some(reader.get_f32());
                }
                "DLBE" => {
                    wall.disable_lighting_below = Some(reader.get_f32());
                }
                "SIVI" => {
                    wall.is_side_visible = reader.get_bool();
                }
                "REFL" => {
                    wall.is_reflection_enabled = Some(reader.get_bool());
                }
                "TMRN" => {
                    wall.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    wall.timer_interval = reader.get_i32();
                }
                "PMAT" => {
                    wall.physics_material = Some(reader.get_string());
                }
                "ISBS" => {
                    wall.is_bottom_solid = reader.get_bool();
                }
                "CLDW" => {
                    wall.is_collidable = reader.get_bool();
                }
                "TMON" => {
                    wall.is_timer_enabled = reader.get_bool();
                }
                "VSBL" => {
                    wall.is_top_bottom_visible = reader.get_bool();
                }
                "SLGA" => {
                    wall.slingshot_animation = reader.get_bool();
                }
                "SVBL" => {
                    wall.is_side_visible = reader.get_bool();
                }
                "DILI" => {
                    wall.disable_lighting_top_old = Some(reader.get_f32());
                }
                "DILT" => {
                    wall.disable_lighting_top = Some(reader.get_f32());
                }
                "DILB" => {
                    wall.disable_lighting_below = Some(reader.get_f32());
                }
                "MAPH" => {
                    wall.physics_material = Some(reader.get_string());
                }
                "REEN" => {
                    wall.is_reflection_enabled = Some(reader.get_bool());
                }
                "IMAG" => {
                    wall.image = reader.get_string();
                }
                "SIMG" => {
                    wall.side_image = reader.get_string();
                }
                "SIMA" => {
                    wall.side_material = reader.get_string();
                }
                "TOMA" => {
                    wall.top_material = reader.get_string();
                }
                "SLMA" => {
                    wall.slingshot_material = reader.get_string();
                }
                "HTTP" => {
                    wall.height_top = reader.get_f32();
                }
                "DSPT" => {
                    wall.display_texture = reader.get_bool();
                }
                "SLGF" => {
                    wall.slingshot_force = reader.get_f32();
                }
                "WFCT" => {
                    wall.friction = reader.get_f32();
                }
                "WSCT" => {
                    wall.scatter = reader.get_f32();
                }
                "PNTS" => {
                    // this is just a tag with no data
                }
                "DPNT" => {
                    // many of these
                    let point = DragPoint::biff_read(reader);
                    wall.drag_points.push(point);
                }
                _ => {
                    if !wall.read_shared_attribute(tag_str, reader) {
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
        wall
    }
}

impl BiffWrite for Wall {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_tagged_bool("HTEV", self.hit_event);
        writer.write_tagged_bool("DROP", self.is_droppable);
        writer.write_tagged_bool("FLIP", self.is_flipbook);
        writer.write_tagged_bool("ISBS", self.is_bottom_solid);
        writer.write_tagged_bool("CLDW", self.is_collidable);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_f32("THRS", self.threshold);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_string("SIMG", &self.side_image);
        writer.write_tagged_string("SIMA", &self.side_material);
        writer.write_tagged_string("TOMA", &self.top_material);
        writer.write_tagged_string("SLMA", &self.slingshot_material);
        writer.write_tagged_f32("HTBT", self.height_bottom);
        writer.write_tagged_f32("HTTP", self.height_top);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_bool("DSPT", self.display_texture);
        writer.write_tagged_f32("SLGF", self.slingshot_force);
        writer.write_tagged_f32("SLTH", self.slingshot_threshold);
        writer.write_tagged_f32("ELAS", self.elasticity);
        if let Some(elasticity_falloff) = self.elasticity_falloff {
            writer.write_tagged_f32("ELFO", elasticity_falloff);
        }
        writer.write_tagged_f32("WFCT", self.friction);
        writer.write_tagged_f32("WSCT", self.scatter);
        writer.write_tagged_bool("VSBL", self.is_top_bottom_visible);
        writer.write_tagged_bool("SLGA", self.slingshot_animation);
        writer.write_tagged_bool("SVBL", self.is_side_visible);
        if let Some(disable_lighting_top_old) = self.disable_lighting_top_old {
            writer.write_tagged_f32("DILI", disable_lighting_top_old);
        }
        if let Some(disable_lighting_top) = self.disable_lighting_top {
            writer.write_tagged_f32("DILT", disable_lighting_top);
        }
        if let Some(disable_lighting_below) = self.disable_lighting_below {
            writer.write_tagged_f32("DILB", disable_lighting_below);
        }
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        if let Some(physics_material) = &self.physics_material {
            writer.write_tagged_string("MAPH", physics_material);
        }
        if let Some(overwrite_physics) = self.overwrite_physics {
            writer.write_tagged_bool("OVPH", overwrite_physics);
        }

        self.write_shared_attributes(writer);

        writer.write_marker_tag("PNTS");
        // many of these
        for point in &self.drag_points {
            writer.write_tagged("DPNT", point)
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        let wall = Wall {
            hit_event: true,
            is_droppable: true,
            is_flipbook: true,
            is_bottom_solid: true,
            is_collidable: true,
            is_timer_enabled: true,
            timer_interval: 1,
            threshold: 2.0,
            image: "image".to_string(),
            side_image: "side_image".to_string(),
            side_material: "side_material".to_string(),
            top_material: "top_material".to_string(),
            slingshot_material: "slingshot_material".to_string(),
            height_bottom: 3.0,
            height_top: 4.0,
            name: "name".to_string(),
            display_texture: true,
            slingshot_force: 5.0,
            slingshot_threshold: 6.0,
            elasticity: 7.0,
            elasticity_falloff: Some(8.0),
            friction: 9.0,
            scatter: 10.0,
            is_top_bottom_visible: true,
            slingshot_animation: true,
            is_side_visible: true,
            disable_lighting_top_old: Some(rng.random()),
            disable_lighting_top: Some(rng.random()),
            disable_lighting_below: Some(12.0),
            is_reflection_enabled: Some(true),
            physics_material: Some("physics_material".to_string()),
            overwrite_physics: Some(true),
            is_locked: true,
            editor_layer: Some(13),
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: Some(true),
            part_group_name: Some("part_group_name".to_string()),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Wall::biff_write(&wall, &mut writer);
        let wall_read = Wall::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(wall, wall_read);
    }
}
