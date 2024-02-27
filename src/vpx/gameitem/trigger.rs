use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{dragpoint::DragPoint, vertex2d::Vertex2D, TRIGGER_SHAPE_WIRE_A};

#[derive(Debug, PartialEq, Dummy)]
pub struct Trigger {
    pub center: Vertex2D,
    pub radius: f32,
    pub rotation: f32,
    pub wire_thickness: Option<f32>, // WITI (was missing in 10.01)
    pub scale_x: f32,
    pub scale_y: f32,
    pub is_timer_enabled: bool,
    pub timer_interval: i32,
    pub material: String,
    pub surface: String,
    pub is_visible: bool,
    pub is_enabled: bool,
    pub hit_height: f32,
    pub name: String,
    // [BiffInt("SHAP", Pos = 15)]
    // public int Shape = TriggerShape.TriggerWireA;

    // [BiffFloat("ANSP", Pos = 16)]
    // public float AnimSpeed = 1f;

    // [BiffBool("REEN", Pos = 17)]
    // public bool IsReflectionEnabled = true;
    pub shape: u32,
    pub anim_speed: f32,
    pub is_reflection_enabled: Option<bool>, // REEN (was missing in 10.01)

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,

    drag_points: Vec<DragPoint>,
}

#[derive(Serialize, Deserialize)]
struct TriggerJson {
    center: Vertex2D,
    radius: f32,
    rotation: f32,
    wire_thickness: Option<f32>,
    scale_x: f32,
    scale_y: f32,
    is_timer_enabled: bool,
    timer_interval: i32,
    material: String,
    surface: String,
    is_visible: bool,
    is_enabled: bool,
    hit_height: f32,
    name: String,
    shape: u32,
    anim_speed: f32,
    is_reflection_enabled: Option<bool>,
    is_locked: bool,
    editor_layer: u32,
    editor_layer_name: Option<String>,
    editor_layer_visibility: Option<bool>,
    drag_points: Vec<DragPoint>,
}

impl TriggerJson {
    pub fn from_trigger(trigger: &Trigger) -> Self {
        Self {
            center: trigger.center,
            radius: trigger.radius,
            rotation: trigger.rotation,
            wire_thickness: trigger.wire_thickness,
            scale_x: trigger.scale_x,
            scale_y: trigger.scale_y,
            is_timer_enabled: trigger.is_timer_enabled,
            timer_interval: trigger.timer_interval,
            material: trigger.material.clone(),
            surface: trigger.surface.clone(),
            is_visible: trigger.is_visible,
            is_enabled: trigger.is_enabled,
            hit_height: trigger.hit_height,
            name: trigger.name.clone(),
            shape: trigger.shape,
            anim_speed: trigger.anim_speed,
            is_reflection_enabled: trigger.is_reflection_enabled,
            is_locked: trigger.is_locked,
            editor_layer: trigger.editor_layer,
            editor_layer_name: trigger.editor_layer_name.clone(),
            editor_layer_visibility: trigger.editor_layer_visibility,
            drag_points: trigger.drag_points.clone(),
        }
    }
    pub fn to_trigger(&self) -> Trigger {
        Trigger {
            center: self.center,
            radius: self.radius,
            rotation: self.rotation,
            wire_thickness: self.wire_thickness,
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            material: self.material.clone(),
            surface: self.surface.clone(),
            is_visible: self.is_visible,
            is_enabled: self.is_enabled,
            hit_height: self.hit_height,
            name: self.name.clone(),
            shape: self.shape,
            anim_speed: self.anim_speed,
            is_reflection_enabled: self.is_reflection_enabled,
            is_locked: self.is_locked,
            editor_layer: self.editor_layer,
            editor_layer_name: self.editor_layer_name.clone(),
            editor_layer_visibility: self.editor_layer_visibility,
            drag_points: self.drag_points.clone(),
        }
    }
}

impl Serialize for Trigger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TriggerJson::from_trigger(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Trigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let trigger_json = TriggerJson::deserialize(deserializer)?;
        Ok(trigger_json.to_trigger())
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Trigger {
            center: Default::default(),
            radius: 25.0,
            rotation: Default::default(),
            wire_thickness: Default::default(),
            scale_x: Default::default(),
            scale_y: Default::default(),
            is_timer_enabled: false,
            timer_interval: Default::default(),
            material: Default::default(),
            surface: Default::default(),
            is_visible: true,
            is_enabled: true,
            hit_height: 50.0,
            name: Default::default(),
            shape: TRIGGER_SHAPE_WIRE_A,
            anim_speed: Default::default(),
            is_reflection_enabled: None, //true,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            drag_points: Default::default(),
        }
    }
}

impl BiffRead for Trigger {
    fn biff_read(reader: &mut BiffReader<'_>) -> Trigger {
        let mut trigger = Trigger::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                // tag_str: SHAP
                // tag_str: ANSP
                // tag_str: REEN
                "VCEN" => {
                    trigger.center = Vertex2D::biff_read(reader);
                }
                "RADI" => {
                    trigger.radius = reader.get_f32();
                }
                "ROTA" => {
                    trigger.rotation = reader.get_f32();
                }
                "WITI" => {
                    trigger.wire_thickness = Some(reader.get_f32());
                }
                "SCAX" => {
                    trigger.scale_x = reader.get_f32();
                }
                "SCAY" => {
                    trigger.scale_y = reader.get_f32();
                }
                "TMON" => {
                    trigger.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    trigger.timer_interval = reader.get_i32();
                }
                "MATR" => {
                    trigger.material = reader.get_string();
                }
                "SURF" => {
                    trigger.surface = reader.get_string();
                }
                "VSBL" => {
                    trigger.is_visible = reader.get_bool();
                }
                "EBLD" => {
                    trigger.is_enabled = reader.get_bool();
                }
                "THOT" => {
                    trigger.hit_height = reader.get_f32();
                }
                "NAME" => {
                    trigger.name = reader.get_wide_string();
                }
                "SHAP" => {
                    trigger.shape = reader.get_u32();
                }
                "ANSP" => {
                    trigger.anim_speed = reader.get_f32();
                }
                "REEN" => {
                    trigger.is_reflection_enabled = Some(reader.get_bool());
                }
                // shared
                "LOCK" => {
                    trigger.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    trigger.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    trigger.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    trigger.editor_layer_visibility = Some(reader.get_bool());
                }
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    trigger.drag_points.push(point);
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
        trigger
    }
}

impl BiffWrite for Trigger {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("RADI", self.radius);
        writer.write_tagged_f32("ROTA", self.rotation);
        if let Some(wire_thickness) = self.wire_thickness {
            writer.write_tagged_f32("WITI", wire_thickness);
        }
        writer.write_tagged_f32("SCAX", self.scale_x);
        writer.write_tagged_f32("SCAY", self.scale_y);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("EBLD", self.is_enabled);
        writer.write_tagged_bool("VSBL", self.is_visible);
        writer.write_tagged_f32("THOT", self.hit_height);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("SHAP", self.shape);
        writer.write_tagged_f32("ANSP", self.anim_speed);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
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

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let trigger = Trigger {
            center: Vertex2D::new(1.0, 2.0),
            radius: 25.0,
            rotation: 3.0,
            wire_thickness: Some(4.0),
            scale_x: 5.0,
            scale_y: 6.0,
            is_timer_enabled: true,
            timer_interval: 7,
            material: "test material".to_string(),
            surface: "test surface".to_string(),
            is_visible: false,
            is_enabled: false,
            hit_height: 8.0,
            name: "test name".to_string(),
            shape: 9,
            anim_speed: 10.0,
            is_reflection_enabled: Some(false),
            is_locked: true,
            editor_layer: 11,
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: Some(false),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Trigger::biff_write(&trigger, &mut writer);
        let trigger_read = Trigger::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(trigger, trigger_read);
    }
}
