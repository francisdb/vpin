use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum KickerType {
    Invisible = 0,
    Hole = 1,
    Cup = 2,
    HoleSimple = 3,
    Williams = 4,
    Gottlieb = 5,
    Cup2 = 6,
}

impl From<u32> for KickerType {
    fn from(value: u32) -> Self {
        match value {
            0 => KickerType::Invisible,
            1 => KickerType::Hole,
            2 => KickerType::Cup,
            3 => KickerType::HoleSimple,
            4 => KickerType::Williams,
            5 => KickerType::Gottlieb,
            6 => KickerType::Cup2,
            _ => panic!("Invalid KickerType value {}", value),
        }
    }
}

impl From<&KickerType> for u32 {
    fn from(value: &KickerType) -> Self {
        match value {
            KickerType::Invisible => 0,
            KickerType::Hole => 1,
            KickerType::Cup => 2,
            KickerType::HoleSimple => 3,
            KickerType::Williams => 4,
            KickerType::Gottlieb => 5,
            KickerType::Cup2 => 6,
        }
    }
}

/// Serialize as lowercase string
impl Serialize for KickerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            KickerType::Invisible => "invisible",
            KickerType::Hole => "hole",
            KickerType::Cup => "cup",
            KickerType::HoleSimple => "holesimple",
            KickerType::Williams => "williams",
            KickerType::Gottlieb => "gottlieb",
            KickerType::Cup2 => "cup2",
        };
        serializer.serialize_str(value)
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for KickerType {
    fn deserialize<D>(deserializer: D) -> Result<KickerType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KickerTypeVisitor;

        impl<'de> serde::de::Visitor<'de> for KickerTypeVisitor {
            type Value = KickerType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<KickerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(KickerType::Invisible),
                    1 => Ok(KickerType::Hole),
                    2 => Ok(KickerType::Cup),
                    3 => Ok(KickerType::HoleSimple),
                    4 => Ok(KickerType::Williams),
                    5 => Ok(KickerType::Gottlieb),
                    6 => Ok(KickerType::Cup2),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number between 0 and 6",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<KickerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "invisible" => Ok(KickerType::Invisible),
                    "hole" => Ok(KickerType::Hole),
                    "cup" => Ok(KickerType::Cup),
                    "holesimple" => Ok(KickerType::HoleSimple),
                    "williams" => Ok(KickerType::Williams),
                    "gottlieb" => Ok(KickerType::Gottlieb),
                    "cup2" => Ok(KickerType::Cup2),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "invisible",
                            "hole",
                            "cup",
                            "holesimple",
                            "williams",
                            "gottlieb",
                            "cup2",
                        ],
                    )),
                }
            }
        }

        deserializer.deserialize_any(KickerTypeVisitor)
    }
}

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
    kicker_type: KickerType,
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
    kicker_type: KickerType,
    scatter: f32,
    hit_accuracy: f32,
    hit_height: Option<f32>,
    orientation: f32,
    fall_through: bool,
    legacy_mode: bool,
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
            kicker_type: kicker.kicker_type.clone(),
            scatter: kicker.scatter,
            hit_accuracy: kicker.hit_accuracy,
            hit_height: kicker.hit_height,
            orientation: kicker.orientation,
            fall_through: kicker.fall_through,
            legacy_mode: kicker.legacy_mode,
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
            kicker_type: KickerType::Hole,
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
                    kicker.kicker_type = reader.get_u32().into();
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
        writer.write_tagged_u32("TYPE", (&self.kicker_type).into());
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
    use fake::{Fake, Faker};

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
            kicker_type: Faker.fake(),
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

    #[test]
    fn test_kicker_type_json() {
        let sizing_type = KickerType::Cup;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"cup\"");
        let sizing_type_read: KickerType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(1);
        let sizing_type_read: KickerType = serde_json::from_value(json).unwrap();
        assert_eq!(KickerType::Hole, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `invisible`, `hole`, `cup`, `holesimple`, `williams`, `gottlieb`, `cup2`\", line: 0, column: 0)"]
    fn test_kicker_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: KickerType = serde_json::from_value(json).unwrap();
    }
}
