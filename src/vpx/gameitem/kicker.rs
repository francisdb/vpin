use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Dummy)]
pub struct Kicker {
    center: Vertex2D,
    radius: f32,
    is_timer_enabled: bool,
    timer_interval: u32,
    material: String,
    surface: String,
    is_enabled: bool,
    pub name: String,
    kicker_type: u32,
    scatter: f32,
    hit_accuracy: f32,
    hit_height: Option<f32>, // KHHI (was missing in 10.01)
    orientation: f32,
    fall_through: bool,
    legacy_mode: bool,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct KickerJson {
    center: Vertex2D,
    radius: f32,
    is_timer_enabled: bool,
    timer_interval: u32,
    material: String,
    surface: String,
    is_enabled: bool,
    name: String,
    kicker_type: u32,
    scatter: f32,
    hit_accuracy: f32,
    hit_height: Option<f32>,
    orientation: f32,
    fall_through: bool,
    legacy_mode: bool,
    is_locked: bool,
    editor_layer: u32,
    editor_layer_name: Option<String>,
    editor_layer_visibility: Option<bool>,
}

impl KickerJson {
    fn from_kicker(kicker: &Kicker) -> Self {
        Self {
            center: kicker.center,
            radius: kicker.radius,
            is_timer_enabled: kicker.is_timer_enabled,
            timer_interval: kicker.timer_interval,
            material: kicker.material.clone(),
            surface: kicker.surface.clone(),
            is_enabled: kicker.is_enabled,
            name: kicker.name.clone(),
            kicker_type: kicker.kicker_type,
            scatter: kicker.scatter,
            hit_accuracy: kicker.hit_accuracy,
            hit_height: kicker.hit_height,
            orientation: kicker.orientation,
            fall_through: kicker.fall_through,
            legacy_mode: kicker.legacy_mode,
            is_locked: kicker.is_locked,
            editor_layer: kicker.editor_layer,
            editor_layer_name: kicker.editor_layer_name.clone(),
            editor_layer_visibility: kicker.editor_layer_visibility,
        }
    }

    fn into_kicker(self) -> Kicker {
        Kicker {
            center: self.center,
            radius: self.radius,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            material: self.material,
            surface: self.surface,
            is_enabled: self.is_enabled,
            name: self.name,
            kicker_type: self.kicker_type,
            scatter: self.scatter,
            hit_accuracy: self.hit_accuracy,
            hit_height: self.hit_height,
            orientation: self.orientation,
            fall_through: self.fall_through,
            legacy_mode: self.legacy_mode,
            is_locked: self.is_locked,
            editor_layer: self.editor_layer,
            editor_layer_name: self.editor_layer_name,
            editor_layer_visibility: self.editor_layer_visibility,
        }
    }
}

impl Serialize for Kicker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        KickerJson::from_kicker(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Kicker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kicker_json = KickerJson::deserialize(deserializer)?;
        Ok(kicker_json.into_kicker())
    }
}

impl Kicker {
    pub const KICKER_TYPE_INVISIBLE: u32 = 0;
    pub const KICKER_TYPE_HOLE: u32 = 1;
    pub const KICKER_TYPE_CUP: u32 = 2;
    pub const KICKER_TYPE_HOLE_SIMPLE: u32 = 3;
    pub const KICKER_TYPE_WILLIAMS: u32 = 4;
    pub const KICKER_TYPE_GOTTLIEB: u32 = 5;
    pub const KICKER_TYPE_CUP2: u32 = 6;
}

impl Default for Kicker {
    fn default() -> Self {
        Self {
            center: Default::default(),
            radius: 25.0,
            is_timer_enabled: false,
            timer_interval: 0,
            material: Default::default(),
            surface: Default::default(),
            is_enabled: true,
            name: Default::default(),
            kicker_type: Kicker::KICKER_TYPE_HOLE,
            scatter: 0.0,
            hit_accuracy: 0.7,
            hit_height: None, //40.0,
            orientation: 0.0,
            fall_through: false,
            legacy_mode: true,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

impl BiffRead for Kicker {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut kicker = Kicker::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    kicker.center = Vertex2D::biff_read(reader);
                }
                "RADI" => {
                    kicker.radius = reader.get_f32();
                }
                "TMON" => {
                    kicker.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    kicker.timer_interval = reader.get_u32();
                }
                "MATR" => {
                    kicker.material = reader.get_string();
                }
                "SURF" => {
                    kicker.surface = reader.get_string();
                }
                "EBLD" => {
                    kicker.is_enabled = reader.get_bool();
                }
                "NAME" => {
                    kicker.name = reader.get_wide_string();
                }
                "TYPE" => {
                    kicker.kicker_type = reader.get_u32();
                }
                "KSCT" => {
                    kicker.scatter = reader.get_f32();
                }
                "KHAC" => {
                    kicker.hit_accuracy = reader.get_f32();
                }
                "KHHI" => {
                    kicker.hit_height = Some(reader.get_f32());
                }
                "KORI" => {
                    kicker.orientation = reader.get_f32();
                }
                "FATH" => {
                    kicker.fall_through = reader.get_bool();
                }
                "LEMO" => {
                    kicker.legacy_mode = reader.get_bool();
                }

                // shared
                "LOCK" => {
                    kicker.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    kicker.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    kicker.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    kicker.editor_layer_visibility = Some(reader.get_bool());
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
        kicker
    }
}

impl BiffWrite for Kicker {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("RADI", self.radius);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_bool("EBLD", self.is_enabled);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("TYPE", self.kicker_type);
        writer.write_tagged_f32("KSCT", self.scatter);
        writer.write_tagged_f32("KHAC", self.hit_accuracy);
        if let Some(hit_height) = self.hit_height {
            writer.write_tagged_f32("KHHI", hit_height);
        }
        writer.write_tagged_f32("KORI", self.orientation);
        writer.write_tagged_bool("FATH", self.fall_through);
        writer.write_tagged_bool("LEMO", self.legacy_mode);
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

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let kicker = Kicker {
            center: Vertex2D::new(1.0, 2.0),
            radius: 3.0,
            is_timer_enabled: true,
            timer_interval: 4,
            material: "material".to_string(),
            surface: "surface".to_string(),
            is_enabled: false,
            name: "name".to_string(),
            kicker_type: 5,
            scatter: 6.0,
            hit_accuracy: 7.0,
            hit_height: Some(8.0),
            orientation: 9.0,
            fall_through: true,
            legacy_mode: false,
            is_locked: true,
            editor_layer: 10,
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: Some(false),
        };
        let mut writer = BiffWriter::new();
        Kicker::biff_write(&kicker, &mut writer);
        let kicker_read = Kicker::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(kicker, kicker_read);
    }
}
