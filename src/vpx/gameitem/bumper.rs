use super::{GameItem, vertex2d::Vertex2D};
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::{HasSharedAttributes, TimerDataRoot, WriteSharedAttributes};
use crate::vpx::json::F32WithNanInf;
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, PartialEq)]
pub struct Bumper {
    pub center: Vertex2D,
    pub radius: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    pub threshold: f32,
    pub force: f32,
    pub scatter: Option<f32>,
    // BSCT (added in ?)
    pub height_scale: f32,
    pub ring_speed: f32,
    pub orientation: f32,
    pub ring_drop_offset: Option<f32>,
    // RDLI (added in ?)
    pub cap_material: String,
    pub base_material: String,
    pub socket_material: String,
    pub ring_material: Option<String>,
    // RIMA (added in ?)
    surface: String,
    pub name: String,
    pub is_cap_visible: bool,
    pub is_base_visible: bool,
    pub is_ring_visible: Option<bool>,       // RIVS (added in ?)
    pub is_socket_visible: Option<bool>,     // SKVS (added in ?)
    pub hit_event: Option<bool>,             // HAHE (added in ?)
    pub is_collidable: Option<bool>,         // COLI (added in ?)
    pub is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)

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
struct BumperJson {
    center: Vertex2D,
    radius: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    threshold: f32,
    force: f32,
    scatter: Option<f32>,
    // BSCT (added in ?)
    height_scale: f32,
    ring_speed: f32,
    orientation: f32,
    ring_drop_offset: Option<F32WithNanInf>,
    // RDLI (added in ?)
    cap_material: String,
    base_material: String,
    socket_material: String,
    ring_material: Option<String>,
    // RIMA (added in ?)
    surface: String,
    name: String,
    is_cap_visible: bool,
    is_base_visible: bool,
    is_ring_visible: Option<bool>,
    is_socket_visible: Option<bool>,
    hit_event: Option<bool>,
    is_collidable: Option<bool>,
    is_reflection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl From<&Bumper> for BumperJson {
    fn from(bumper: &Bumper) -> Self {
        Self {
            center: bumper.center,
            radius: bumper.radius,
            is_timer_enabled: bumper.is_timer_enabled,
            timer_interval: bumper.timer_interval,
            threshold: bumper.threshold,
            force: bumper.force,
            scatter: bumper.scatter,
            height_scale: bumper.height_scale,
            ring_speed: bumper.ring_speed,
            orientation: bumper.orientation,
            ring_drop_offset: bumper.ring_drop_offset.map(F32WithNanInf::from),
            cap_material: bumper.cap_material.clone(),
            base_material: bumper.base_material.clone(),
            socket_material: bumper.socket_material.clone(),
            ring_material: bumper.ring_material.clone(),
            surface: bumper.surface.clone(),
            name: bumper.name.clone(),
            is_cap_visible: bumper.is_cap_visible,
            is_base_visible: bumper.is_base_visible,
            is_ring_visible: bumper.is_ring_visible,
            is_socket_visible: bumper.is_socket_visible,
            hit_event: bumper.hit_event,
            is_collidable: bumper.is_collidable,
            is_reflection_enabled: bumper.is_reflection_enabled,
            part_group_name: bumper.part_group_name.clone(),
        }
    }
}

impl Default for Bumper {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            radius: 45.0,
            is_timer_enabled: false,
            timer_interval: 0,
            threshold: 1.0,
            force: 15.0,
            scatter: None, //0.0
            height_scale: 90.0,
            ring_speed: 0.5,
            orientation: 0.0,
            ring_drop_offset: None, //0.0
            cap_material: Default::default(),
            base_material: Default::default(),
            socket_material: Default::default(),
            ring_material: None,
            surface: Default::default(),
            name: Default::default(),
            is_cap_visible: true,
            is_base_visible: true,
            is_ring_visible: None,       //true
            is_socket_visible: None,     //true
            hit_event: None,             //true
            is_collidable: None,         //true
            is_reflection_enabled: None, //true
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl Serialize for Bumper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bumper_json = BumperJson::from(self);
        bumper_json.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Bumper {
    fn deserialize<D>(deserializer: D) -> Result<Bumper, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bumper_json = BumperJson::deserialize(deserializer)?;
        let bumper = Bumper {
            center: bumper_json.center,
            radius: bumper_json.radius,
            is_timer_enabled: bumper_json.is_timer_enabled,
            timer_interval: bumper_json.timer_interval,
            threshold: bumper_json.threshold,
            force: bumper_json.force,
            scatter: bumper_json.scatter,
            height_scale: bumper_json.height_scale,
            ring_speed: bumper_json.ring_speed,
            orientation: bumper_json.orientation,
            ring_drop_offset: bumper_json.ring_drop_offset.map(f32::from),
            cap_material: bumper_json.cap_material,
            base_material: bumper_json.base_material,
            socket_material: bumper_json.socket_material,
            ring_material: bumper_json.ring_material,
            surface: bumper_json.surface,
            name: bumper_json.name,
            is_cap_visible: bumper_json.is_cap_visible,
            is_base_visible: bumper_json.is_base_visible,
            is_ring_visible: bumper_json.is_ring_visible,
            is_socket_visible: bumper_json.is_socket_visible,
            hit_event: bumper_json.hit_event,
            is_collidable: bumper_json.is_collidable,
            is_reflection_enabled: bumper_json.is_reflection_enabled,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: bumper_json.part_group_name,
        };
        Ok(bumper)
    }
}

impl GameItem for Bumper {
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasSharedAttributes for Bumper {
    fn name(&self) -> &str {
        &self.name
    }

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

impl TimerDataRoot for Bumper {
    fn is_timer_enabled(&self) -> bool {
        self.is_timer_enabled
    }
    fn timer_interval(&self) -> i32 {
        self.timer_interval
    }
}

impl BiffRead for Bumper {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut bumper = Bumper::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    bumper.center = Vertex2D::biff_read(reader);
                }
                "RADI" => {
                    bumper.radius = reader.get_f32();
                }
                "TMON" => {
                    bumper.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    bumper.timer_interval = reader.get_i32();
                }
                "THRS" => {
                    bumper.threshold = reader.get_f32();
                }
                "FORC" => {
                    bumper.force = reader.get_f32();
                }
                "BSCT" => {
                    bumper.scatter = Some(reader.get_f32());
                }
                "HISC" => {
                    bumper.height_scale = reader.get_f32();
                }
                "RISP" => {
                    bumper.ring_speed = reader.get_f32();
                }
                "ORIN" => {
                    bumper.orientation = reader.get_f32();
                }
                "RDLI" => {
                    bumper.ring_drop_offset = Some(reader.get_f32());
                }
                "MATR" => {
                    bumper.cap_material = reader.get_string();
                }
                "BAMA" => {
                    bumper.base_material = reader.get_string();
                }
                "SKMA" => {
                    bumper.socket_material = reader.get_string();
                }
                "RIMA" => {
                    bumper.ring_material = Some(reader.get_string());
                }
                "SURF" => {
                    bumper.surface = reader.get_string();
                }
                "NAME" => {
                    bumper.name = reader.get_wide_string();
                }
                "CAVI" => {
                    bumper.is_cap_visible = reader.get_bool();
                }
                "BSVS" => {
                    bumper.is_base_visible = reader.get_bool();
                }
                "RIVS" => {
                    bumper.is_ring_visible = Some(reader.get_bool());
                }
                "SKVS" => {
                    bumper.is_socket_visible = Some(reader.get_bool());
                }
                "HAHE" => {
                    bumper.hit_event = Some(reader.get_bool());
                }
                "COLI" => {
                    bumper.is_collidable = Some(reader.get_bool());
                }
                "REEN" => {
                    bumper.is_reflection_enabled = Some(reader.get_bool());
                }

                // shared
                "LOCK" => {
                    bumper.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    bumper.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    bumper.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    bumper.editor_layer_visibility = Some(reader.get_bool());
                }
                "GRUP" => {
                    bumper.part_group_name = Some(reader.get_string());
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
        bumper
    }
}

impl BiffWrite for Bumper {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("RADI", self.radius);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_f32("THRS", self.threshold);
        writer.write_tagged_f32("FORC", self.force);
        if let Some(scatter) = self.scatter {
            writer.write_tagged_f32("BSCT", scatter);
        }
        writer.write_tagged_f32("HISC", self.height_scale);
        writer.write_tagged_f32("RISP", self.ring_speed);
        writer.write_tagged_f32("ORIN", self.orientation);
        if let Some(ring_drop_offset) = self.ring_drop_offset {
            writer.write_tagged_f32("RDLI", ring_drop_offset);
        }
        writer.write_tagged_string("MATR", &self.cap_material);
        writer.write_tagged_string("BAMA", &self.base_material);
        writer.write_tagged_string("SKMA", &self.socket_material);
        if let Some(ring_material) = &self.ring_material {
            writer.write_tagged_string("RIMA", ring_material);
        }
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_bool("CAVI", self.is_cap_visible);
        writer.write_tagged_bool("BSVS", self.is_base_visible);
        if let Some(is_ring_visible) = self.is_ring_visible {
            writer.write_tagged_bool("RIVS", is_ring_visible);
        }
        if let Some(is_socket_visible) = self.is_socket_visible {
            writer.write_tagged_bool("SKVS", is_socket_visible);
        }
        if let Some(hit_event) = self.hit_event {
            writer.write_tagged_bool("HAHE", hit_event);
        }
        if let Some(is_collidable) = self.is_collidable {
            writer.write_tagged_bool("COLI", is_collidable);
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
        // random data not same as default data above
        let bumper = Bumper {
            center: Vertex2D::new(1.0, 2.0),
            radius: 45.0,
            is_timer_enabled: true,
            timer_interval: 3,
            threshold: 1.0,
            force: 15.0,
            scatter: Some(0.0),
            height_scale: 90.0,
            ring_speed: 0.5,
            orientation: 0.0,
            ring_drop_offset: Some(0.0),
            cap_material: "ctest cap material".to_string(),
            base_material: "test base material".to_string(),
            socket_material: "test socket material".to_string(),
            ring_material: Some("test ring material".to_string()),
            surface: "test surface".to_string(),
            name: "test bumper".to_string(),
            is_cap_visible: true,
            is_base_visible: true,
            is_ring_visible: Some(true),
            is_socket_visible: Some(true),
            hit_event: Some(true),
            is_collidable: Some(true),
            is_reflection_enabled: Some(true),
            is_locked: true,
            editor_layer: 5,
            editor_layer_name: Some("layer".to_string()),
            editor_layer_visibility: Some(true),
            part_group_name: Some("part group".to_string()),
        };
        let mut writer = BiffWriter::new();
        Bumper::biff_write(&bumper, &mut writer);
        let bumper_read = Bumper::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(bumper, bumper_read);
    }
}
