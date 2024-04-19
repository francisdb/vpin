use crate::vpx::color::ColorJson;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::dragpoint::DragPoint;

#[derive(Debug, PartialEq, Clone, Dummy, Default)]
pub enum Filter {
    None = 0,
    Additive = 1,
    #[default]
    Overlay = 2,
    Multiply = 3,
    Screen = 4,
}

impl From<u32> for Filter {
    fn from(value: u32) -> Self {
        match value {
            0 => Filter::None,
            1 => Filter::Additive,
            2 => Filter::Overlay,
            3 => Filter::Multiply,
            4 => Filter::Screen,
            _ => panic!("Invalid Filter value {}", value),
        }
    }
}

impl From<&Filter> for u32 {
    fn from(value: &Filter) -> Self {
        match value {
            Filter::None => 0,
            Filter::Additive => 1,
            Filter::Overlay => 2,
            Filter::Multiply => 3,
            Filter::Screen => 4,
        }
    }
}

/// Serialize Filter as a lowercase string
impl Serialize for Filter {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Filter::None => "none",
            Filter::Additive => "additive",
            Filter::Overlay => "overlay",
            Filter::Multiply => "multiply",
            Filter::Screen => "screen",
        };
        serializer.serialize_str(value)
    }
}

/// Deserialize Filter from a lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> Result<Filter, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(s) => match s.as_str() {
                "none" => Ok(Filter::None),
                "additive" => Ok(Filter::Additive),
                "overlay" => Ok(Filter::Overlay),
                "multiply" => Ok(Filter::Multiply),
                "screen" => Ok(Filter::Screen),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid Filter value {}, expecting \"none\", \"additive\", \"overlay\", \"multiply\" or \"screen\"",
                    s
                ))),
            },
            Value::Number(n) => {
                let n = n.as_u64().unwrap();
                match n {
                    0 => Ok(Filter::None),
                    1 => Ok(Filter::Additive),
                    2 => Ok(Filter::Overlay),
                    3 => Ok(Filter::Multiply),
                    4 => Ok(Filter::Screen),
                    _ => Err(serde::de::Error::custom(
                        "Invalid Filter value, expecting 0, 1, 2, 3 or 4",
                    )),
                }
            }
            _ => Err(serde::de::Error::custom(
                "Invalid Filter value, expecting string or number",
            )),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Dummy, Default)]
pub enum ImageAlignment {
    ImageModeWorld = 0,
    #[default]
    ImageModeWrap = 1,
}

impl From<u32> for ImageAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => ImageAlignment::ImageModeWorld,
            1 => ImageAlignment::ImageModeWrap,
            _ => panic!("Invalid ImageAlignment value {}", value),
        }
    }
}

impl From<&ImageAlignment> for u32 {
    fn from(value: &ImageAlignment) -> Self {
        match value {
            ImageAlignment::ImageModeWorld => 0,
            ImageAlignment::ImageModeWrap => 1,
        }
    }
}

/// Serialize ImageAlignment as a lowercase string
impl Serialize for ImageAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            ImageAlignment::ImageModeWorld => "world",
            ImageAlignment::ImageModeWrap => "wrap",
        };
        serializer.serialize_str(value)
    }
}

/// Deserialize ImageAlignment from a lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for ImageAlignment {
    fn deserialize<D>(deserializer: D) -> Result<ImageAlignment, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ImageAlignmentVisitor;

        impl<'de> serde::de::Visitor<'de> for ImageAlignmentVisitor {
            type Value = ImageAlignment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<ImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(ImageAlignment::ImageModeWorld),
                    1 => Ok(ImageAlignment::ImageModeWrap),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"0 or 1",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<ImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "world" => Ok(ImageAlignment::ImageModeWorld),
                    "wrap" => Ok(ImageAlignment::ImageModeWrap),
                    _ => Err(serde::de::Error::unknown_variant(value, &["world", "wrap"])),
                }
            }
        }

        deserializer.deserialize_any(ImageAlignmentVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Flasher {
    pub height: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub color: Color,
    pub is_timer_enabled: bool,
    pub timer_interval: i32,
    pub name: String,
    pub image_a: String,
    pub image_b: String,
    pub alpha: i32,
    pub modulate_vs_add: f32,
    pub is_visible: bool,
    pub add_blend: bool,
    pub is_dmd: Option<bool>,
    // IDMD added in 10.2?
    pub display_texture: bool,
    pub depth_bias: f32,
    pub image_alignment: ImageAlignment,
    pub filter: Filter,
    pub filter_amount: u32,
    // FIAM
    pub light_map: Option<String>,
    // LMAP added in 10.8
    pub drag_points: Vec<DragPoint>,
    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FlasherJson {
    height: f32,
    pos_x: f32,
    pos_y: f32,
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    color: ColorJson,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    image_a: String,
    image_b: String,
    alpha: i32,
    modulate_vs_add: f32,
    is_visible: bool,
    add_blend: bool,
    is_dmd: Option<bool>,
    display_texture: bool,
    depth_bias: f32,
    image_alignment: ImageAlignment,
    filter: Filter,
    filter_amount: u32,
    light_map: Option<String>,
    drag_points: Vec<DragPoint>,
}

impl FlasherJson {
    pub fn from_flasher(flasher: &Flasher) -> Self {
        Self {
            height: flasher.height,
            pos_x: flasher.pos_x,
            pos_y: flasher.pos_y,
            rot_x: flasher.rot_x,
            rot_y: flasher.rot_y,
            rot_z: flasher.rot_z,
            color: ColorJson::from_color(&flasher.color),
            is_timer_enabled: flasher.is_timer_enabled,
            timer_interval: flasher.timer_interval,
            name: flasher.name.clone(),
            image_a: flasher.image_a.clone(),
            image_b: flasher.image_b.clone(),
            alpha: flasher.alpha,
            modulate_vs_add: flasher.modulate_vs_add,
            is_visible: flasher.is_visible,
            add_blend: flasher.add_blend,
            is_dmd: flasher.is_dmd,
            display_texture: flasher.display_texture,
            depth_bias: flasher.depth_bias,
            image_alignment: flasher.image_alignment.clone(),
            filter: flasher.filter.clone(),
            filter_amount: flasher.filter_amount,
            light_map: flasher.light_map.clone(),
            drag_points: flasher.drag_points.clone(),
        }
    }
    pub fn to_flasher(&self) -> Flasher {
        Flasher {
            height: self.height,
            pos_x: self.pos_x,
            pos_y: self.pos_y,
            rot_x: self.rot_x,
            rot_y: self.rot_y,
            rot_z: self.rot_z,
            color: self.color.to_color(),
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            name: self.name.clone(),
            image_a: self.image_a.clone(),
            image_b: self.image_b.clone(),
            alpha: self.alpha,
            modulate_vs_add: self.modulate_vs_add,
            is_visible: self.is_visible,
            add_blend: self.add_blend,
            is_dmd: self.is_dmd,
            display_texture: self.display_texture,
            depth_bias: self.depth_bias,
            image_alignment: self.image_alignment.clone(),
            filter: self.filter.clone(),
            filter_amount: self.filter_amount,
            light_map: self.light_map.clone(),
            drag_points: self.drag_points.clone(),
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

impl Serialize for Flasher {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FlasherJson::from_flasher(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Flasher {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = FlasherJson::deserialize(deserializer)?;
        Ok(json.to_flasher())
    }
}

impl BiffRead for Flasher {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut height = 50.0;
        let mut pos_x = Default::default();
        let mut pos_y = Default::default();
        let mut rot_x = Default::default();
        let mut rot_y = Default::default();
        let mut rot_z = Default::default();
        let mut color = Color::new_bgr(0xfffffff);
        let mut is_timer_enabled = Default::default();
        let mut timer_interval = Default::default();
        let mut name = Default::default();
        let mut image_a = Default::default();
        let mut image_b = Default::default();
        let mut alpha = 100;
        let mut modulate_vs_add = 0.9;
        let mut is_visible = true;
        let mut add_blend = Default::default();
        let mut is_dmd = None;
        let mut display_texture = Default::default();
        let mut depth_bias = Default::default();
        let mut image_alignment = ImageAlignment::ImageModeWrap;
        let mut filter = Filter::Overlay;
        let mut filter_amount: u32 = 100;
        let mut light_map: Option<String> = None;

        // these are shared between all items
        let mut is_locked: bool = false;
        let mut editor_layer: u32 = Default::default();
        let mut editor_layer_name: Option<String> = None;
        let mut editor_layer_visibility: Option<bool> = None;

        let mut drag_points: Vec<DragPoint> = Default::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "FHEI" => {
                    height = reader.get_f32();
                }
                "FLAX" => {
                    pos_x = reader.get_f32();
                }
                "FLAY" => {
                    pos_y = reader.get_f32();
                }
                "FROX" => {
                    rot_x = reader.get_f32();
                }
                "FROY" => {
                    rot_y = reader.get_f32();
                }
                "FROZ" => {
                    rot_z = reader.get_f32();
                }
                "COLR" => {
                    color = Color::biff_read_bgr(reader);
                }
                "TMON" => {
                    is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    timer_interval = reader.get_i32();
                }
                "NAME" => {
                    name = reader.get_wide_string();
                }
                "IMAG" => {
                    image_a = reader.get_string();
                }
                "IMAB" => {
                    image_b = reader.get_string();
                }
                "FALP" => {
                    alpha = reader.get_i32();
                }
                "MOVA" => {
                    modulate_vs_add = reader.get_f32();
                }
                "FVIS" => {
                    is_visible = reader.get_bool();
                }
                "DSPT" => {
                    display_texture = reader.get_bool();
                }
                "ADDB" => {
                    add_blend = reader.get_bool();
                }
                "IDMD" => {
                    is_dmd = Some(reader.get_bool());
                }
                "FLDB" => {
                    depth_bias = reader.get_f32();
                }
                "ALGN" => {
                    image_alignment = reader.get_u32().into();
                }
                "FILT" => {
                    filter = reader.get_u32().into();
                }
                "FIAM" => {
                    filter_amount = reader.get_u32();
                }
                "LMAP" => {
                    light_map = Some(reader.get_string());
                }
                // shared
                "LOCK" => {
                    is_locked = reader.get_bool();
                }
                "LAYR" => {
                    editor_layer = reader.get_u32();
                }
                "LANR" => {
                    editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    editor_layer_visibility = Some(reader.get_bool());
                }

                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    drag_points.push(point);
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
        Flasher {
            height,
            pos_x,
            pos_y,
            rot_x,
            rot_y,
            rot_z,
            color,
            is_timer_enabled,
            timer_interval,
            name,
            image_a,
            image_b,
            alpha,
            modulate_vs_add,
            is_visible,
            add_blend,
            is_dmd,
            display_texture,
            depth_bias,
            image_alignment,
            filter,
            filter_amount,
            light_map,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
            drag_points,
        }
    }
}

impl BiffWrite for Flasher {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_f32("FHEI", self.height);
        writer.write_tagged_f32("FLAX", self.pos_x);
        writer.write_tagged_f32("FLAY", self.pos_y);
        writer.write_tagged_f32("FROX", self.rot_x);
        writer.write_tagged_f32("FROY", self.rot_y);
        writer.write_tagged_f32("FROZ", self.rot_z);
        writer.write_tagged_with("COLR", &self.color, Color::biff_write_bgr);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("IMAG", &self.image_a);
        writer.write_tagged_string("IMAB", &self.image_b);
        writer.write_tagged_i32("FALP", self.alpha);
        writer.write_tagged_f32("MOVA", self.modulate_vs_add);
        writer.write_tagged_bool("FVIS", self.is_visible);
        writer.write_tagged_bool("DSPT", self.display_texture);
        writer.write_tagged_bool("ADDB", self.add_blend);
        if let Some(is_dmd) = self.is_dmd {
            writer.write_tagged_bool("IDMD", is_dmd);
        }
        writer.write_tagged_f32("FLDB", self.depth_bias);
        writer.write_tagged_u32("ALGN", (&self.image_alignment).into());
        writer.write_tagged_u32("FILT", (&self.filter).into());
        writer.write_tagged_u32("FIAM", self.filter_amount);
        if let Some(light_map) = &self.light_map {
            writer.write_tagged_string("LMAP", light_map);
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

        for drag_point in &self.drag_points {
            writer.write_tagged("DPNT", drag_point);
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
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::thread_rng();
        // values not equal to the defaults
        let flasher = Flasher {
            height: rng.gen(),
            pos_x: rng.gen(),
            pos_y: rng.gen(),
            rot_x: rng.gen(),
            rot_y: rng.gen(),
            rot_z: rng.gen(),
            color: Color::new_bgr(rng.gen()),
            is_timer_enabled: rng.gen(),
            timer_interval: rng.gen(),
            name: "test name".to_string(),
            image_a: "test image a".to_string(),
            image_b: "test image b".to_string(),
            alpha: rng.gen(),
            modulate_vs_add: rng.gen(),
            is_visible: rng.gen(),
            add_blend: rng.gen(),
            is_dmd: rng.gen(),
            display_texture: rng.gen(),
            depth_bias: rng.gen(),
            image_alignment: Faker.fake(),
            filter: Faker.fake(),
            filter_amount: rng.gen(),
            light_map: Some("test light map".to_string()),
            is_locked: rng.gen(),
            editor_layer: rng.gen(),
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: rng.gen(),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Flasher::biff_write(&flasher, &mut writer);
        let flasher_read = Flasher::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(flasher, flasher_read);
    }

    #[test]
    fn test_alignment_json() {
        let sizing_type = ImageAlignment::ImageModeWrap;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"wrap\"");
        let sizing_type_read: ImageAlignment = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: ImageAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(ImageAlignment::ImageModeWorld, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected `world` or `wrap`\", line: 0, column: 0)"]
    fn test_alignment_json_fail() {
        let json = serde_json::Value::from("foo");
        let _: ImageAlignment = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_filter_json() {
        let sizing_type = Filter::Overlay;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"overlay\"");
        let sizing_type_read: Filter = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: Filter = serde_json::from_value(json).unwrap();
        assert_eq!(Filter::None, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"Invalid Filter value foo, expecting \\\"none\\\", \\\"additive\\\", \\\"overlay\\\", \\\"multiply\\\" or \\\"screen\\\"\", line: 0, column: 0)"]
    fn test_filter_json_fail() {
        let json = serde_json::Value::from("foo");
        let _: Filter = serde_json::from_value(json).unwrap();
    }
}
