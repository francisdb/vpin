use crate::vpx::biff::{BiffRead, BiffWrite, BiffWriter};
use crate::vpx::gameitem::vertex4d::Vertex4D;
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Dummy)]
enum RenderProbeType {
    PlaneReflection = 0,
    ScreenSpaceTransparency = 1,
}

impl From<u32> for RenderProbeType {
    fn from(i: u32) -> Self {
        match i {
            0 => RenderProbeType::PlaneReflection,
            1 => RenderProbeType::ScreenSpaceTransparency,
            _ => panic!("Unknown MaterialType {}", i),
        }
    }
}

impl From<&RenderProbeType> for u32 {
    fn from(r: &RenderProbeType) -> Self {
        match r {
            RenderProbeType::PlaneReflection => 0,
            RenderProbeType::ScreenSpaceTransparency => 1,
        }
    }
}

/// Serialize as lowercase string
impl<'de> Deserialize<'de> for RenderProbeType {
    fn deserialize<D>(deserializer: D) -> Result<RenderProbeType, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct RenderProbeTypeVisitor;
        impl<'de> serde::de::Visitor<'de> for RenderProbeTypeVisitor {
            type Value = RenderProbeType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a number")
            }

            fn visit_u64<E>(self, value: u64) -> Result<RenderProbeType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(RenderProbeType::PlaneReflection),
                    1 => Ok(RenderProbeType::ScreenSpaceTransparency),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number between 0 and 1",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<RenderProbeType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "plane_reflection" => Ok(RenderProbeType::PlaneReflection),
                    "screen_space_transparency" => Ok(RenderProbeType::ScreenSpaceTransparency),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["plane_reflection", "screen_space_transparency"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(RenderProbeTypeVisitor)
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl Serialize for RenderProbeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let s = match self {
            RenderProbeType::PlaneReflection => "plane_reflection",
            RenderProbeType::ScreenSpaceTransparency => "screen_space_transparency",
        };
        serializer.serialize_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Dummy)]
enum ReflectionMode {
    /// No reflections
    None = 0,
    /// Only balls reflections
    Balls = 1,
    /// Only static (prerendered) reflections
    Static = 2,
    /// Static reflections and balls, without depth sync (static or dynamic reflection may be rendered while they should be occluded)
    StaticNBalls = 3,
    /// Static and dynamic reflections, without depth sync (static or dynamic reflection may be rendered while they should be occluded)
    StaticNDynamic = 4,
    /// All reflections are dynamic allowing for correct occlusion between them at the cost of performance (static are still prerendered)
    Dynamic = 5,
    /// Unknown mode - seen on a blank table created a while ago
    Unknown = 6,
}

impl From<u32> for ReflectionMode {
    fn from(i: u32) -> Self {
        match i {
            0 => ReflectionMode::None,
            1 => ReflectionMode::Balls,
            2 => ReflectionMode::Static,
            3 => ReflectionMode::StaticNBalls,
            4 => ReflectionMode::StaticNDynamic,
            5 => ReflectionMode::Dynamic,
            6 => ReflectionMode::Unknown,
            _ => panic!("Unknown ReflectionMode {}", i),
        }
    }
}

impl From<&ReflectionMode> for u32 {
    fn from(r: &ReflectionMode) -> Self {
        match r {
            ReflectionMode::None => 0,
            ReflectionMode::Balls => 1,
            ReflectionMode::Static => 2,
            ReflectionMode::StaticNBalls => 3,
            ReflectionMode::StaticNDynamic => 4,
            ReflectionMode::Dynamic => 5,
            ReflectionMode::Unknown => 6,
        }
    }
}

/// Serialize as lowercase string
impl Serialize for ReflectionMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let s = match self {
            ReflectionMode::None => "none",
            ReflectionMode::Balls => "balls",
            ReflectionMode::Static => "static",
            ReflectionMode::StaticNBalls => "static_and_balls",
            ReflectionMode::StaticNDynamic => "static_and_dynamic",
            ReflectionMode::Dynamic => "dynamic",
            ReflectionMode::Unknown => "unknown",
        };
        serializer.serialize_str(s)
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for ReflectionMode {
    fn deserialize<D>(deserializer: D) -> Result<ReflectionMode, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct ReflectionModeVisitor;
        impl<'de> serde::de::Visitor<'de> for ReflectionModeVisitor {
            type Value = ReflectionMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a number")
            }

            fn visit_str<E>(self, value: &str) -> Result<ReflectionMode, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "none" => Ok(ReflectionMode::None),
                    "balls" => Ok(ReflectionMode::Balls),
                    "static" => Ok(ReflectionMode::Static),
                    "static_and_balls" => Ok(ReflectionMode::StaticNBalls),
                    "static_and_dynamic" => Ok(ReflectionMode::StaticNDynamic),
                    "dynamic" => Ok(ReflectionMode::Dynamic),
                    "unknown" => Ok(ReflectionMode::Unknown),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "none",
                            "balls",
                            "static",
                            "static_and_balls",
                            "static_and_dynamic",
                            "dynamic",
                            "unknown",
                        ],
                    )),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<ReflectionMode, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(ReflectionMode::None),
                    1 => Ok(ReflectionMode::Balls),
                    2 => Ok(ReflectionMode::Static),
                    3 => Ok(ReflectionMode::StaticNBalls),
                    4 => Ok(ReflectionMode::StaticNDynamic),
                    5 => Ok(ReflectionMode::Dynamic),
                    6 => Ok(ReflectionMode::Unknown),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number between 0 and 6",
                    )),
                }
            }
        }
        deserializer.deserialize_any(ReflectionModeVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Dummy)]
pub struct RenderProbe {
    type_: RenderProbeType,
    name: String,
    roughness: u32,
    /// Old stuff, not used anymore, but still in the file
    roughness_clear: Option<u32>,
    /// Plane equation: xyz is the normal, w is the projected distance
    reflection_plane: Vertex4D,
    reflection_mode: ReflectionMode,
    /// RLMP - added in 10.8.0 beta period
    /// Disable rendering of lightmaps in reflection render probes, needed to avoid having reflections of playfield lightmaps onto the playfield itself
    disable_light_reflection: Option<bool>,
}

/// This one is a mess and proof that the vpinball code needs unit tests
/// `trailing_data` can be:
///  * a second full ENDB tag (8 bytes) + 4 bytes of random data
///    because size calculation is done wrong.
///  * empty for some tables created during a specific period of 10.8.0 development
#[derive(Debug, Clone, PartialEq, Dummy)]
pub struct RenderProbeWithGarbage {
    pub render_probe: RenderProbe,
    pub(crate) trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RenderProbeJson {
    type_: RenderProbeType,
    name: String,
    roughness: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    roughness_clear: Option<u32>,
    reflection_plane: Vertex4D,
    reflection_mode: ReflectionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_light_reflection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trailing_data: Option<Vec<u8>>,
}

impl RenderProbeJson {
    pub fn from_renderprobe(render_probe_with_garbage: &RenderProbeWithGarbage) -> Self {
        let render_probe = &render_probe_with_garbage.render_probe;
        let trailing_data = if render_probe_with_garbage.trailing_data.is_empty() {
            None
        } else {
            Some(render_probe_with_garbage.trailing_data.clone())
        };
        Self {
            type_: render_probe.type_.clone(),
            name: render_probe.name.clone(),
            roughness: render_probe.roughness,
            roughness_clear: render_probe.roughness_clear,
            reflection_plane: render_probe.reflection_plane,
            reflection_mode: render_probe.reflection_mode.clone(),
            disable_light_reflection: render_probe.disable_light_reflection,
            trailing_data,
        }
    }

    pub fn to_renderprobe(&self) -> RenderProbeWithGarbage {
        let renderprobe = RenderProbe {
            type_: self.type_.clone(),
            name: self.name.clone(),
            roughness: self.roughness,
            roughness_clear: self.roughness_clear,
            reflection_plane: self.reflection_plane,
            reflection_mode: self.reflection_mode.clone(),
            disable_light_reflection: self.disable_light_reflection,
        };
        RenderProbeWithGarbage {
            render_probe: renderprobe,
            trailing_data: self.trailing_data.clone().unwrap_or_default(),
        }
    }
}

impl Default for RenderProbe {
    fn default() -> Self {
        RenderProbe {
            type_: RenderProbeType::PlaneReflection,
            name: String::new(),
            roughness: 0,
            roughness_clear: None,
            reflection_plane: Vertex4D::default(),
            reflection_mode: ReflectionMode::None,
            disable_light_reflection: None, //false,
        }
    }
}

impl BiffRead for RenderProbe {
    fn biff_read(reader: &mut crate::vpx::biff::BiffReader<'_>) -> Self {
        let mut render_probe = RenderProbe::default();
        loop {
            reader.next(crate::vpx::biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "TYPE" => render_probe.type_ = reader.get_u32().into(),
                "NAME" => render_probe.name = reader.get_string(),
                "RBAS" => render_probe.roughness = reader.get_u32(),
                "RCLE" => render_probe.roughness_clear = Some(reader.get_u32()),
                "RPLA" => render_probe.reflection_plane = Vertex4D::biff_read(reader),
                "RMOD" => render_probe.reflection_mode = reader.get_u32().into(),
                "RLMP" => render_probe.disable_light_reflection = Some(reader.get_bool()),
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
        render_probe
    }
}

impl BiffWrite for RenderProbe {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_tagged_u32("TYPE", (&self.type_).into());
        writer.write_tagged_string("NAME", &self.name);
        writer.write_tagged_u32("RBAS", self.roughness);
        if let Some(rcle) = self.roughness_clear {
            writer.write_tagged_u32("RCLE", rcle);
        }
        writer.write_tagged("RPLA", &self.reflection_plane);
        writer.write_tagged_u32("RMOD", (&self.reflection_mode).into());
        if let Some(disable_light_reflection) = self.disable_light_reflection {
            writer.write_tagged_bool("RLMP", disable_light_reflection);
        }
        writer.close(true);
    }
}

impl BiffRead for RenderProbeWithGarbage {
    fn biff_read(reader: &mut crate::vpx::biff::BiffReader<'_>) -> Self {
        // since this is not a proper record we disable the warning
        reader.disable_warn_remaining();
        let render_probe = RenderProbe::biff_read(reader);
        let trailing_data = reader.get_remaining().to_vec();
        RenderProbeWithGarbage {
            render_probe,
            trailing_data,
        }
    }
}

impl BiffWrite for RenderProbeWithGarbage {
    fn biff_write(&self, writer: &mut BiffWriter) {
        RenderProbe::biff_write(&self.render_probe, writer);
        writer.write_data(&self.trailing_data);
    }
}

// tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::biff::BiffReader;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read() {
        let render_probe = RenderProbe {
            type_: Faker.fake(),
            name: "test".to_string(),
            roughness: 1,
            roughness_clear: Some(2),
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: Faker.fake(),
            disable_light_reflection: Some(false),
        };
        let mut writer = BiffWriter::new();
        RenderProbe::biff_write(&render_probe, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let render_probe_read = RenderProbe::biff_read(&mut reader);
        assert_eq!(render_probe, render_probe_read);
    }

    #[test]
    fn test_write_read_with_garbage() {
        let render_probe = RenderProbe {
            type_: RenderProbeType::ScreenSpaceTransparency,
            name: "test".to_string(),
            roughness: 1,
            roughness_clear: Some(2),
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: ReflectionMode::Dynamic,
            disable_light_reflection: Some(false),
        };
        let render_probe_with_garbage = RenderProbeWithGarbage {
            render_probe,
            trailing_data: vec![1, 2, 3, 4],
        };
        let mut writer = BiffWriter::new();
        RenderProbeWithGarbage::biff_write(&render_probe_with_garbage, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let render_probe_with_garbage_read = RenderProbeWithGarbage::biff_read(&mut reader);
        assert_eq!(render_probe_with_garbage, render_probe_with_garbage_read);
    }

    #[test]
    fn test_json() {
        let render_probe = RenderProbe {
            type_: RenderProbeType::ScreenSpaceTransparency,
            name: "test".to_string(),
            roughness: 1,
            roughness_clear: Some(2),
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: ReflectionMode::Dynamic,
            disable_light_reflection: Some(true),
        };
        let render_probe_with_garbage = RenderProbeWithGarbage {
            render_probe,
            trailing_data: vec![1, 2, 3, 4],
        };
        let json = serde_json::to_string(&RenderProbeJson::from_renderprobe(
            &render_probe_with_garbage,
        ))
        .unwrap();
        let render_probe_with_garbage_read =
            RenderProbeJson::to_renderprobe(&serde_json::from_str(&json).unwrap());
        assert_eq!(render_probe_with_garbage, render_probe_with_garbage_read);
    }

    #[test]
    fn test_reflection_mode_json() {
        let sizing_type = ReflectionMode::Balls;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"balls\"");
        let sizing_type_read: ReflectionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(3);
        let sizing_type_read: ReflectionMode = serde_json::from_value(json).unwrap();
        assert_eq!(ReflectionMode::StaticNBalls, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `none`, `balls`, `static`, `static_and_balls`, `static_and_dynamic`, `dynamic`, `unknown`\", line: 0, column: 0)"]
    fn test_reflection_mode_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: ReflectionMode = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_render_probe_type_json() {
        let sizing_type = RenderProbeType::ScreenSpaceTransparency;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"screen_space_transparency\"");
        let sizing_type_read: RenderProbeType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RenderProbeType = serde_json::from_value(json).unwrap();
        assert_eq!(RenderProbeType::PlaneReflection, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected `plane_reflection` or `screen_space_transparency`\", line: 0, column: 0)"]
    fn test_render_probe_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: RenderProbeType = serde_json::from_value(json).unwrap();
    }
}
