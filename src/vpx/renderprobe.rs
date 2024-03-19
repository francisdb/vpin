use crate::vpx::biff::{BiffRead, BiffWrite, BiffWriter};
use crate::vpx::gameitem::vertex4d::Vertex4D;
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Dummy)]
enum RenderProbeType {
    PlaneReflection = 0,
    ScreenSpaceTransparency = 1,
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
    type_: i32,
    name: String,
    roughness: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    roughness_clear: Option<u32>,
    reflection_plane: Vertex4D,
    reflection_mode: i32,
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
            type_: render_probe.type_.to_i32(),
            name: render_probe.name.clone(),
            roughness: render_probe.roughness,
            roughness_clear: render_probe.roughness_clear,
            reflection_plane: render_probe.reflection_plane,
            reflection_mode: render_probe.reflection_mode.to_i32(),
            disable_light_reflection: render_probe.disable_light_reflection,
            trailing_data,
        }
    }

    pub fn to_renderprobe(&self) -> RenderProbeWithGarbage {
        let renderprobe = RenderProbe {
            type_: RenderProbeType::from_i32(self.type_),
            name: self.name.clone(),
            roughness: self.roughness,
            roughness_clear: self.roughness_clear,
            reflection_plane: self.reflection_plane,
            reflection_mode: ReflectionMode::from_i32(self.reflection_mode),
            disable_light_reflection: self.disable_light_reflection,
        };
        RenderProbeWithGarbage {
            render_probe: renderprobe,
            trailing_data: self.trailing_data.clone().unwrap_or_default(),
        }
    }
}

impl RenderProbeType {
    fn from_i32(i: i32) -> Self {
        match i {
            0 => RenderProbeType::PlaneReflection,
            1 => RenderProbeType::ScreenSpaceTransparency,
            _ => panic!("Unknown MaterialType {}", i),
        }
    }
    fn to_i32(&self) -> i32 {
        match self {
            RenderProbeType::PlaneReflection => 0,
            RenderProbeType::ScreenSpaceTransparency => 1,
        }
    }
}

impl ReflectionMode {
    fn from_i32(i: i32) -> Self {
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
    fn to_i32(&self) -> i32 {
        match self {
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
                "TYPE" => render_probe.type_ = RenderProbeType::from_i32(reader.get_i32()),
                "NAME" => render_probe.name = reader.get_string(),
                "RBAS" => render_probe.roughness = reader.get_u32(),
                "RCLE" => render_probe.roughness_clear = Some(reader.get_u32()),
                "RPLA" => render_probe.reflection_plane = Vertex4D::biff_read(reader),
                "RMOD" => render_probe.reflection_mode = ReflectionMode::from_i32(reader.get_i32()),
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
        writer.write_tagged_i32("TYPE", self.type_.to_i32());
        writer.write_tagged_string("NAME", &self.name);
        writer.write_tagged_u32("RBAS", self.roughness);
        if let Some(rcle) = self.roughness_clear {
            writer.write_tagged_u32("RCLE", rcle);
        }
        writer.write_tagged("RPLA", &self.reflection_plane);
        writer.write_tagged_i32("RMOD", self.reflection_mode.to_i32());
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
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read() {
        let render_probe = RenderProbe {
            type_: RenderProbeType::ScreenSpaceTransparency,
            name: "test".to_string(),
            roughness: 1,
            roughness_clear: Some(2),
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: ReflectionMode::Dynamic,
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
}
