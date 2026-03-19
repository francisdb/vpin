use super::dragpoint::DragPoint;
use crate::impl_shared_attributes;
use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Blend mode used when combining image_a and image_b on flashers.
///
/// When a flasher has both `image_a` and `image_b` set, this filter determines
/// how the two textures are blended together. The blend intensity is controlled
/// by `filter_amount` (0-100).
#[derive(Debug, PartialEq, Clone, Default)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum Filter {
    /// No filtering/blending applied
    None = 0,
    /// Additive blending - adds the color values together, creating a brightening effect
    Additive = 1,
    /// Overlay blending - combines Multiply and Screen modes based on the base color
    #[default]
    Overlay = 2,
    /// Multiply blending - multiplies the color values, creating a darkening effect
    Multiply = 3,
    /// Screen blending - inverts, multiplies, and inverts again, creating a brightening effect
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

/// Render mode for flashers, determining how the flasher is rendered and which
/// properties are used.
///
/// Default in use by vpinball is [`RenderMode::Flasher`].
///
/// Introduced in VPX 10.8.1 (BIFF tag `RDMD`), replacing the older boolean `is_dmd` field.
///
/// See also [`render_style`](Flasher::render_style) which is interpreted differently
/// depending on this mode.
#[derive(Debug, PartialEq, Clone, Default)]
#[cfg_attr(test, derive(fake::Dummy))]
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
    /// Render of one of the ancillary windows (backglass, topper, scoreview)
    ExtRender = 4,
}

impl From<u32> for RenderMode {
    fn from(value: u32) -> Self {
        match value {
            0 => RenderMode::Flasher,
            1 => RenderMode::DMD,
            2 => RenderMode::Display,
            3 => RenderMode::AlphaSeg,
            4 => RenderMode::ExtRender,
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
            RenderMode::ExtRender => 4,
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
            RenderMode::ExtRender => "ext_render",
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
                "ext_render" => Ok(RenderMode::ExtRender),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid RenderMode value {s}, expecting \"flasher\", \"dmd\", \"display\", \"alpha_seg\" or \"ext_render\""
                ))),
            },
            Value::Number(n) => {
                let n = n.as_u64().unwrap();
                match n {
                    0 => Ok(RenderMode::Flasher),
                    1 => Ok(RenderMode::DMD),
                    2 => Ok(RenderMode::Display),
                    3 => Ok(RenderMode::AlphaSeg),
                    4 => Ok(RenderMode::ExtRender),
                    _ => Err(serde::de::Error::custom(
                        "Invalid RenderMode value, expecting 0, 1, 2, 3 or 4",
                    )),
                }
            }
            _ => Err(serde::de::Error::custom(
                "Invalid RenderMode value, expecting string or number",
            )),
        }
    }
}

/// DMD shading profile used as [`Flasher::render_style`] when
/// [`render_mode`](Flasher::render_mode) is [`RenderMode::DMD`].
///
/// These are indices into the DMD profile settings arrays (7 profiles).
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DmdStyle {
    #[default]
    Legacy = 0,
    /// Classic neon plasma DMD
    Plasma = 1,
    /// Red LED DMD (used after RoHS regulation entry into force)
    RedLED = 2,
    GreenLED = 3,
    YellowLED = 4,
    GenericPlasma = 5,
    GenericLED = 6,
}

impl From<u32> for DmdStyle {
    fn from(value: u32) -> Self {
        match value {
            0 => DmdStyle::Legacy,
            1 => DmdStyle::Plasma,
            2 => DmdStyle::RedLED,
            3 => DmdStyle::GreenLED,
            4 => DmdStyle::YellowLED,
            5 => DmdStyle::GenericPlasma,
            6 => DmdStyle::GenericLED,
            _ => panic!("Invalid DmdStyle value {value}, expected 0–6"),
        }
    }
}

impl From<DmdStyle> for u32 {
    fn from(value: DmdStyle) -> Self {
        value as u32
    }
}

/// CRT/screen shading profile used as [`Flasher::render_style`] when
/// [`render_mode`](Flasher::render_mode) is [`RenderMode::Display`].
///
/// The profile index is passed directly to the CRT shader.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DisplayStyle {
    #[default]
    Pixelated = 0,
    Smoothed = 1,
    CRT = 2,
}

impl From<u32> for DisplayStyle {
    fn from(value: u32) -> Self {
        match value {
            0 => DisplayStyle::Pixelated,
            1 => DisplayStyle::Smoothed,
            2 => DisplayStyle::CRT,
            _ => panic!("Invalid DisplayStyle value {value}, expected 0–2"),
        }
    }
}

impl From<DisplayStyle> for u32 {
    fn from(value: DisplayStyle) -> Self {
        value as u32
    }
}

/// Segment display shading style, used as the low bits (`value % 8`) of
/// [`Flasher::render_style`] when [`render_mode`](Flasher::render_mode) is
/// [`RenderMode::AlphaSeg`].
///
/// These are indices into the 8 alphanumeric segment profile settings arrays.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum SegStyle {
    /// Neon plasma
    #[default]
    Plasma = 0,
    /// VFD blueish
    BlueVFD = 1,
    /// VFD greenish
    GreenVFD = 2,
    RedLED = 3,
    GreenLED = 4,
    YellowLED = 5,
    GenericPlasma = 6,
    GenericLED = 7,
}

impl From<u32> for SegStyle {
    fn from(value: u32) -> Self {
        match value {
            0 => SegStyle::Plasma,
            1 => SegStyle::BlueVFD,
            2 => SegStyle::GreenVFD,
            3 => SegStyle::RedLED,
            4 => SegStyle::GreenLED,
            5 => SegStyle::YellowLED,
            6 => SegStyle::GenericPlasma,
            7 => SegStyle::GenericLED,
            _ => panic!("Invalid SegStyle value {value}, expected 0–7"),
        }
    }
}

impl From<SegStyle> for u32 {
    fn from(value: SegStyle) -> Self {
        value as u32
    }
}

/// Segment display family/shape hint, used as the high bits (`value / 8`) of
/// [`Flasher::render_style`] when [`render_mode`](Flasher::render_mode) is
/// [`RenderMode::AlphaSeg`].
///
/// Selects the segment shape assets (e.g. `7seg-gts.png`, `7seg-bally.png`).
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum SegFamily {
    #[default]
    Generic = 0,
    Gottlieb = 1,
    Williams = 2,
    Bally = 3,
    Atari = 4,
}

impl From<u32> for SegFamily {
    fn from(value: u32) -> Self {
        match value {
            0 => SegFamily::Generic,
            1 => SegFamily::Gottlieb,
            2 => SegFamily::Williams,
            3 => SegFamily::Bally,
            4 => SegFamily::Atari,
            _ => panic!("Invalid SegFamily value {value}, expected 0–4"),
        }
    }
}

impl From<SegFamily> for u32 {
    fn from(value: SegFamily) -> Self {
        value as u32
    }
}

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Flasher {
    /// Height (z position) of the flasher above the playfield in VPU.
    /// This is added to the z coordinate after rotation is applied.
    /// BIFF tag: `FHEI`
    pub height: f32,
    /// X position of the flasher center in table coordinates.
    /// Changing this moves all drag points by the same delta.
    /// BIFF tag: `FLAX`
    pub pos_x: f32,
    /// Y position of the flasher center in table coordinates.
    /// Changing this moves all drag points by the same delta.
    /// BIFF tag: `FLAY`
    pub pos_y: f32,
    /// Rotation around the X axis in degrees.
    /// BIFF tag: `FROX`
    pub rot_x: f32,
    /// Rotation around the Y axis in degrees.
    /// BIFF tag: `FROY`
    pub rot_y: f32,
    /// Rotation around the Z axis in degrees.
    /// BIFF tag: `FROZ`
    pub rot_z: f32,
    pub color: Color,
    pub name: String,
    /// Primary texture for the flasher.
    /// When only image_a is set (no image_b), this texture is displayed directly.
    /// When both image_a and image_b are set, they are blended together using
    /// the `filter` and `modulate_vs_add` settings.
    /// BIFF tag: `IMAG`
    pub image_a: String,
    /// Secondary texture for blending with image_a.
    /// When set along with image_a, both textures are blended together in the shader
    /// based on `filter` (None, Additive, Overlay, Multiply, Screen) and
    /// `modulate_vs_add` settings. If only image_b is set (no image_a), it acts
    /// as the primary texture.
    /// BIFF tag: `IMAB`
    pub image_b: String,
    /// Overall alpha/opacity of the flasher (0-100).
    /// BIFF tag: `FALP`
    pub alpha: i32,
    /// Controls blending between image_a and image_b when both are set.
    /// Range: 0.0 to 1.0. Values close to 0 favor image_a, values close to 1 favor image_b.
    /// Clamped internally to avoid 0 (disables blend) and 1 (looks bad with day/night changes).
    /// BIFF tag: `MOVA`
    pub modulate_vs_add: f32,
    pub is_visible: bool,
    pub add_blend: bool,
    /// Indicates if this flasher is a DMD (dot matrix display).
    /// BIFF tag: `IDMD` added in 10.2? Since 10.8.1 no longer written and replaced by RDMD
    pub is_dmd: Option<bool>,
    /// Render mode for the flasher, determining how it is rendered and which properties are used.
    /// replaces `is_dmd` and changes from a bool to an enum
    /// BIFF tag: `RDMD` Since 10.8.1
    pub render_mode: Option<RenderMode>,
    /// Application-defined style profile reference whose meaning depends on
    /// [`render_mode`](Flasher::render_mode):
    ///
    /// - **[`Flasher`](RenderMode::Flasher)**: not used.
    /// - **[`DMD`](RenderMode::DMD)**: a [`DmdStyle`] value (0–6).
    /// - **[`Display`](RenderMode::Display)**: a [`DisplayStyle`] value (0–2).
    /// - **[`AlphaSeg`](RenderMode::AlphaSeg)**: packed value where
    ///   `value % 8` is a [`SegStyle`] (0–7) and `value / 8` is a
    ///   [`SegFamily`] (0–4).
    /// - **[`ExtRender`](RenderMode::ExtRender)**: window identifier
    ///   (1=Backglass, 2=ScoreView, 3=Topper).
    ///
    /// BIFF tag: `RSTL` Since 10.8.1
    pub render_style: Option<u32>,
    /// Roughness of the glass overlay above DMD/Display/AlphaSeg renders.
    /// BIFF tag: `GRGH` Since 10.8.1
    pub glass_roughness: Option<f32>,
    /// Ambient color of the glass overlay (RGB packed as `0x00RRGGBB`).
    /// BIFF tag: `GAMB` Since 10.8.1
    pub glass_ambient: Option<u32>,
    /// Top padding for the glass overlay area.
    /// BIFF tag: `GTOP` Since 10.8.1
    pub glass_pad_top: Option<f32>,
    /// Bottom padding for the glass overlay area.
    /// BIFF tag: `GBOT` Since 10.8.1
    pub glass_pad_bottom: Option<f32>,
    /// Left padding for the glass overlay area.
    /// BIFF tag: `GLFT` Since 10.8.1
    pub glass_pad_left: Option<f32>,
    /// Right padding for the glass overlay area.
    /// BIFF tag: `GRHT` Since 10.8.1
    pub glass_pad_right: Option<f32>,
    /// Image source link for display content (default source is script).
    /// BIFF tag: `LINK` Since 10.8.1
    pub image_src_link: Option<String>,
    /// Whether to display the texture in the VPinball editor preview.
    /// This does NOT affect runtime rendering — textures are always rendered if set.
    /// Also used on: [`Wall`], [`Primitive`].
    /// BIFF tag: `DSPT`
    pub display_texture: bool,
    /// Offset applied when depth-sorting transparent and overlapping objects.
    /// Higher values move the object "further away" in the sort order, causing it
    /// to render behind objects with lower bias.
    /// Also used on: [`Primitive`], [`Ramp`], [`Light`], [`HitTarget`].
    /// BIFF tag: `FLDB`
    pub depth_bias: f32,
    /// Controls how the texture is mapped onto the flasher polygon.
    /// - [`World`](RampImageAlignment::World): UVs are based on table coordinates.
    /// - [`Wrap`](RampImageAlignment::Wrap): UVs are based on the flasher bounding box
    ///   (texture is stretched to fit).
    ///
    /// Also used on: [`Ramp`].
    /// BIFF tag: `ALGN`
    pub image_alignment: RampImageAlignment,
    /// Blend mode used when combining image_a and image_b.
    /// See [`Filter`] for available modes.
    pub filter: Filter,
    /// Intensity of the filter effect (0-100).
    /// BIFF tag: `FIAM`
    pub filter_amount: u32,
    /// BIFF tag: `LMAP` added in 10.8
    pub light_map: Option<String>,
    /// BIFF tag: `BGLS` added in 10.8.1
    pub backglass: Option<bool>,

    /// Polygon vertices defining the flasher shape.
    ///
    /// These points define a 2D polygon that forms the flasher mesh. The coordinates
    /// are stored as absolute positions in table space. The z coordinate of each
    /// drag point is typically 0.
    ///
    /// Relationship with `pos_x`/`pos_y`:
    /// - `pos_x`/`pos_y` define the center of the flasher
    /// - When `pos_x`/`pos_y` change in the editor, all drag points are moved by
    ///   the same delta (they move together as a group)
    /// - The rotation center for `rot_x`/`rot_y`/`rot_z` is calculated from the
    ///   bounding box of these drag points
    ///
    /// The mesh generation process:
    /// 1. Vertices are created from these drag points
    /// 2. The bounding box center is calculated from min/max x and y
    /// 3. Vertices are translated to the bounding box center, rotated by `rot_z`,
    ///    `rot_y`, `rot_x`, then translated back with `height` added to z
    ///
    /// Minimum 3 points required to form a valid polygon.
    pub drag_points: Vec<DragPoint>,

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
}
impl_shared_attributes!(Flasher);

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
            timer: TimerData::default(),
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
            editor_layer: None,
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
    #[serde(flatten)]
    pub timer: TimerData,
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
            timer: flasher.timer.clone(),
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
            timer: self.timer.clone(),
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
            editor_layer: None,
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
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    flasher.drag_points.push(point);
                }
                _ => {
                    if !flasher.timer.biff_read_tag(tag_str, reader)
                        && !flasher.read_shared_attribute(tag_str, reader)
                    {
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
        self.timer.biff_write(writer);
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
    use rand::RngExt;

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
            timer: TimerData {
                is_enabled: rng.random(),
                interval: rng.random(),
            },
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
            editor_layer: Some(rng.random()),
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
