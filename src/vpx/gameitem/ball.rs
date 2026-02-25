use super::vertex3d::Vertex3D;
use crate::impl_shared_attributes;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::color::Color;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Serialize};

/// A ball (pinball) game item, typically used for captive ball effects.
///
/// # Texture and Decal Handling
///
/// VPinball has a two-layer texture system for balls:
///
/// 1. **Base texture** (`image` / `gamedata.ball_image`):
///    - This is typically an HDR environment map used for reflections
///    - Mapped using either equirectangular or spherical mapping (`spherical_mapping`)
///    - Falls back to `gamedata.ball_image` if empty
///
/// 2. **Decal/overlay texture** (`image_decal` / `gamedata.ball_image_front`):
///    - Overlaid on top of the base texture
///    - Behavior depends on `decal_mode` / `gamedata.ball_decal_mode`:
///
/// ## Decal Modes (controlled by `decal_mode`)
///
/// **When `decal_mode = false` (scratches mode):**
/// - The decal texture is treated as an alpha scratch texture
/// - Blending: additive (`ballImageColor += decalColor * decalAlpha`)
/// - Scratches affect material properties:
///   - `diffuse *= decalColor` (makes scratched areas more rough/matte)
///   - `specular *= (1 - decalColor)` (reduces specular in scratched areas)
/// - Used to add surface imperfections and wear to the ball
///
/// **When `decal_mode = true` (logo/decal mode):**
/// - The decal texture is a proper image/logo
/// - Blending: screen blend (`ScreenHDR(ballImageColor, decalColor)`)
/// - Emission is halved (`0.5 * envEmissionScale`) compared to scratches mode
/// - Used for custom ball designs, logos, or artwork
///
/// # Shader Techniques
///
/// VPinball uses different shader techniques based on settings:
/// - `RenderBall` / `RenderBall_DecalMode`: Equirectangular environment mapping
/// - `RenderBall_SphericalMap` / `RenderBall_SphericalMap_DecalMode`: Spherical UV mapping
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Ball {
    /// Name of the ball.
    ///
    /// BIFF tag: `NAME`
    pub name: String,

    /// Position of the ball in 3D space (x, y, z).
    ///
    /// BIFF tag: `VCEN`
    pub pos: Vertex3D,

    /// Radius of the ball in VP units. Default is 25.0.
    ///
    /// BIFF tag: `RADI`
    pub radius: f32,

    /// Mass of the ball, affects physics.
    ///
    /// 1 VP mass unit = 80g (mass of a standard pinball).
    /// Default is 1.0 (standard ball mass).
    ///
    /// BIFF tag: `MASS`
    pub mass: f32,

    /// Forces the ball to appear in playfield reflections even when elevated.
    ///
    /// Normally, balls on ramps, in kickers, or above the playfield are not
    /// rendered in the reflection pass to avoid visual artifacts. When this
    /// is `true`, the ball will always appear in reflections regardless of
    /// its position.
    ///
    /// BIFF tag: `FREF`
    pub force_reflection: bool,

    /// Controls how the decal texture (`image_decal`) is blended onto the ball.
    ///
    /// - `false` (scratches mode): Decal is an alpha scratch texture, blended additively.
    ///   Scratches affect both diffuse and specular properties, creating surface wear.
    /// - `true` (decal mode): Decal is a proper logo/image, blended using screen blend.
    ///
    /// Falls back to `gamedata.ball_decal_mode` for table-wide default.
    ///
    /// BIFF tag: `DCMD`
    pub decal_mode: bool,

    /// Base image/texture name for the ball (typically HDR environment map).
    ///
    /// This is used for environment reflections on the ball surface.
    /// If empty, falls back to `gamedata.ball_image`.
    /// The mapping method is controlled by `spherical_mapping`.
    ///
    /// BIFF tag: `IMAG`
    pub image: String,

    /// Decal/overlay image name for the ball.
    ///
    /// This texture is overlaid on top of the base `image`.
    /// How it's blended depends on `decal_mode`:
    /// - Scratches mode (`decal_mode = false`): Additive blend using alpha
    /// - Decal mode (`decal_mode = true`): Screen blend for logos/artwork
    ///
    /// If empty, falls back to `gamedata.ball_image_front`.
    ///
    /// BIFF tag: `DIMG`
    pub image_decal: String,

    /// Scale factor for bulb light intensity on the ball surface.
    ///
    /// Controls how strongly point lights (bulbs) affect the ball's appearance.
    /// Default is 1.0.
    ///
    /// BIFF tag: `BISC`
    pub bulb_intensity_scale: f32,

    /// Strength of the ball's reflection on the playfield surface.
    ///
    /// Controls how visible this ball is in the playfield reflection pass.
    /// Range: 0.0 (invisible in reflections) to 1.0 (full reflection strength).
    /// Default is 1.0.
    ///
    /// BIFF tag: `PFRF`
    pub playfield_reflection_strength: f32,

    /// Color tint applied to the ball material.
    ///
    /// This color multiplies the ball's base appearance. White (255,255,255)
    /// means no tinting. Can be used for colored balls (e.g., red, blue).
    ///
    /// BIFF tag: `COLR`
    pub color: Color,

    /// Controls the UV mapping method for the base texture (`image`).
    ///
    /// - `false`: Equirectangular mapping (standard HDR environment map layout)
    /// - `true`: Spherical UV mapping (direct sphere projection)
    ///
    /// Most HDR ball textures use equirectangular mapping (false).
    ///
    /// BIFF tag: `SPHR`
    pub spherical_mapping: bool,

    /// Whether this ball appears in playfield reflections.
    ///
    /// When `true`, the ball is rendered in the reflection pass.
    /// When `false`, the ball won't appear as a reflection on the playfield.
    ///
    /// BIFF tag: `REEN`
    pub is_reflection_enabled: bool,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    pub editor_layer_visibility: Option<bool>,
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Ball);

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
    #[serde(flatten)]
    pub timer: TimerData,
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
            timer: ball.timer.clone(),
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
            timer: TimerData::default(),
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
            timer: ball_json.timer.clone(),
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
                "NAME" => {
                    ball.name = reader.get_wide_string();
                }
                _ => {
                    if !ball.timer.biff_read_tag(tag_str, reader)
                        && !ball.read_shared_attribute(tag_str, reader) {
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
        self.timer.biff_write(writer);
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
            timer: TimerData { is_enabled: true, interval: 500 },
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
