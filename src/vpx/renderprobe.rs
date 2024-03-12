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
}

#[derive(Debug, Clone, PartialEq, Dummy)]
pub struct RenderProbe {
    type_: RenderProbeType,
    name: String,
    roughness: i32,
    /// Plane equation: xyz is the normal, w is the projected distance
    reflection_plane: Vertex4D,
    reflection_mode: ReflectionMode,
    /// RLMP - added in 10.8.0 beta period
    /// Disable rendering of lightmaps in reflection render probes, needed to avoid having reflections of playfield lightmaps onto the playfield itself
    disable_light_reflection: Option<bool>,
}

/// This one is a mess and proof that the vpinball code needs unit tests
/// The last 12 bytes are a second full ENDB tag + 4 bytes of random data
/// because size calculation is done wrong.
#[derive(Debug, Clone, PartialEq, Dummy)]
pub struct RenderProbeWithGarbage {
    pub render_probe: RenderProbe,
    /// 4 bytes of random data, but needed for reproducibility
    pub(crate) trailing_data: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RenderProbeJson {
    type_: i32,
    name: String,
    roughness: i32,
    reflection_plane: Vertex4D,
    reflection_mode: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_light_reflection: Option<bool>,
    trailing_data: Option<[u8; 4]>,
}

impl RenderProbeJson {
    pub fn from_renderprobe(render_probe_with_garbage: &RenderProbeWithGarbage) -> Self {
        let render_probe = &render_probe_with_garbage.render_probe;
        Self {
            type_: render_probe.type_.to_i32(),
            name: render_probe.name.clone(),
            roughness: render_probe.roughness,
            reflection_plane: render_probe.reflection_plane,
            reflection_mode: render_probe.reflection_mode.to_i32(),
            disable_light_reflection: render_probe.disable_light_reflection,
            trailing_data: Some(render_probe_with_garbage.trailing_data.clone()),
        }
    }

    pub fn to_renderprobe(&self) -> RenderProbeWithGarbage {
        let renderprobe = RenderProbe {
            type_: RenderProbeType::from_i32(self.type_),
            name: self.name.clone(),
            roughness: self.roughness,
            reflection_plane: self.reflection_plane,
            reflection_mode: ReflectionMode::from_i32(self.reflection_mode),
            disable_light_reflection: self.disable_light_reflection,
        };
        RenderProbeWithGarbage {
            render_probe: renderprobe,
            trailing_data: self.trailing_data.unwrap_or([0; 4]),
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
        }
    }
}

impl Default for RenderProbe {
    fn default() -> Self {
        RenderProbe {
            type_: RenderProbeType::PlaneReflection,
            name: String::new(),
            roughness: 0,
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
                "RBAS" => render_probe.roughness = reader.get_i32(),
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
        writer.write_tagged_i32("RBAS", self.roughness);
        writer.write_tagged("RPLA", &self.reflection_plane);
        writer.write_tagged_i32("RMOD", self.reflection_mode.to_i32());
        if let Some(disable_light_reflection) = self.disable_light_reflection {
            writer.write_tagged_bool("RLMP", disable_light_reflection);
        }
        writer.close(true);
    }
}

/// 4 bytes long + ENDB
const ENDB_TAG: [u8; 8] = [4, 0, 0, 0, 69, 78, 68, 66];

impl BiffRead for RenderProbeWithGarbage {
    fn biff_read(reader: &mut crate::vpx::biff::BiffReader<'_>) -> Self {
        // since this is not a proper record we disable the warning
        reader.disable_warn_remaining();
        let render_probe = RenderProbe::biff_read(reader);
        let remaining = reader.get_no_remaining_update(12);
        println!("remaining {:?}", remaining);
        // first part is a full ENDB tag
        let endb_tag = &remaining[0..8];
        if endb_tag != ENDB_TAG {
            panic!("Expected ENDB tag, got {:?}", endb_tag);
        }
        let trailing_data: [u8; 4] = <[u8; 4]>::try_from(&remaining[8..12]).unwrap();
        RenderProbeWithGarbage {
            render_probe,
            trailing_data,
        }
    }
}

impl BiffWrite for RenderProbeWithGarbage {
    fn biff_write(&self, writer: &mut BiffWriter) {
        RenderProbe::biff_write(&self.render_probe, writer);
        writer.write_data(&ENDB_TAG);
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
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: ReflectionMode::Dynamic,
            disable_light_reflection: Some(false),
        };
        let render_probe_with_garbage = RenderProbeWithGarbage {
            render_probe,
            trailing_data: [1, 2, 3, 4],
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
            reflection_plane: Vertex4D::new(1.0, 2.0, 3.0, 4.0),
            reflection_mode: ReflectionMode::Dynamic,
            disable_light_reflection: Some(true),
        };
        let render_probe_with_garbage = RenderProbeWithGarbage {
            render_probe,
            trailing_data: [1, 2, 3, 4],
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
