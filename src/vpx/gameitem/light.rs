//! Light game item — represents a light source on the playfield.
//!
//! # Render Modes
//!
//! Lights have three modes, controlled by [`Light::visible`] and [`Light::is_bulb_light`]:
//!
//! | UI Mode     | `visible` | `is_bulb_light` | Description |
//! |-------------|-----------|-----------------|-------------|
//! | **Hidden**  | `false`   | (any)           | Not rendered at all. Still exists for scripting. |
//! | **Classic** | `true`    | `false`         | Flat lightmap polygon on the playfield surface. |
//! | **Halo**    | `true`    | `true`          | Radial glow effect blended over the playfield. |
//!
//! ## Classic Mode
//!
//! The light is rendered as a flat polygon (defined by drag points) on the surface
//! of the playfield (or a wall/ramp if [`Light::surface`] is set). It uses VPinball's
//! `basicShader` with the `light_with_texture` or `light_without_texture` technique
//! from `ClassicLightShader.hlsl`.
//!
//! When [`Light::image`] is set, the texture is applied using **table-space UVs**
//! (`x / table_width`, `y / table_height`), meaning the light polygon shows a
//! cutout of the table texture. The light color modulates this texture via
//! overlay/screen blending based on intensity.
//!
//! When no image is set, the polygon is rendered with a radial color gradient
//! from the center outward (using [`Light::color`] at center → [`Light::color2`]
//! at falloff edge), with UVs centered on the light position.
//!
//! The [`Light::is_image_mode`] flag ("Passthrough") disables the lighting
//! calculation and just passes the texture through unmodified.
//!
//! ## Halo Mode (Bulb)
//!
//! The light is rendered as a radial glow effect using VPinball's `lightShader`
//! with the `bulb_light` technique from `LightShader.hlsl`. The [`Light::image`]
//! field is **ignored** — any value present is stale data from a previous Classic
//! mode setting. (In `light.cpp`: `offTexel = m_BulbLight ? nullptr : GetImage(m_szImage)`)
//!
//! The halo polygon is positioned at `surface_height +`[`Light::bulb_halo_height`],
//! allowing the glow to float above or below the surface. The blending uses a
//! modulate-vs-add approach controlled by [`Light::bulb_modulate_vs_add`]:
//! - Values near 0.0 → additive blending (glow adds to underlying color)
//! - Values near 1.0 → modulate blending (glow tints the underlying color)
//!
//! [`Light::transmission_scale`] controls how much light bleeds through surfaces
//! (rendered in VPinball's separate light buffer pass).
//!
//! ## Bulb Mesh
//!
//! When [`Light::show_bulb_mesh`] is enabled (typically with Halo mode), a 3D bulb
//! and socket mesh is rendered at the light position:
//! - **Socket**: Dark metallic mesh (`base_color = 0x181818`, `roughness = 0.9`)
//! - **Bulb**: Translucent glass mesh (`opacity = 0.2`, `clearcoat = 0xFFFFFF`)
//!
//! The bulb mesh is scaled by [`Light::mesh_radius`] and positioned at surface
//! height. An additional additive emission pass renders a faint glow over the bulb
//! mesh itself (at 2% of intensity) to simulate light directly reaching the camera.
//!
//! # Fading
//!
//! The [`Light::fader`] controls how the light transitions between on/off states:
//! - **None**: Instant on/off
//! - **Linear**: Linear fade at [`Light::fade_speed_up`]/[`Light::fade_speed_down`]
//!   rates
//! - **Incandescent**: Physically-based filament simulation with temperature
//!   ramping (warm glow when turning on, red fade when turning off)

use super::{dragpoint::DragPoint, vertex2d::Vertex2D};
use crate::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use crate::vpx::json::F32WithNanInf;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Shadow casting mode for Halo/Bulb lights.
///
/// Controls whether balls cast shadows from this light source. Only applies to
/// Halo mode ([`Light::is_bulb_light`] `= true`); Classic lights do not support
/// shadows.
///
/// When enabled, the shader traces rays from the light center to each surface
/// fragment and checks for ball occlusion (up to 8 balls). The shadow uses a
/// soft penumbra based on a fixed 5 VPU light radius (`BallShadows.fxh`).
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum ShadowMode {
    /// No shadow casting. Uses the `bulb_light` shader technique.
    None = 0,
    /// Balls cast raytraced shadows from this light. Uses the
    /// `bulb_light_with_ball_shadows` shader technique.
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

/// Controls how a light transitions between on/off states.
///
/// See the [Fading section](self#fading) in the module docs for an overview.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum Fader {
    /// Instant on/off — intensity jumps directly to the target value.
    None = 0,
    /// Linear fade using [`Light::fade_speed_up`] and [`Light::fade_speed_down`]
    /// as rates (intensity per ms).
    Linear = 1,
    /// Physically-based filament simulation. Models a BULB_44 tungsten filament
    /// with thermal inertia — it heats up when powered (warm white glow ramping
    /// on) and cools down when unpowered (red-shifting fade off). The fade speed
    /// parameters modulate the thermal time constant. The filament temperature
    /// also tints the light color relative to a 2700K reference.
    ///
    /// The simulation (in `bulb.cpp`) is based on:
    /// - **Stefan-Boltzmann law** for radiated power
    /// - Coblentz & Emerson, *"Luminous radiation from a black body and the
    ///   mechanical equivalent of light"* — temperature to visible emission
    /// - D.C. Agrawal, *"The Coiling Factor in the Tungsten Filament Lamps"* —
    ///   coil form factor corrections for resistivity and emissivity
    /// - D.C. Agrawal, *"Heating-times of tungsten filament incandescent
    ///   lamps"* — tungsten specific heat model
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

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Light {
    /// The name of the light, used for identification and referencing in scripts.
    /// BIFF tag: `NAME` (wide string)
    pub name: String,
    pub center: Vertex2D,    // VCEN
    pub height: Option<f32>, // HGHT added in 10.8
    pub falloff_radius: f32, // RADI
    pub falloff_power: f32,  // FAPO
    /// 0 = off, 1 = on, 2 = blinking
    /// m_d.m_state == 0.f ? 0 : (m_d.m_state == 2.f ? 2 : 1);
    /// BIFF tag: `STAT` deprecated, planned or removal in to 10.9+
    pub state_u32: u32,
    /// 0..1 is modulated from off to on, 2 is blinking
    /// BIFF tag: `STTF` added in 10.8
    pub state: Option<f32>,
    /// Light color at the center/near the light source.
    /// The shader interpolates between this color and color2 based on distance.
    /// BIFF tag: `COLR`
    pub color: Color,
    /// Light color at the falloff edge/far from the light source (also called "ColorFull").
    /// The shader interpolates between color and this color based on distance,
    /// creating a color gradient effect from the center outward.
    /// BIFF tag: `COL2`
    pub color2: Color,
    /// Blink pattern string used when the light state is "blinking" (state = 2).
    ///
    /// Each character represents one frame of the blink cycle:
    /// - `'1'`: light is ON during this frame
    /// - `'0'`: light is OFF during this frame
    ///
    /// The pattern repeats cyclically. For example, `"10"` alternates on/off every
    /// `blink_interval` milliseconds, while `"1100"` creates a longer on/off cycle.
    ///
    /// The runtime intensity when blinking is:
    /// ```text
    /// currentIntensity = intensity * intensity_scale * (pattern[frame] == '1' ? 1.0 : 0.0)
    /// ```
    ///
    /// If set to an empty string at runtime, VPinball forces it to `"0"` (always off).
    ///
    /// ## Default
    /// `"10"` (alternating on/off)
    ///
    /// BIFF tag: `BPAT`
    pub blink_pattern: String,
    /// Texture image name displayed on the light's polygon mesh (Classic mode only).
    ///
    /// In VPinball this is the "Image" property (COM: `get_Image`/`put_Image`),
    /// internally `m_szImage` inherited from `BaseProperty`.
    /// It defines the texture applied to the light's flat polygon (the "insert"
    /// shape defined by drag points) on the playfield.
    ///
    /// ## UV Mapping
    /// When an image is set, UV coordinates use table-space mapping:
    /// ```text
    /// tu = x / table_width
    /// tv = y / table_height
    /// ```
    /// When no image is set, UVs are radial from the light center:
    /// ```text
    /// tu = 0.5 + (x - center.x) * inv_maxdist
    /// tv = 0.5 + (y - center.y) * inv_maxdist
    /// ```
    ///
    /// ## Rendering
    /// In Classic mode ([`is_bulb_light`](Self::is_bulb_light) `= false`), this
    /// texture is bound as `SHADER_tex_light_color` and rendered with the
    /// `light_with_texture` shader technique. The light color modulates/blends
    /// with this texture based on the current intensity.
    ///
    /// In Halo mode ([`is_bulb_light`](Self::is_bulb_light) `= true`), this field
    /// is **ignored** — VPinball explicitly sets the texture to `nullptr`
    /// (`light.cpp`: `offTexel = m_BulbLight ? nullptr : GetImage(m_szImage)`).
    /// Any value present is stale data from a previous Classic mode setting;
    /// the UI hides the image field when Halo mode is selected.
    ///
    /// ## Default
    /// Empty string (no image)
    ///
    /// BIFF tag: `IMG1`
    pub image: String,
    /// Time in milliseconds between each step of the blink pattern.
    ///
    /// Controls the speed of the blinking animation when the light state is "blinking"
    /// (state = 2). Each character in `blink_pattern` is displayed for this duration
    /// before advancing to the next character.
    ///
    /// For example, with `blink_pattern = "10"` and `blink_interval = 125`:
    /// - The light is ON for 125ms, then OFF for 125ms (4 Hz blink rate)
    ///
    /// ## Default
    /// `125` (milliseconds)
    ///
    /// BIFF tag: `BINT`
    pub blink_interval: u32,
    /// Light intensity/brightness multiplier.
    ///
    /// ## Range
    /// - **0.0**: Light is off (no emission)
    /// - **1.0**: Default/normal brightness
    /// - **> 1.0**: Brighter than normal (HDR values supported)
    ///
    /// Typical values range from 0.0 to 10.0+, though there's no hard maximum.
    ///
    /// ## Unit
    /// Unitless multiplier. The final light emission is calculated as:
    /// ```text
    /// currentIntensity = intensity * intensity_scale * lightState
    /// ```
    /// Where `intensity_scale` is a runtime-only multiplier (not persisted),
    /// and `lightState` is 0.0 (off) to 1.0 (on) or 2.0 (blinking).
    ///
    /// ## Shader Usage
    /// In the light shader, intensity is scaled before being sent to the GPU:
    /// ```cpp
    /// lightColor_intensity.w = m_currentIntensity * 0.02f;  // For bulb lights
    /// // or
    /// lightColor_intensity.w = m_currentIntensity;          // For image lights
    /// ```
    ///
    /// The intensity also affects the light's transmission through surfaces
    /// when `transmission_scale` is applied.
    ///
    /// ## Default
    /// Default value is `1.0`
    ///
    /// BIFF tag: `BWTH`
    pub intensity: f32,
    /// Light transmission scale for "light through playfield" effects.
    ///
    /// ## Purpose
    /// Controls how much light passes through surfaces to illuminate objects
    /// below/behind (like GI lights shining through a translucent plastic).
    /// This is used during the "light buffer" render pass where VPinball
    /// calculates transmitted lighting.
    ///
    /// ## Range
    /// - **0.0**: No light transmission (light doesn't pass through surfaces)
    /// - **0.5**: Default, 50% of light intensity transmits
    /// - **1.0**: Full transmission (all light passes through)
    ///
    /// ## Unit
    /// Unitless multiplier (0.0 to 1.0, though values > 1.0 are allowed).
    ///
    /// ## Shader Usage
    /// Applied in the light shader during the light buffer render pass:
    /// ```cpp
    /// if (g_pplayer->m_renderer->IsRenderPass(Renderer::LIGHT_BUFFER))
    ///     lightColor_intensity.w *= m_d.m_transmissionScale;
    /// ```
    /// This multiplies the final light intensity by the transmission scale,
    /// affecting how much light "bleeds through" to objects on the other side.
    ///
    /// ## Default
    /// Default value is `0.5`
    ///
    /// BIFF tag: `TRMS`
    pub transmission_scale: f32,
    /// Name of the surface (ramp or wall top) this light sits on.
    /// Used to determine the light's base height (z position).
    /// If empty, the light sits on the playfield.
    /// BIFF tag: SURF
    pub surface: String,
    pub is_backglass: bool,   // BGLS
    pub depth_bias: f32,      // LIDB
    pub fade_speed_up: f32,   // FASP, can be Inf (Dr. Dude (Bally 1990)v3.0.vpx)
    pub fade_speed_down: f32, // FASD, can be Inf (Dr. Dude (Bally 1990)v3.0.vpx)
    /// Selects the light render mode: Halo (`true`) or Classic (`false`).
    ///
    /// - **Classic** (`false`): Flat lightmap polygon using the [`image`](Self::image)
    ///   texture with surface material properties. Uses `basicShader` and the
    ///   `light_with_texture`/`light_without_texture` techniques.
    /// - **Halo** (`true`): Radial glow effect using `lightShader` and the
    ///   `bulb_light` technique. The [`image`](Self::image) field is ignored.
    ///   The halo height is controlled by [`bulb_halo_height`](Self::bulb_halo_height),
    ///   and the blend mode by [`bulb_modulate_vs_add`](Self::bulb_modulate_vs_add).
    ///
    /// Combined with [`visible`](Self::visible), this maps to the UI's three-mode
    /// dropdown: Hidden (`visible=false`), Classic, or Halo.
    ///
    /// See the [module-level documentation](self) for full rendering details.
    ///
    /// ## Default
    /// `false` (Classic mode)
    ///
    /// ## BIFF tag
    /// `BULT`
    pub is_bulb_light: bool,
    /// Passthrough / image mode flag (Classic mode only).
    ///
    /// When `true`, the lighting calculation is disabled and the [`image`](Self::image)
    /// texture is displayed unmodified (passthrough). When `false`, the surface
    /// material lighting is applied to the texture.
    ///
    /// This has no effect in Halo mode ([`is_bulb_light`](Self::is_bulb_light) `= true`).
    ///
    /// ## Default
    /// `false`
    ///
    /// ## BIFF tag
    /// `IMMO`
    pub is_image_mode: bool,
    /// Whether to render the 3D bulb and socket mesh at the light position.
    ///
    /// When enabled, a translucent glass bulb and a dark metallic socket mesh
    /// are rendered. Typically used together with Halo mode. The mesh is scaled
    /// by [`mesh_radius`](Self::mesh_radius).
    ///
    /// See the [Bulb Mesh section](self#bulb-mesh) in the module docs for details.
    ///
    /// ## Default
    /// `false`
    ///
    /// ## BIFF tag
    /// `SHBM`
    pub show_bulb_mesh: bool,
    /// Whether the bulb glass mesh is rendered in the static pass.
    ///
    /// When `true`, the translucent bulb glass mesh is pre-rendered once in the
    /// static render pass (not updated per frame). When `false`, it is rendered
    /// in the dynamic pass every frame, which is necessary if the bulb's
    /// appearance needs to change at runtime (e.g. animated emission glow).
    ///
    /// Note: the socket mesh is always rendered in the static pass regardless
    /// of this setting, since bulbs are not movable.
    ///
    /// Only relevant when [`show_bulb_mesh`](Self::show_bulb_mesh) is enabled.
    ///
    /// ## Default
    /// `true`
    ///
    /// ## BIFF tag
    /// `STBM` (added in 10.?)
    pub has_static_bulb_mesh: Option<bool>,
    /// Whether this light is included in ball reflections.
    ///
    /// When `true`, this light is added to the list of lights that are reflected
    /// on the ball surface. The renderer collects all lights with this flag
    /// enabled (excluding backglass lights) and passes them to the ball shader
    /// for reflection calculations.
    ///
    /// ## Default
    /// `true`
    ///
    /// ## BIFF tag
    /// `SHRB`
    pub show_reflection_on_ball: bool,
    /// Scale factor for the 3D bulb and socket mesh.
    ///
    /// The built-in bulb/socket meshes are multiplied by this value to determine
    /// their size. Only relevant when [`show_bulb_mesh`](Self::show_bulb_mesh) is
    /// enabled.
    ///
    /// ## Default
    /// `20.0`
    ///
    /// ## BIFF tag
    /// `BMSC`
    pub mesh_radius: f32,
    /// Controls the blend mode between modulation and addition for the halo
    /// glow effect (Halo mode only).
    ///
    /// - Values near **0.0** → additive blending (glow brightens the scene)
    /// - Values near **1.0** → modulate blending (glow tints the scene)
    ///
    /// Internally clamped to `[0.0001, 0.9999]` since 0.0 disables blending
    /// entirely and 1.0 looks bad with day→night transitions.
    ///
    /// ## Default
    /// `0.9`
    ///
    /// ## BIFF tag
    /// `BMVA`
    pub bulb_modulate_vs_add: f32,
    /// Height offset of the halo glow polygon above the surface (Halo mode only).
    ///
    /// The halo is rendered at `surface_height + bulb_halo_height`. Positive values
    /// raise the halo above the surface, negative values lower it below. This allows
    /// the glow to appear to emanate from above or inside the playfield.
    ///
    /// Note: this is independent of [`height`](Self::height), which only affects the
    /// light's emission point for point-light calculations.
    ///
    /// ## Default
    /// `28.0`
    ///
    /// ## BIFF tag
    /// `BHHI`
    pub bulb_halo_height: f32,
    /// Shadow casting mode for this light (Halo mode only).
    ///
    /// Only applies to Halo/Bulb lights ([`is_bulb_light`](Self::is_bulb_light)
    /// `= true`). Classic lights do not cast shadows.
    ///
    /// See [`ShadowMode`] for the available options.
    ///
    /// ## Default
    /// [`ShadowMode::None`]
    ///
    /// ## BIFF tag
    /// `SHDW` (added in 10.8)
    pub shadows: Option<ShadowMode>,
    /// Controls how the light transitions between on/off states.
    ///
    /// See [`Fader`] for the available options.
    ///
    /// ## Default
    /// [`Fader::Linear`]
    ///
    /// ## BIFF tag
    /// `FADE` (added in 10.8)
    pub fader: Option<Fader>,
    /// Whether the light is rendered at all.
    ///
    /// When `false`, the light is in "Hidden" mode — it still exists for scripting
    /// but produces no visual output. When `true`, the render mode depends on
    /// [`is_bulb_light`](Self::is_bulb_light).
    ///
    /// When absent (`None`), the light defaults to visible.
    ///
    /// ## BIFF tag
    /// `VSBL` (added in 10.8)
    pub visible: Option<bool>,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,

    // last
    pub drag_points: Vec<DragPoint>,
}
impl_shared_attributes!(Light);

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
    #[serde(flatten)]
    pub timer: TimerData,
    blink_pattern: String,
    #[serde(alias = "off_image")]
    image: String,
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
            timer: light.timer.clone(),
            blink_pattern: light.blink_pattern.clone(),
            image: light.image.clone(),
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
            timer: self.timer.clone(),
            blink_pattern: self.blink_pattern.clone(),
            image: self.image.clone(),
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
            editor_layer: None,
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
        let timer = TimerData::default();
        let blink_pattern: String = "10".to_owned();
        let image: String = Default::default();
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
        let editor_layer: Option<u32> = None;
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
            timer,
            blink_pattern,
            image,
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
                "BPAT" => light.blink_pattern = reader.get_string(),
                "IMG1" => light.image = reader.get_string(),
                "BINT" => light.blink_interval = reader.get_u32(),
                "BWTH" => light.intensity = reader.get_f32(),
                "TRMS" => light.transmission_scale = reader.get_f32(),
                "SURF" => light.surface = reader.get_string(),
                "NAME" => light.name = reader.get_wide_string(),

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

                // many of these
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    light.drag_points.push(point);
                }
                other => {
                    if !light.timer.biff_read_tag(other, reader)
                        && !light.read_shared_attribute(other, reader)
                    {
                        warn!(
                            "Unknown tag {} for {}",
                            other,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag();
                    }
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
        self.timer.biff_write(writer);
        writer.write_tagged_string("BPAT", &self.blink_pattern);
        writer.write_tagged_string("IMG1", &self.image);
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
            timer: TimerData {
                is_enabled: true,
                interval: 7,
            },
            blink_pattern: "test pattern".to_string(),
            image: "test image".to_string(),
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
            editor_layer: Some(17),
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
