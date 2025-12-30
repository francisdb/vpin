use super::vertex3d::Vertex3D;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::color::Color;
use crate::vpx::gameitem::select::{HasSharedAttributes, TimerDataRoot, WriteSharedAttributes};
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, PartialEq)]
pub struct Ball {
    pub pos: Vertex3D,
    pub radius: f32,
    pub mass: f32,
    pub force_reflection: bool,
    pub decal_mode: bool,
    pub image: String,
    pub image_decal: String,
    pub bulb_intensity_scale: f32,
    pub playfield_reflection_strength: f32,
    pub color: Color,
    pub spherical_mapping: bool,
    pub is_reflection_enabled: bool,
    is_timer_enabled: bool,
    timer_interval: i32,
    pub name: String,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    pub editor_layer_visibility: Option<bool>,
    pub part_group_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct BallJson {
    pos: Vertex3D,
    radius: f32,
    mass: f32,
    force_reflection: bool,
    decal_mode: bool,
    image: String,
    image_decal: String,
    bulb_intensity_scale: f32,
    playfield_reflection_strength: f32,
    color: Color,
    spherical_mapping: bool,
    is_reflection_enabled: bool,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl From<&Ball> for BallJson {
    fn from(ball: &Ball) -> Self {
        Self {
            pos: ball.pos,
            radius: ball.radius,
            mass: ball.mass,
            force_reflection: ball.force_reflection,
            decal_mode: ball.decal_mode,
            image: ball.image.clone(),
            image_decal: ball.image_decal.clone(),
            bulb_intensity_scale: ball.bulb_intensity_scale,
            playfield_reflection_strength: ball.playfield_reflection_strength,
            color: ball.color,
            spherical_mapping: ball.spherical_mapping,
            is_reflection_enabled: ball.is_reflection_enabled,
            is_timer_enabled: ball.is_timer_enabled,
            timer_interval: ball.timer_interval,
            name: ball.name.clone(),
            part_group_name: ball.part_group_name.clone(),
        }
    }
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            pos: Vertex3D::new(0.0, 0.0, 25.0),
            radius: 25.0,
            mass: 1.0,
            force_reflection: false,
            decal_mode: false,
            image: Default::default(),
            image_decal: Default::default(),
            bulb_intensity_scale: 1.0,
            playfield_reflection_strength: 1.0,
            color: Color::WHITE,
            spherical_mapping: false,
            is_reflection_enabled: true,
            is_timer_enabled: false,
            timer_interval: 0,
            name: Default::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl Serialize for Ball {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ball_json = BallJson::from(self);
        ball_json.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Ball {
    fn deserialize<D>(deserializer: D) -> Result<Ball, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ball_json = BallJson::deserialize(deserializer)?;
        let ball = Ball {
            pos: ball_json.pos,
            radius: ball_json.radius,
            mass: ball_json.mass,
            force_reflection: ball_json.force_reflection,
            decal_mode: ball_json.decal_mode,
            image: ball_json.image,
            image_decal: ball_json.image_decal,
            bulb_intensity_scale: ball_json.bulb_intensity_scale,
            playfield_reflection_strength: ball_json.playfield_reflection_strength,
            color: ball_json.color,
            spherical_mapping: ball_json.spherical_mapping,
            is_reflection_enabled: ball_json.is_reflection_enabled,
            is_timer_enabled: ball_json.is_timer_enabled,
            timer_interval: ball_json.timer_interval,
            name: ball_json.name,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: ball_json.part_group_name,
        };
        Ok(ball)
    }
}

impl HasSharedAttributes for Ball {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_locked(&self) -> bool {
        self.is_locked
    }

    fn editor_layer(&self) -> Option<u32> {
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

    fn set_is_locked(&mut self, locked: bool) {
        self.is_locked = locked;
    }

    fn set_editor_layer(&mut self, layer: Option<u32>) {
        self.editor_layer = layer;
    }

    fn set_editor_layer_name(&mut self, name: Option<String>) {
        self.editor_layer_name = name;
    }

    fn set_editor_layer_visibility(&mut self, visibility: Option<bool>) {
        self.editor_layer_visibility = visibility;
    }

    fn set_part_group_name(&mut self, name: Option<String>) {
        self.part_group_name = name;
    }
}

impl TimerDataRoot for Ball {
    fn is_timer_enabled(&self) -> bool {
        self.is_timer_enabled
    }
    fn timer_interval(&self) -> i32 {
        self.timer_interval
    }
}

impl BiffRead for Ball {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut ball = Ball::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    ball.pos = Vertex3D::read_unpadded(reader);
                }
                "RADI" => {
                    ball.radius = reader.get_f32();
                }
                "MASS" => {
                    ball.mass = reader.get_f32();
                }
                "FREF" => {
                    ball.force_reflection = reader.get_bool();
                }
                "DCMD" => {
                    ball.decal_mode = reader.get_bool();
                }
                "IMAG" => {
                    ball.image = reader.get_string();
                }
                "DIMG" => {
                    ball.image_decal = reader.get_string();
                }
                "BISC" => {
                    ball.bulb_intensity_scale = reader.get_f32();
                }
                "PFRF" => {
                    ball.playfield_reflection_strength = reader.get_f32();
                }
                "COLR" => {
                    ball.color = Color::biff_read(reader);
                }
                "SPHR" => {
                    ball.spherical_mapping = reader.get_bool();
                }
                "REEN" => {
                    ball.is_reflection_enabled = reader.get_bool();
                }
                "TMON" => {
                    ball.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    ball.timer_interval = reader.get_i32();
                }
                "NAME" => {
                    ball.name = reader.get_wide_string();
                }
                _ => {
                    if !ball.read_shared_attribute(tag_str, reader) {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag();
                    }
                }
            }
        }
        ball
    }
}

impl BiffWrite for Ball {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_with("VCEN", &self.pos, Vertex3D::write_unpadded);
        writer.write_tagged_f32("RADI", self.radius);
        writer.write_tagged_f32("MASS", self.mass);
        writer.write_tagged_bool("FREF", self.force_reflection);
        writer.write_tagged_bool("DCMD", self.decal_mode);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_string("DIMG", &self.image_decal);
        writer.write_tagged_f32("BISC", self.bulb_intensity_scale);
        writer.write_tagged_f32("PFRF", self.playfield_reflection_strength);
        writer.write_tagged_with("COLR", &self.color, Color::biff_write);
        writer.write_tagged_bool("SPHR", self.spherical_mapping);
        writer.write_tagged_bool("REEN", self.is_reflection_enabled);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);

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
        let ball = Ball {
            pos: Vertex3D::new(1.0, 2.0, 3.0),
            radius: 30.0,
            mass: 2.5,
            force_reflection: true,
            decal_mode: true,
            image: "test_image".to_string(),
            image_decal: "test_decal".to_string(),
            bulb_intensity_scale: 1.5,
            playfield_reflection_strength: 0.8,
            color: Color::rgb(128, 64, 32),
            spherical_mapping: true,
            is_reflection_enabled: false,
            is_timer_enabled: true,
            timer_interval: 500,
            name: "test ball".to_string(),
            is_locked: true,
            editor_layer: Some(3),
            editor_layer_name: Some("layer".to_string()),
            editor_layer_visibility: Some(true),
            part_group_name: Some("part group".to_string()),
        };
        let mut writer = BiffWriter::new();
        Ball::biff_write(&ball, &mut writer);
        let ball_read = Ball::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(ball, ball_read);
    }
}
