use super::{dragpoint::DragPoint, vertex2d::Vertex2D};
use crate::vpx::gameitem::select::{HasSharedAttributes, WriteSharedAttributes};
use crate::vpx::json::F32WithNanInf;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum ShadowMode {
    None = 0,
    RaytracedBallShadows = 1,
}

impl From<u32> for ShadowMode {
    fn from(value: u32) -> Self {
        match value {
            0 => ShadowMode::None,
            1 => ShadowMode::RaytracedBallShadows,
            _ => panic!("Unknown value for ShadowMode: {value}"),
        }
    }
}

impl From<&ShadowMode> for u32 {
    fn from(value: &ShadowMode) -> Self {
        match value {
            ShadowMode::None => 0,
            ShadowMode::RaytracedBallShadows => 1,
        }
    }
}

/// Serialize to lowercase string
impl Serialize for ShadowMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            ShadowMode::None => "none",
            ShadowMode::RaytracedBallShadows => "raytraced_ball_shadows",
        };
        serializer.serialize_str(value)
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for ShadowMode {
    fn deserialize<D>(deserializer: D) -> Result<ShadowMode, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ShadowModeVisitor;

        impl serde::de::Visitor<'_> for ShadowModeVisitor {
            type Value = ShadowMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<ShadowMode, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(ShadowMode::None),
                    1 => Ok(ShadowMode::RaytracedBallShadows),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"0 or 1",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<ShadowMode, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "none" => Ok(ShadowMode::None),
                    "raytraced_ball_shadows" => Ok(ShadowMode::RaytracedBallShadows),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["none", "raytraced_ball_shadows"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(ShadowModeVisitor)
    }
}

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum Fader {
    None = 0,
    Linear = 1,
    Incandescent = 2,
}

impl From<u32> for Fader {
    fn from(value: u32) -> Self {
        match value {
            0 => Fader::None,
            1 => Fader::Linear,
            2 => Fader::Incandescent,
            _ => panic!("Unknown value for Fader: {value}"),
        }
    }
}

impl From<&Fader> for u32 {
    fn from(value: &Fader) -> Self {
        match value {
            Fader::None => 0,
            Fader::Linear => 1,
            Fader::Incandescent => 2,
        }
    }
}

/// Serialize to lowercase string
impl Serialize for Fader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Fader::None => "none",
            Fader::Linear => "linear",
            Fader::Incandescent => "incandescent",
        };
        serializer.serialize_str(value)
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for Fader {
    fn deserialize<D>(deserializer: D) -> Result<Fader, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FaderVisitor;

        impl serde::de::Visitor<'_> for FaderVisitor {
            type Value = Fader;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Fader, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(Fader::None),
                    1 => Ok(Fader::Linear),
                    2 => Ok(Fader::Incandescent),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"0, 1 or 2",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Fader, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "none" => Ok(Fader::None),
                    "linear" => Ok(Fader::Linear),
                    "incandescent" => Ok(Fader::Incandescent),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["none", "linear", "incandescent"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(FaderVisitor)
    }
}

#[derive(Debug, PartialEq, Dummy)]
pub struct Light {
    pub center: Vertex2D,    // VCEN
    pub height: Option<f32>, // HGHT added in 10.8
    pub falloff_radius: f32, // RADI
    pub falloff_power: f32,  // FAPO
    /// STAT deprecated, planned or removal in to 10.9+
    /// m_d.m_state == 0.f ? 0 : (m_d.m_state == 2.f ? 2 : 1);
    pub state_u32: u32,
    pub state: Option<f32>,                 // STTF added in 10.8
    pub color: Color,                       // COLR
    pub color2: Color,                      // COL2
    pub is_timer_enabled: bool,             // TMON
    pub timer_interval: i32,                // TMIN
    pub blink_pattern: String,              // BPAT
    pub off_image: String,                  // IMG1
    pub blink_interval: u32,                // BINT
    pub intensity: f32,                     // BWTH
    pub transmission_scale: f32,            // TRMS
    pub surface: String,                    // SURF
    pub name: String,                       // NAME
    pub is_backglass: bool,                 // BGLS
    pub depth_bias: f32,                    // LIDB
    pub fade_speed_up: f32,                 // FASP, can be Inf (Dr. Dude (Bally 1990)v3.0.vpx)
    pub fade_speed_down: f32,               // FASD, can be Inf (Dr. Dude (Bally 1990)v3.0.vpx)
    pub is_bulb_light: bool,                // BULT
    pub is_image_mode: bool,                // IMMO
    pub show_bulb_mesh: bool,               // SHBM
    pub has_static_bulb_mesh: Option<bool>, // STBM (added in 10.?)
    pub show_reflection_on_ball: bool,      // SHRB
    pub mesh_radius: f32,                   // BMSC
    pub bulb_modulate_vs_add: f32,          // BMVA
    pub bulb_halo_height: f32,              // BHHI
    pub shadows: Option<ShadowMode>,        // SHDW added in 10.8
    pub fader: Option<Fader>,               // FADE added in 10.8
    pub visible: Option<bool>,              // VSBL added in 10.8

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,

    // last
    pub drag_points: Vec<DragPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LightJson {
    center: Vertex2D,
    height: Option<f32>,
    falloff_radius: f32,
    falloff_power: f32,
    state_u32: u32,
    state: Option<f32>,
    color: Color,
    color2: Color,
    is_timer_enabled: bool,
    timer_interval: i32,
    blink_pattern: String,
    off_image: String,
    blink_interval: u32,
    intensity: f32,
    transmission_scale: f32,
    surface: String,
    name: String,
    is_backglass: bool,
    depth_bias: f32,
    fade_speed_up: F32WithNanInf,
    fade_speed_down: F32WithNanInf,
    is_bulb_light: bool,
    is_image_mode: bool,
    show_bulb_mesh: bool,
    has_static_bulb_mesh: Option<bool>,
    show_reflection_on_ball: bool,
    mesh_radius: f32,
    bulb_modulate_vs_add: f32,
    bulb_halo_height: f32,
    shadows: Option<ShadowMode>,
    fader: Option<Fader>,
    visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
    drag_points: Vec<DragPoint>,
}

impl LightJson {
    fn from_light(light: &Light) -> Self {
        Self {
            center: light.center,
            height: light.height,
            falloff_radius: light.falloff_radius,
            falloff_power: light.falloff_power,
            state_u32: light.state_u32,
            state: light.state,
            color: light.color,
            color2: light.color2,
            is_timer_enabled: light.is_timer_enabled,
            timer_interval: light.timer_interval,
            blink_pattern: light.blink_pattern.clone(),
            off_image: light.off_image.clone(),
            blink_interval: light.blink_interval,
            intensity: light.intensity,
            transmission_scale: light.transmission_scale,
            surface: light.surface.clone(),
            name: light.name.clone(),
            is_backglass: light.is_backglass,
            depth_bias: light.depth_bias,
            fade_speed_up: light.fade_speed_up.into(),
            fade_speed_down: light.fade_speed_down.into(),
            is_bulb_light: light.is_bulb_light,
            is_image_mode: light.is_image_mode,
            show_bulb_mesh: light.show_bulb_mesh,
            has_static_bulb_mesh: light.has_static_bulb_mesh,
            show_reflection_on_ball: light.show_reflection_on_ball,
            mesh_radius: light.mesh_radius,
            bulb_modulate_vs_add: light.bulb_modulate_vs_add,
            bulb_halo_height: light.bulb_halo_height,
            shadows: light.shadows.clone(),
            fader: light.fader.clone(),
            visible: light.visible,
            part_group_name: light.part_group_name.clone(),
            drag_points: light.drag_points.clone(),
        }
    }

    fn to_light(&self) -> Light {
        Light {
            center: self.center,
            height: self.height,
            falloff_radius: self.falloff_radius,
            falloff_power: self.falloff_power,
            state_u32: self.state_u32,
            state: self.state,
            color: self.color,
            color2: self.color2,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            blink_pattern: self.blink_pattern.clone(),
            off_image: self.off_image.clone(),
            blink_interval: self.blink_interval,
            intensity: self.intensity,
            transmission_scale: self.transmission_scale,
            surface: self.surface.clone(),
            name: self.name.clone(),
            is_backglass: self.is_backglass,
            depth_bias: self.depth_bias,
            fade_speed_up: self.fade_speed_up.into(),
            fade_speed_down: self.fade_speed_down.into(),
            is_bulb_light: self.is_bulb_light,
            is_image_mode: self.is_image_mode,
            show_bulb_mesh: self.show_bulb_mesh,
            has_static_bulb_mesh: self.has_static_bulb_mesh,
            show_reflection_on_ball: self.show_reflection_on_ball,
            mesh_radius: self.mesh_radius,
            bulb_modulate_vs_add: self.bulb_modulate_vs_add,
            bulb_halo_height: self.bulb_halo_height,
            shadows: self.shadows.clone(),
            fader: self.fader.clone(),
            visible: self.visible,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
            drag_points: self.drag_points.clone(),
        }
    }
}

impl Serialize for Light {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LightJson::from_light(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Light {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let light_json = LightJson::deserialize(deserializer)?;
        Ok(light_json.to_light())
    }
}

impl Default for Light {
    fn default() -> Self {
        let name = Default::default();
        let height: Option<f32> = None;
        let center: Vertex2D = Default::default();
        let falloff_radius: f32 = Default::default();
        let falloff_power: f32 = Default::default();
        let status: u32 = Default::default();
        let state: Option<f32> = None;
        // Default to 2700K incandescent bulb
        let color: Color = Color::rgb(255, 169, 87);
        // Default to 2700K incandescent bulb (burst is useless since VPX is HDR)
        let color2: Color = Color::rgb(255, 169, 87);
        let is_timer_enabled: bool = false;
        let timer_interval: i32 = Default::default();
        let blink_pattern: String = "10".to_owned();
        let off_image: String = Default::default();
        let blink_interval: u32 = Default::default();
        let intensity: f32 = 1.0;
        let transmission_scale: f32 = 0.5;
        let surface: String = Default::default();
        let is_backglass: bool = false;
        let depth_bias: f32 = Default::default();
        let fade_speed_up: f32 = 0.2;
        let fade_speed_down: f32 = 0.2;
        let is_bulb_light: bool = false;
        let is_image_mode: bool = false;
        let show_bulb_mesh: bool = false;
        let has_static_bulb_mesh: Option<bool> = None; //true;
        let show_reflection_on_ball: bool = true;
        let mesh_radius: f32 = 20.0;
        let bulb_modulate_vs_add: f32 = 0.9;
        let bulb_halo_height: f32 = 28.0;
        let shadows: Option<ShadowMode> = None;
        let fader: Option<Fader> = None;
        let visible: Option<bool> = None;

        // these are shared between all items
        let is_locked: bool = false;
        let editor_layer: u32 = Default::default();
        let editor_layer_name: Option<String> = None;
        let editor_layer_visibility: Option<bool> = None;
        let part_group_name: Option<String> = None;
        Self {
            center,
            height,
            falloff_radius,
            falloff_power,
            state_u32: status,
            state,
            color,
            color2,
            is_timer_enabled,
            timer_interval,
            blink_pattern,
            off_image,
            blink_interval,
            intensity,
            transmission_scale,
            surface,
            name,
            is_backglass,
            depth_bias,
            fade_speed_up,
            fade_speed_down,
            is_bulb_light,
            is_image_mode,
            show_bulb_mesh,
            has_static_bulb_mesh,
            show_reflection_on_ball,
            mesh_radius,
            bulb_modulate_vs_add,
            bulb_halo_height,
            shadows,
            fader,
            visible,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
            part_group_name,
            drag_points: Vec::new(),
        }
    }
}

impl HasSharedAttributes for Light {
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

impl BiffRead for Light {
    fn biff_read(reader: &mut BiffReader<'_>) -> Light {
        let mut light = Light::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => light.center = Vertex2D::biff_read(reader),
                "HGHT" => light.height = Some(reader.get_f32()),
                "RADI" => light.falloff_radius = reader.get_f32(),
                "FAPO" => light.falloff_power = reader.get_f32(),
                "STAT" => light.state_u32 = reader.get_u32(),
                "STTF" => light.state = Some(reader.get_f32()),
                "COLR" => light.color = Color::biff_read(reader),
                "COL2" => light.color2 = Color::biff_read(reader),
                "TMON" => light.is_timer_enabled = reader.get_bool(),
                "TMIN" => light.timer_interval = reader.get_i32(),
                "BPAT" => light.blink_pattern = reader.get_string(),
                "IMG1" => light.off_image = reader.get_string(),
                "BINT" => light.blink_interval = reader.get_u32(),
                "BWTH" => light.intensity = reader.get_f32(),
                "TRMS" => light.transmission_scale = reader.get_f32(),
                "SURF" => light.surface = reader.get_string(),
                "NAME" => light.name = reader.get_wide_string(),
                // shared
                "LOCK" => light.is_locked = reader.get_bool(),
                "LAYR" => light.editor_layer = reader.get_u32(),
                "LANR" => light.editor_layer_name = Some(reader.get_string()),
                "LVIS" => light.editor_layer_visibility = Some(reader.get_bool()),

                "BGLS" => light.is_backglass = reader.get_bool(),
                "LIDB" => light.depth_bias = reader.get_f32(),
                "FASP" => light.fade_speed_up = reader.get_f32(),
                "FASD" => light.fade_speed_down = reader.get_f32(),
                "BULT" => light.is_bulb_light = reader.get_bool(),
                "IMMO" => light.is_image_mode = reader.get_bool(),
                "SHBM" => light.show_bulb_mesh = reader.get_bool(),
                "STBM" => light.has_static_bulb_mesh = Some(reader.get_bool()),
                "SHRB" => light.show_reflection_on_ball = reader.get_bool(),
                "BMSC" => light.mesh_radius = reader.get_f32(),
                "BMVA" => light.bulb_modulate_vs_add = reader.get_f32(),
                "BHHI" => light.bulb_halo_height = reader.get_f32(),
                "SHDW" => light.shadows = Some(reader.get_u32().into()),
                "FADE" => light.fader = Some(reader.get_u32().into()),
                "VSBL" => light.visible = Some(reader.get_bool()),
                "GRUP" => {
                    light.part_group_name = Some(reader.get_string());
                }
                // many of these
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    light.drag_points.push(point);
                }
                other => {
                    println!(
                        "Unknown tag {} for {}",
                        other,
                        std::any::type_name::<Self>()
                    );
                    reader.skip_tag();
                }
            }
        }
        light
    }
}

impl BiffWrite for Light {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        // write all fields like n the read
        writer.write_tagged("VCEN", &self.center);
        if let Some(height) = self.height {
            writer.write_tagged_f32("HGHT", height);
        }
        writer.write_tagged_f32("RADI", self.falloff_radius);
        writer.write_tagged_f32("FAPO", self.falloff_power);
        writer.write_tagged_u32("STAT", self.state_u32);
        if let Some(state) = self.state {
            writer.write_tagged_f32("STTF", state);
        }
        writer.write_tagged_with("COLR", &self.color, Color::biff_write);
        writer.write_tagged_with("COL2", &self.color2, Color::biff_write);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_string("BPAT", &self.blink_pattern);
        writer.write_tagged_string("IMG1", &self.off_image);
        writer.write_tagged_u32("BINT", self.blink_interval);
        writer.write_tagged_f32("BWTH", self.intensity);
        writer.write_tagged_f32("TRMS", self.transmission_scale);

        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);

        writer.write_tagged_bool("BGLS", self.is_backglass);
        writer.write_tagged_f32("LIDB", self.depth_bias);
        writer.write_tagged_f32("FASP", self.fade_speed_up);
        writer.write_tagged_f32("FASD", self.fade_speed_down);
        writer.write_tagged_bool("BULT", self.is_bulb_light);
        writer.write_tagged_bool("IMMO", self.is_image_mode);
        writer.write_tagged_bool("SHBM", self.show_bulb_mesh);
        if let Some(stbm) = self.has_static_bulb_mesh {
            writer.write_tagged_bool("STBM", stbm);
        }
        writer.write_tagged_bool("SHRB", self.show_reflection_on_ball);
        writer.write_tagged_f32("BMSC", self.mesh_radius);
        writer.write_tagged_f32("BMVA", self.bulb_modulate_vs_add);
        writer.write_tagged_f32("BHHI", self.bulb_halo_height);
        if let Some(shadows) = &self.shadows {
            writer.write_tagged_u32("SHDW", shadows.into());
        }
        if let Some(fader) = &self.fader {
            writer.write_tagged_u32("FADE", fader.into());
        }
        if let Some(visible) = self.visible {
            writer.write_tagged_bool("VSBL", visible);
        }

        self.write_shared_attributes(writer);

        // many of these
        for point in &self.drag_points {
            writer.write_tagged("DPNT", point);
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
        let light = Light {
            center: Vertex2D::new(1.0, 2.0),
            height: Some(3.0),
            falloff_radius: 25.0,
            falloff_power: 3.0,
            state_u32: 4,
            state: Some(5.0),
            color: Faker.fake(),
            color2: Faker.fake(),
            is_timer_enabled: true,
            timer_interval: 7,
            blink_pattern: "test pattern".to_string(),
            off_image: "test image".to_string(),
            blink_interval: 8,
            intensity: 9.0,
            transmission_scale: 10.0,
            surface: "test surface".to_string(),
            name: "test name".to_string(),
            is_backglass: false,
            depth_bias: 11.0,
            fade_speed_up: 12.0,
            fade_speed_down: 13.0,
            is_bulb_light: true,
            is_image_mode: true,
            show_bulb_mesh: false,
            has_static_bulb_mesh: Some(false),
            show_reflection_on_ball: false,
            mesh_radius: 14.0,
            bulb_modulate_vs_add: 15.0,
            bulb_halo_height: 16.0,
            shadows: Faker.fake(),
            fader: Faker.fake(),
            visible: Some(true),
            is_locked: false,
            editor_layer: 17,
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(true),
            part_group_name: Some("test group".to_string()),
            drag_points: vec![DragPoint::default()],
        };
        let mut writer = BiffWriter::new();
        Light::biff_write(&light, &mut writer);
        let light_read = Light::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(light, light_read);
    }

    #[test]
    fn test_fader_json() {
        let sizing_type = Fader::Linear;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"linear\"");
        let sizing_type_read: Fader = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(2);
        let sizing_type_read: Fader = serde_json::from_value(json).unwrap();
        assert_eq!(Fader::Incandescent, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `none`, `linear`, `incandescent`\", line: 0, column: 0)"]
    fn test_fader_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: Fader = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_shadow_mode_json() {
        let sizing_type = ShadowMode::RaytracedBallShadows;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"raytraced_ball_shadows\"");
        let sizing_type_read: ShadowMode = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: ShadowMode = serde_json::from_value(json).unwrap();
        assert_eq!(ShadowMode::None, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected `none` or `raytraced_ball_shadows`\", line: 0, column: 0)"]
    fn test_shadow_mode_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: ShadowMode = serde_json::from_value(json).unwrap();
    }
}
