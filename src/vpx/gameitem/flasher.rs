use super::dragpoint::DragPoint;
use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
use crate::vpx::gameitem::select::{HasSharedAttributes, TimerDataRoot, WriteSharedAttributes};
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

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
            _ => panic!("Invalid Filter value {value}"),
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
                    "Invalid Filter value {s}, expecting \"none\", \"additive\", \"overlay\", \"multiply\" or \"screen\""
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

/// RenderMode
///
/// Default in use by vpinball is Flasher
/// Introduced in 10.8.1
#[derive(Debug, PartialEq, Clone, Dummy, Default)]
pub enum RenderMode {
    /// Custom blended images
    #[default]
    Flasher = 0,
    /// Dot matrix display (Plasma, LED, ...)
    DMD = 1,
    /// Screen (CRT, LCD, ...)
    Display = 2,
    /// Alphanumeric segment display (VFD, Plasma, LED, ...)
    AlphaSeg = 3,
}

impl From<u32> for RenderMode {
    fn from(value: u32) -> Self {
        match value {
            0 => RenderMode::Flasher,
            1 => RenderMode::DMD,
            2 => RenderMode::Display,
            3 => RenderMode::AlphaSeg,
            _ => panic!("Invalid RenderMode value {value}"),
        }
    }
}

impl From<&RenderMode> for u32 {
    fn from(value: &RenderMode) -> Self {
        match value {
            RenderMode::Flasher => 0,
            RenderMode::DMD => 1,
            RenderMode::Display => 2,
            RenderMode::AlphaSeg => 3,
        }
    }
}

impl Serialize for RenderMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            RenderMode::Flasher => "flasher",
            RenderMode::DMD => "dmd",
            RenderMode::Display => "display",
            RenderMode::AlphaSeg => "alpha_seg",
        };
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for RenderMode {
    fn deserialize<D>(deserializer: D) -> Result<RenderMode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(s) => match s.as_str() {
                "flasher" => Ok(RenderMode::Flasher),
                "dmd" => Ok(RenderMode::DMD),
                "display" => Ok(RenderMode::Display),
                "alpha_seg" => Ok(RenderMode::AlphaSeg),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid RenderMode value {s}, expecting \"flasher\", \"dmd\", \"display\" or \"alpha_seg\""
                ))),
            },
            Value::Number(n) => {
                let n = n.as_u64().unwrap();
                match n {
                    0 => Ok(RenderMode::Flasher),
                    1 => Ok(RenderMode::DMD),
                    2 => Ok(RenderMode::Display),
                    3 => Ok(RenderMode::AlphaSeg),
                    _ => Err(serde::de::Error::custom(
                        "Invalid RenderMode value, expecting 0, 1, 2 or 3",
                    )),
                }
            }
            _ => Err(serde::de::Error::custom(
                "Invalid RenderMode value, expecting string or number",
            )),
        }
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
    is_timer_enabled: bool,
    timer_interval: i32,
    pub name: String,
    pub image_a: String,
    pub image_b: String,
    pub alpha: i32,
    pub modulate_vs_add: f32,
    pub is_visible: bool,
    pub add_blend: bool,
    /// IDMD added in 10.2? Since 10.8.1 no longer written and replaced by RDMD
    pub is_dmd: Option<bool>,
    /// Since 10.8.1
    pub render_mode: Option<RenderMode>,
    /// Since 10.8.1
    pub render_style: Option<u32>,
    /// Since 10.8.1
    pub glass_roughness: Option<f32>,
    /// Since 10.8.1
    pub glass_ambient: Option<u32>,
    /// Since 10.8.1
    pub glass_pad_top: Option<f32>,
    /// Since 10.8.1
    pub glass_pad_bottom: Option<f32>,
    /// Since 10.8.1
    pub glass_pad_left: Option<f32>,
    /// Since 10.8.1
    pub glass_pad_right: Option<f32>,
    /// Since 10.8.1
    pub image_src_link: Option<String>,
    pub display_texture: bool,
    pub depth_bias: f32,
    pub image_alignment: RampImageAlignment,
    pub filter: Filter,
    pub filter_amount: u32,
    // FIAM
    pub light_map: Option<String>,
    // BGLS added in 10.8.1
    pub backglass: Option<bool>,
    // LMAP added in 10.8
    pub drag_points: Vec<DragPoint>,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}

impl Default for Flasher {
    fn default() -> Self {
        Self {
            height: 50.0,
            pos_x: 0.0,
            pos_y: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            color: Color::WHITE,
            is_timer_enabled: false,
            timer_interval: 0,
            name: "".to_string(),
            image_a: "".to_string(),
            image_b: "".to_string(),
            alpha: 100,
            modulate_vs_add: 0.9,
            is_visible: true,
            add_blend: false,
            is_dmd: None,
            render_mode: None,
            render_style: None,
            glass_roughness: None,
            glass_ambient: None,
            glass_pad_top: None,
            glass_pad_bottom: None,
            glass_pad_left: None,
            glass_pad_right: None,
            image_src_link: None,
            display_texture: false,
            depth_bias: 0.0,
            image_alignment: RampImageAlignment::Wrap,
            filter: Filter::Overlay,
            filter_amount: 100,
            light_map: None,
            backglass: None,
            drag_points: vec![],

            is_locked: false,
            editor_layer: 0,
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FlasherJson {
    height: f32,
    pos_x: f32,
    pos_y: f32,
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    color: Color,
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
    render_mode: Option<RenderMode>,
    render_style: Option<u32>,
    glass_roughness: Option<f32>,
    glass_ambient: Option<u32>,
    glass_pad_top: Option<f32>,
    glass_pad_bottom: Option<f32>,
    glass_pad_left: Option<f32>,
    glass_pad_right: Option<f32>,
    image_src_link: Option<String>,
    display_texture: bool,
    depth_bias: f32,
    image_alignment: RampImageAlignment,
    filter: Filter,
    filter_amount: u32,
    light_map: Option<String>,
    backglass: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
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
            color: flasher.color,
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
            render_mode: flasher.render_mode.clone(),
            render_style: flasher.render_style,
            glass_roughness: flasher.glass_roughness,
            glass_ambient: flasher.glass_ambient,
            glass_pad_top: flasher.glass_pad_top,
            glass_pad_bottom: flasher.glass_pad_bottom,
            glass_pad_left: flasher.glass_pad_left,
            glass_pad_right: flasher.glass_pad_right,
            image_src_link: flasher.image_src_link.clone(),
            display_texture: flasher.display_texture,
            depth_bias: flasher.depth_bias,
            image_alignment: flasher.image_alignment.clone(),
            filter: flasher.filter.clone(),
            filter_amount: flasher.filter_amount,
            light_map: flasher.light_map.clone(),
            backglass: flasher.backglass,
            part_group_name: flasher.part_group_name.clone(),
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
            color: self.color,
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
            render_mode: self.render_mode.clone(),
            render_style: self.render_style,
            glass_roughness: self.glass_roughness,
            glass_ambient: self.glass_ambient,
            glass_pad_top: self.glass_pad_top,
            glass_pad_bottom: self.glass_pad_bottom,
            glass_pad_left: self.glass_pad_left,
            glass_pad_right: self.glass_pad_right,
            image_src_link: self.image_src_link.clone(),
            display_texture: self.display_texture,
            depth_bias: self.depth_bias,
            image_alignment: self.image_alignment.clone(),
            filter: self.filter.clone(),
            filter_amount: self.filter_amount,
            light_map: self.light_map.clone(),
            backglass: self.backglass,
            part_group_name: self.part_group_name.clone(),
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

impl HasSharedAttributes for Flasher {
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

impl TimerDataRoot for Flasher {
    fn is_timer_enabled(&self) -> bool {
        self.is_timer_enabled
    }
    fn timer_interval(&self) -> i32 {
        self.timer_interval
    }
}

impl BiffRead for Flasher {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut flasher = Flasher::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "FHEI" => {
                    flasher.height = reader.get_f32();
                }
                "FLAX" => {
                    flasher.pos_x = reader.get_f32();
                }
                "FLAY" => {
                    flasher.pos_y = reader.get_f32();
                }
                "FROX" => {
                    flasher.rot_x = reader.get_f32();
                }
                "FROY" => {
                    flasher.rot_y = reader.get_f32();
                }
                "FROZ" => {
                    flasher.rot_z = reader.get_f32();
                }
                "COLR" => {
                    flasher.color = Color::biff_read(reader);
                }
                "TMON" => {
                    flasher.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    flasher.timer_interval = reader.get_i32();
                }
                "NAME" => {
                    flasher.name = reader.get_wide_string();
                }
                "IMAG" => {
                    flasher.image_a = reader.get_string();
                }
                "IMAB" => {
                    flasher.image_b = reader.get_string();
                }
                "FALP" => {
                    flasher.alpha = reader.get_i32();
                }
                "MOVA" => {
                    flasher.modulate_vs_add = reader.get_f32();
                }
                "FVIS" => {
                    flasher.is_visible = reader.get_bool();
                }
                "DSPT" => {
                    flasher.display_texture = reader.get_bool();
                }
                "ADDB" => {
                    flasher.add_blend = reader.get_bool();
                }
                "IDMD" => {
                    flasher.is_dmd = Some(reader.get_bool());
                }
                "RDMD" => {
                    flasher.render_mode = Some(reader.get_u32().into());
                }
                "RSTL" => {
                    flasher.render_style = Some(reader.get_u32());
                }
                "GRGH" => {
                    flasher.glass_roughness = Some(reader.get_f32());
                }
                "GAMB" => {
                    flasher.glass_ambient = Some(reader.get_u32());
                }
                "GTOP" => {
                    flasher.glass_pad_top = Some(reader.get_f32());
                }
                "GBOT" => {
                    flasher.glass_pad_bottom = Some(reader.get_f32());
                }
                "GLFT" => {
                    flasher.glass_pad_left = Some(reader.get_f32());
                }
                "GRHT" => {
                    flasher.glass_pad_right = Some(reader.get_f32());
                }
                "LINK" => {
                    flasher.image_src_link = Some(reader.get_string());
                }
                "FLDB" => {
                    flasher.depth_bias = reader.get_f32();
                }
                "ALGN" => {
                    flasher.image_alignment = reader.get_u32().into();
                }
                "FILT" => {
                    flasher.filter = reader.get_u32().into();
                }
                "FIAM" => {
                    flasher.filter_amount = reader.get_u32();
                }
                "LMAP" => {
                    flasher.light_map = Some(reader.get_string());
                }
                "BGLS" => {
                    flasher.backglass = Some(reader.get_bool());
                }
                // shared
                "LOCK" => {
                    flasher.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    flasher.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    flasher.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    flasher.editor_layer_visibility = Some(reader.get_bool());
                }
                "GRUP" => {
                    flasher.part_group_name = Some(reader.get_string());
                }

                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    flasher.drag_points.push(point);
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
        flasher
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
        writer.write_tagged_with("COLR", &self.color, Color::biff_write);
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
        if let Some(render_mode) = &self.render_mode {
            writer.write_tagged_u32("RDMD", render_mode.into());
        }
        if let Some(render_style) = self.render_style {
            writer.write_tagged_u32("RSTL", render_style);
        }
        if let Some(glass_roughness) = self.glass_roughness {
            writer.write_tagged_f32("GRGH", glass_roughness);
        }
        if let Some(glass_ambient) = self.glass_ambient {
            writer.write_tagged_u32("GAMB", glass_ambient);
        }
        if let Some(glass_pad_top) = self.glass_pad_top {
            writer.write_tagged_f32("GTOP", glass_pad_top);
        }
        if let Some(glass_pad_bottom) = self.glass_pad_bottom {
            writer.write_tagged_f32("GBOT", glass_pad_bottom);
        }
        if let Some(glass_pad_left) = self.glass_pad_left {
            writer.write_tagged_f32("GLFT", glass_pad_left);
        }
        if let Some(glass_pad_right) = self.glass_pad_right {
            writer.write_tagged_f32("GRHT", glass_pad_right);
        }
        if let Some(image_src_link) = &self.image_src_link {
            writer.write_tagged_string("LINK", image_src_link);
        }
        writer.write_tagged_f32("FLDB", self.depth_bias);
        writer.write_tagged_u32("ALGN", (&self.image_alignment).into());
        writer.write_tagged_u32("FILT", (&self.filter).into());
        writer.write_tagged_u32("FIAM", self.filter_amount);
        if let Some(light_map) = &self.light_map {
            writer.write_tagged_string("LMAP", light_map);
        }
        if let Some(backglass) = &self.backglass {
            writer.write_tagged_bool("BGLS", *backglass);
        }

        self.write_shared_attributes(writer);

        // many of these
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
    use crate::vpx::gameitem::tests::RandomOption;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        // values not equal to the defaults
        let flasher = Flasher {
            height: rng.random(),
            pos_x: rng.random(),
            pos_y: rng.random(),
            rot_x: rng.random(),
            rot_y: rng.random(),
            rot_z: rng.random(),
            color: Faker.fake(),
            is_timer_enabled: rng.random(),
            timer_interval: rng.random(),
            name: "test name".to_string(),
            image_a: "test image a".to_string(),
            image_b: "test image b".to_string(),
            alpha: rng.random(),
            modulate_vs_add: rng.random(),
            is_visible: rng.random(),
            add_blend: rng.random(),
            is_dmd: rng.random_option(),
            render_mode: Some(RenderMode::DMD),
            render_style: rng.random_option(),
            glass_roughness: rng.random_option(),
            glass_ambient: rng.random_option(),
            glass_pad_top: rng.random_option(),
            glass_pad_bottom: rng.random_option(),
            glass_pad_left: rng.random_option(),
            glass_pad_right: rng.random_option(),
            image_src_link: Some("test image src link".to_string()),
            display_texture: rng.random(),
            depth_bias: rng.random(),
            image_alignment: Faker.fake(),
            filter: Faker.fake(),
            filter_amount: rng.random(),
            light_map: Some("test light map".to_string()),
            backglass: rng.random_option(),
            is_locked: rng.random(),
            editor_layer: rng.random(),
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: rng.random_option(),
            part_group_name: Some("test group".to_string()),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Flasher::biff_write(&flasher, &mut writer);
        let flasher_read = Flasher::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(flasher, flasher_read);
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
