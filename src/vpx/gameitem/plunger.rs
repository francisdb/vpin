use super::vertex2d::Vertex2D;
use crate::vpx::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Visual style of a plunger, mirroring vpinball's `PlungerType`.
///
/// Values this library does not know are kept in [`PlungerType::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum PlungerType {
    /// Value 0, outside vpinball's enum, found in "Star Wars (Data East 1992) VPW v1.2.2.vpx".
    /// Kept as a named variant for compatibility with existing expanded tables.
    Unknown,
    /// `PlungerTypeModern`: rendered 3D plunger with rod and spring.
    Modern,
    /// `PlungerTypeFlat`: flat textured plunger.
    Flat,
    /// `PlungerTypeCustom`: plunger built from the custom rod, ring, spring and tip parameters.
    Custom,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(u32),
}
impl From<u32> for PlungerType {
    fn from(value: u32) -> Self {
        match value {
            0 => PlungerType::Unknown,
            1 => PlungerType::Modern,
            2 => PlungerType::Flat,
            3 => PlungerType::Custom,
            other => {
                warn!("Unknown PlungerType value {other}, keeping it as is");
                PlungerType::Other(other)
            }
        }
    }
}
impl From<&PlungerType> for u32 {
    fn from(value: &PlungerType) -> Self {
        match value {
            PlungerType::Unknown => 0,
            PlungerType::Modern => 1,
            PlungerType::Flat => 2,
            PlungerType::Custom => 3,
            PlungerType::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`PlungerType::Other`]
impl Serialize for PlungerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PlungerType::Unknown => serializer.serialize_str("unknown"),
            PlungerType::Modern => serializer.serialize_str("modern"),
            PlungerType::Flat => serializer.serialize_str("flat"),
            PlungerType::Custom => serializer.serialize_str("custom"),
            PlungerType::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for PlungerType {
    fn deserialize<D>(deserializer: D) -> Result<PlungerType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PlungerTypeVisitor;
        impl serde::de::Visitor<'_> for PlungerTypeVisitor {
            type Value = PlungerType;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a PlungerType as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<PlungerType, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(PlungerType::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<PlungerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "unknown" => Ok(PlungerType::Unknown),
                    "modern" => Ok(PlungerType::Modern),
                    "flat" => Ok(PlungerType::Flat),
                    "custom" => Ok(PlungerType::Custom),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["unknown", "modern", "flat", "custom"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(PlungerTypeVisitor)
    }
}
#[cfg(test)]
mod plunger_type_open_enum_tests {
    /// The legacy raw value 0 must normalize to the named Unknown
    /// variant, never to Other(0): both write the same bytes, so two
    /// representations would break round-trip equality.
    #[test]
    fn legacy_unknown_value_normalizes() {
        assert_eq!(PlungerType::from(0), PlungerType::Unknown);
    }

    use super::PlungerType;

    #[test]
    fn unknown_value_round_trips() {
        let value = PlungerType::from(4_000_000_000);
        assert_eq!(value, PlungerType::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: PlungerType = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<PlungerType>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

/// A plunger: the spring-loaded rod that launches the ball from the shooter
/// lane.
///
/// vpinball simulates it as a moving bar at the bottom of the lane that
/// travels [`stroke`](Self::stroke) VPU up the table from
/// [`center`](Self::center). Two inputs drive it. The script (digital)
/// plunger is moved by the `PullBack` and `Fire` script methods, usually
/// bound to a key, and tuned with [`speed_pull`](Self::speed_pull) and
/// [`speed_fire`](Self::speed_fire). The mechanical plunger of a cabinet is
/// enabled with [`is_mech_plunger`](Self::is_mech_plunger) and follows the
/// analog sensor with the spring strength
/// [`mech_strength`](Self::mech_strength);
/// [`auto_plunger`](Self::auto_plunger) turns either input into a launch
/// button. [`park_position`](Self::park_position),
/// [`scatter_velocity`](Self::scatter_velocity) and
/// [`momentum_xfer`](Self::momentum_xfer) apply to both.
///
/// The look is chosen by [`plunger_type`](Self::plunger_type):
/// [`anim_frames`](Self::anim_frames) only applies to the flat plunger, and
/// [`tip_shape`](Self::tip_shape) and the `rod_*`, `ring_*` and `spring_*`
/// fields only to the custom plunger.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Plunger {
    /// Position of the plunger tip when fully pulled back, in VPU.
    ///
    /// `x` is the center line of the rod; from `y` the tip travels up the
    /// table (decreasing y) by [`stroke`](Self::stroke) VPU when released.
    /// The plunger sits on [`surface`](Self::surface).
    ///
    /// BIFF tag `VCEN`
    pub center: Vertex2D,
    /// Half width of the plunger, in VPU.
    ///
    /// The collision bar spans `center.x - width` to `center.x + width`.
    /// The mesh uses it as the nominal radius scale: the custom rod, ring,
    /// spring and tip diameters are fractions of it, and the flat plunger
    /// image is placed `1.25 x width` above its surface. vpinball default
    /// `25.0`.
    ///
    /// BIFF tag `WDTH`
    pub width: f32,
    /// Length of the plunger body behind the tip, in VPU.
    ///
    /// The base of the rod, and the bottom edge of the flat plunger image,
    /// sit at `center.y + height`; the collision box extends the same
    /// distance. vpinball default `20.0`.
    ///
    /// BIFF tag `HIGH`
    pub height: f32,
    /// Vertical offset of the rendered plunger, in VPU.
    ///
    /// Added to the height of [`surface`](Self::surface) when building the
    /// mesh, to tune where the plunger appears; the collision shape stays
    /// at the surface height. vpinball default `0.0`.
    ///
    /// BIFF tag `ZADJ`
    pub z_adjust: f32,
    /// Travel length of the plunger, in VPU.
    ///
    /// The tip moves between `center.y` (fully retracted) and
    /// `center.y - stroke` (fully forward). vpinball renders 25 animation
    /// frames per 80 VPU of stroke. vpinball default `80.0`.
    ///
    /// BIFF tag `HPSL`
    pub stroke: f32,
    /// Pull force of the script plunger, `PullSpeed` in VBScript.
    ///
    /// The `PullBack` and `PullBackandRetract` script methods (the keyboard
    /// plunger) apply this force every physics step, so the plunger speed
    /// grows by `speed_pull / mass` per step until it reaches the fully
    /// retracted position. Internal force units. vpinball default `5.0`;
    /// this crate's `Default` uses `0.5`.
    ///
    /// BIFF tag `SPDP`
    pub speed_pull: f32,
    /// Release strength of the plunger, `ReleaseSpeed` in VBScript.
    ///
    /// A percentage-like scale where `100` is nominal. The `Fire` script
    /// method releases the plunger at a speed proportional to
    /// `speed_fire / 100` times the pull distance, and the same factor
    /// scales the hit speed reported by a mechanical plunger sensor.
    /// vpinball default `80.0`.
    ///
    /// BIFF tag `SPDF`
    pub speed_fire: f32,
    /// Visual style of the plunger, see [`PlungerType`].
    ///
    /// Selects the mesh: the built-in modern lathe shape, a flat image
    /// strip or a lathe shape built from the custom fields. It does not
    /// change the physics. vpinball default [`PlungerType::Modern`].
    ///
    /// BIFF tag `TYPE`
    pub plunger_type: PlungerType,
    /// Number of animation cells in the [`image`](Self::image) of a
    /// [`PlungerType::Flat`] plunger.
    ///
    /// The image is split into `anim_frames` equally wide columns, the
    /// fully extended plunger in the leftmost and the fully retracted one
    /// in the rightmost; values below `1` count as `1`. Ignored by the
    /// other plunger types. vpinball default `1`.
    ///
    /// BIFF tag `ANFR`
    pub anim_frames: u32,
    /// Name of the material rendered on the plunger; empty for the default
    /// material.
    ///
    /// BIFF tag `MATR`
    pub material: String,
    /// Name of the texture rendered on the plunger; empty for none.
    ///
    /// A flat plunger draws it as an animation strip, see
    /// [`anim_frames`](Self::anim_frames). The modern and custom plungers
    /// wrap it around the lathed body, with the tip in the top quarter of
    /// the image and the ring and rod in the quarters below it.
    ///
    /// BIFF tag `IMAG`
    pub image: String,
    /// Spring strength tying the simulated plunger to a mechanical plunger.
    ///
    /// With [`is_mech_plunger`](Self::is_mech_plunger) set and no script
    /// pull or fire in progress, every physics step accelerates the
    /// simulated plunger toward the sensor position by
    /// `mech_strength x distance x stroke / (mass x 13)`, with some
    /// damping, so higher values track the real plunger more tightly.
    /// Unused without a mechanical plunger. vpinball default `85.0`.
    ///
    /// BIFF tag `MEST`
    pub mech_strength: f32,
    /// Whether the plunger follows the mechanical (analog) plunger input of
    /// a cabinet.
    ///
    /// When `true` the simulated plunger tracks the sensor position (see
    /// [`mech_strength`](Self::mech_strength)), uses the speed reported by
    /// the sensor for the ball hit when available, and with
    /// [`auto_plunger`](Self::auto_plunger) turns a pull-and-release
    /// gesture into a Launch Ball key press. When `false` only the
    /// `PullBack` and `Fire` script methods move it. vpinball default
    /// `false`.
    ///
    /// BIFF tag `MECH`
    pub is_mech_plunger: bool,
    /// Whether the plunger models a launch button or a ROM-controlled
    /// kicker instead of a player-operated spring plunger.
    ///
    /// The `Fire` script method then always releases from the fully
    /// retracted position for a constant launch strength,
    /// `PullBackandRetract` skips the retract motion, and a mechanical
    /// plunger no longer moves the simulated plunger (which stays at
    /// [`park_position`](Self::park_position)); a pull-and-release gesture
    /// on it instead sends a synthetic Launch Ball key press to the script
    /// (key down, key up 100 ms later). vpinball default `false`.
    ///
    /// BIFF tag `APLG`
    pub auto_plunger: bool,
    /// Rest position of the plunger as a fraction of the stroke, measured
    /// from the fully forward end: `0.0` is fully forward, `1.0` fully
    /// retracted.
    ///
    /// The plunger starts and settles here, `PullBack` retracts from here
    /// and `Fire` releases from at least this position. vpinball default
    /// `0.5 / 3.0`, a 0.5 inch rest on a 3 inch stroke.
    ///
    /// BIFF tag `MPRK`
    pub park_position: f32,
    /// Random speed added to the ball along the lane when the plunger hits
    /// it, in vpinball velocity units.
    ///
    /// Approximates the mechanical randomness of a real plunger. The value
    /// is scaled by the table's global difficulty; when the ball leaves
    /// faster than that, a random amount within `+/- scatter_velocity`
    /// (quadratic distribution) is added to its speed. `0.0` disables it.
    /// vpinball default `0.0`.
    ///
    /// BIFF tag `PSCV`
    pub scatter_velocity: f32,
    /// Momentum transfer factor of the plunger hit, relative units where
    /// `1.0` is nominal.
    ///
    /// The impulse given to the ball is the plunger speed times
    /// `momentum_xfer / ball_mass` (ball mass clamped to at least `0.05`),
    /// so tables written before this property existed keep the old
    /// behaviour. vpinball default `1.0`.
    ///
    /// BIFF tag `MOMX`
    pub momentum_xfer: f32,
    /// Whether the plunger is rendered. vpinball default `true`.
    ///
    /// BIFF tag `VSBL`
    pub is_visible: bool,
    /// Whether this plunger appears in playfield reflections.
    ///
    /// When `true`, the ball is rendered in the reflection pass.
    /// When `false`, the ball won't appear as a reflection on the playfield.
    ///
    /// BIFF tag: `REEN` (was missing in 10.01)
    pub is_reflection_enabled: Option<bool>,
    /// Name of the surface (ramp or wall top) this plunger sits on.
    /// Used to determine the plunger's base height (z position).
    /// If empty, the plunger sits on the playfield.
    /// BIFF tag: SURF
    pub surface: String,
    /// Name of the plunger, the identifier used from VBScript. Stored as a
    /// UTF-16 string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Lathe profile of the tip of a [`PlungerType::Custom`] plunger.
    ///
    /// A semicolon separated list of `distance diameter` pairs: the
    /// distance from the tip in VPU (each at least the previous one) and
    /// the diameter at that point as a fraction of [`width`](Self::width),
    /// so `1.0` is a tip as wide as the nominal plunger width. Ignored by
    /// the other plunger types. vpinball default
    /// `"0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .84"`.
    ///
    /// BIFF tag `TIPS`
    pub tip_shape: String,
    /// Diameter of the rod of a custom plunger, as a fraction of
    /// [`width`](Self::width). vpinball default `0.6`.
    ///
    /// BIFF tag `RODD`
    pub rod_diam: f32,
    /// Distance between the tip and the ring of a custom plunger, in VPU.
    /// vpinball default `2.0`.
    ///
    /// BIFF tag `RNGG`
    pub ring_gap: f32,
    /// Diameter of the ring of a custom plunger, as a fraction of
    /// [`width`](Self::width). vpinball default `0.94`.
    ///
    /// BIFF tag `RNGD`
    pub ring_diam: f32,
    /// Length of the ring of a custom plunger along the rod, in VPU.
    /// vpinball default `3.0`.
    ///
    /// BIFF tag `RNGW`
    pub ring_width: f32,
    /// Diameter of the spring of a custom plunger, as a fraction of
    /// [`width`](Self::width). vpinball default `0.77`.
    ///
    /// BIFF tag `SPRD`
    pub spring_diam: f32,
    /// Wire thickness of the spring of a custom plunger, in VPU. vpinball
    /// default `1.38`.
    ///
    /// BIFF tag `SPRG`
    pub spring_gauge: f32,
    /// Number of coils in the spring of a custom plunger.
    ///
    /// Together with [`spring_end_loops`](Self::spring_end_loops) it sets
    /// the compressed spring length (`2.2` VPU per coil) and so the length
    /// of the rod. vpinball default `8.0`.
    ///
    /// BIFF tag `SPRL`
    pub spring_loops: f32,
    /// Number of closely wound coils at the end of the spring of a custom
    /// plunger, spaced at `2.2 x` the wire thickness instead of the
    /// stretched spring pitch. vpinball default `2.5`.
    ///
    /// BIFF tag `SPRE`
    pub spring_end_loops: f32,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    /// Whether the item is locked in the editor to prevent accidental
    /// moving or editing. Editor-only; has no runtime effect.
    ///
    /// BIFF tag `LOCK`
    pub is_locked: bool,
    /// Legacy editor layer index. Removed in 10.8.1, superseded by part
    /// groups (see `part_group_name`). `None` when absent.
    ///
    /// BIFF tag `LAYR`
    pub editor_layer: Option<u32>,
    /// Display name of the legacy editor layer; defaults to
    /// `"Layer_{editor_layer + 1}"`. Editor-only. `None` when absent.
    ///
    /// BIFF tag `LANR`
    pub editor_layer_name: Option<String>,
    /// Whether the legacy editor layer is shown in the editor.
    /// Editor-only; has no runtime effect. `None` when absent.
    ///
    /// BIFF tag `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Plunger);

impl Default for Plunger {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            width: 25.0,
            height: 20.0,
            z_adjust: 0.0,
            stroke: 80.0,
            speed_pull: 0.5,
            speed_fire: 80.0,
            plunger_type: PlungerType::Modern,
            anim_frames: 1,
            material: String::default(),
            image: String::default(),
            mech_strength: 85.0,
            is_mech_plunger: false,
            auto_plunger: false,
            park_position: 0.5 / 3.0,
            scatter_velocity: 0.0,
            momentum_xfer: 1.0,
            timer: TimerData::default(),
            is_visible: true,
            is_reflection_enabled: Some(true),
            surface: String::default(),
            name: String::default(),
            tip_shape: "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .84"
                .to_string(),
            rod_diam: 0.6,
            ring_gap: 2.0,
            ring_diam: 0.94,
            ring_width: 3.0,
            spring_diam: 0.77,
            spring_gauge: 1.38,
            spring_loops: 8.0,
            spring_end_loops: 2.5,
            is_locked: false,
            editor_layer: None,
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PlungerJson {
    center: Vertex2D,
    width: f32,
    height: f32,
    z_adjust: f32,
    stroke: f32,
    speed_pull: f32,
    speed_fire: f32,
    plunger_type: PlungerType,
    anim_frames: u32,
    material: String,
    image: String,
    mech_strength: f32,
    is_mech_plunger: bool,
    auto_plunger: bool,
    park_position: f32,
    scatter_velocity: f32,
    momentum_xfer: f32,
    #[serde(flatten)]
    pub timer: TimerData,
    is_visible: bool,
    is_reflection_enabled: Option<bool>,
    surface: String,
    name: String,
    tip_shape: String,
    rod_diam: f32,
    ring_gap: f32,
    ring_diam: f32,
    ring_width: f32,
    spring_diam: f32,
    spring_gauge: f32,
    spring_loops: f32,
    spring_end_loops: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl PlungerJson {
    pub fn from_plunger(plunger: &Plunger) -> Self {
        Self {
            center: plunger.center,
            width: plunger.width,
            height: plunger.height,
            z_adjust: plunger.z_adjust,
            stroke: plunger.stroke,
            speed_pull: plunger.speed_pull,
            speed_fire: plunger.speed_fire,
            plunger_type: plunger.plunger_type.clone(),
            anim_frames: plunger.anim_frames,
            material: plunger.material.clone(),
            image: plunger.image.clone(),
            mech_strength: plunger.mech_strength,
            is_mech_plunger: plunger.is_mech_plunger,
            auto_plunger: plunger.auto_plunger,
            park_position: plunger.park_position,
            scatter_velocity: plunger.scatter_velocity,
            momentum_xfer: plunger.momentum_xfer,
            timer: plunger.timer.clone(),
            is_visible: plunger.is_visible,
            is_reflection_enabled: plunger.is_reflection_enabled,
            surface: plunger.surface.clone(),
            name: plunger.name.clone(),
            tip_shape: plunger.tip_shape.clone(),
            rod_diam: plunger.rod_diam,
            ring_gap: plunger.ring_gap,
            ring_diam: plunger.ring_diam,
            ring_width: plunger.ring_width,
            spring_diam: plunger.spring_diam,
            spring_gauge: plunger.spring_gauge,
            spring_loops: plunger.spring_loops,
            spring_end_loops: plunger.spring_end_loops,
            part_group_name: plunger.part_group_name.clone(),
        }
    }

    pub fn to_plunger(&self) -> Plunger {
        Plunger {
            center: self.center,
            width: self.width,
            height: self.height,
            z_adjust: self.z_adjust,
            stroke: self.stroke,
            speed_pull: self.speed_pull,
            speed_fire: self.speed_fire,
            plunger_type: self.plunger_type.clone(),
            anim_frames: self.anim_frames,
            material: self.material.clone(),
            image: self.image.clone(),
            mech_strength: self.mech_strength,
            is_mech_plunger: self.is_mech_plunger,
            auto_plunger: self.auto_plunger,
            park_position: self.park_position,
            scatter_velocity: self.scatter_velocity,
            momentum_xfer: self.momentum_xfer,
            timer: self.timer.clone(),
            is_visible: self.is_visible,
            is_reflection_enabled: self.is_reflection_enabled,
            surface: self.surface.clone(),
            name: self.name.clone(),
            tip_shape: self.tip_shape.clone(),
            rod_diam: self.rod_diam,
            ring_gap: self.ring_gap,
            ring_diam: self.ring_diam,
            ring_width: self.ring_width,
            spring_diam: self.spring_diam,
            spring_gauge: self.spring_gauge,
            spring_loops: self.spring_loops,
            spring_end_loops: self.spring_end_loops,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
        }
    }
}

impl Serialize for Plunger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PlungerJson::from_plunger(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Plunger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = PlungerJson::deserialize(deserializer)?;
        Ok(json.to_plunger())
    }
}

impl BiffRead for Plunger {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        // for reading to be backwards compatible some fields need to be None by default
        let mut plunger = Plunger {
            is_reflection_enabled: None,
            ..Default::default()
        };
        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    plunger.center = Vertex2D::biff_read(reader)?;
                }
                "WDTH" => {
                    plunger.width = reader.get_f32()?;
                }
                "HIGH" => {
                    plunger.height = reader.get_f32()?;
                }
                "ZADJ" => {
                    plunger.z_adjust = reader.get_f32()?;
                }
                "HPSL" => {
                    plunger.stroke = reader.get_f32()?;
                }
                "SPDP" => {
                    plunger.speed_pull = reader.get_f32()?;
                }
                "SPDF" => {
                    plunger.speed_fire = reader.get_f32()?;
                }
                "TYPE" => {
                    plunger.plunger_type = reader.get_u32()?.into();
                }
                "ANFR" => {
                    plunger.anim_frames = reader.get_u32()?;
                }
                "MATR" => {
                    plunger.material = reader.get_string()?;
                }
                "IMAG" => {
                    plunger.image = reader.get_string()?;
                }
                "MEST" => {
                    plunger.mech_strength = reader.get_f32()?;
                }
                "MECH" => {
                    plunger.is_mech_plunger = reader.get_bool()?;
                }
                "APLG" => {
                    plunger.auto_plunger = reader.get_bool()?;
                }
                "MPRK" => {
                    plunger.park_position = reader.get_f32()?;
                }
                "PSCV" => {
                    plunger.scatter_velocity = reader.get_f32()?;
                }
                "MOMX" => {
                    plunger.momentum_xfer = reader.get_f32()?;
                }
                "VSBL" => {
                    plunger.is_visible = reader.get_bool()?;
                }
                "REEN" => {
                    plunger.is_reflection_enabled = Some(reader.get_bool()?);
                }
                "SURF" => {
                    plunger.surface = reader.get_string()?;
                }
                "NAME" => {
                    plunger.name = reader.get_wide_string()?;
                }
                "TIPS" => {
                    plunger.tip_shape = reader.get_string()?;
                }
                "RODD" => {
                    plunger.rod_diam = reader.get_f32()?;
                }
                "RNGG" => {
                    plunger.ring_gap = reader.get_f32()?;
                }
                "RNGD" => {
                    plunger.ring_diam = reader.get_f32()?;
                }
                "RNGW" => {
                    plunger.ring_width = reader.get_f32()?;
                }
                "SPRD" => {
                    plunger.spring_diam = reader.get_f32()?;
                }
                "SPRG" => {
                    plunger.spring_gauge = reader.get_f32()?;
                }
                "SPRL" => {
                    plunger.spring_loops = reader.get_f32()?;
                }
                "SPRE" => {
                    plunger.spring_end_loops = reader.get_f32()?;
                }

                _ => {
                    if !plunger.timer.biff_read_tag(tag_str, reader)?
                        && !plunger.read_shared_attribute(tag_str, reader)?
                    {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag()?;
                    }
                }
            }
        }
        Ok(plunger)
    }
}

impl BiffWrite for Plunger {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("WDTH", self.width);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_f32("ZADJ", self.z_adjust);
        writer.write_tagged_f32("HPSL", self.stroke);
        writer.write_tagged_f32("SPDP", self.speed_pull);
        writer.write_tagged_f32("SPDF", self.speed_fire);
        writer.write_tagged_u32("TYPE", (&self.plunger_type).into());
        writer.write_tagged_u32("ANFR", self.anim_frames);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_f32("MEST", self.mech_strength);
        writer.write_tagged_bool("MECH", self.is_mech_plunger);
        writer.write_tagged_bool("APLG", self.auto_plunger);
        writer.write_tagged_f32("MPRK", self.park_position);
        writer.write_tagged_f32("PSCV", self.scatter_velocity);
        writer.write_tagged_f32("MOMX", self.momentum_xfer);
        self.timer.biff_write(writer);
        writer.write_tagged_bool("VSBL", self.is_visible);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("TIPS", &self.tip_shape);
        writer.write_tagged_f32("RODD", self.rod_diam);
        writer.write_tagged_f32("RNGG", self.ring_gap);
        writer.write_tagged_f32("RNGD", self.ring_diam);
        writer.write_tagged_f32("RNGW", self.ring_width);
        writer.write_tagged_f32("SPRD", self.spring_diam);
        writer.write_tagged_f32("SPRG", self.spring_gauge);
        writer.write_tagged_f32("SPRL", self.spring_loops);
        writer.write_tagged_f32("SPRE", self.spring_end_loops);

        self.write_shared_attributes(writer);

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
        let plunger = Plunger {
            center: Vertex2D::new(1.0, 2.0),
            width: 1.0,
            height: 1.0,
            z_adjust: 0.1,
            stroke: 2.0,
            speed_pull: 0.5,
            speed_fire: 3.0,
            plunger_type: Faker.fake(),
            anim_frames: 1,
            material: "test material".to_string(),
            image: "test image".to_string(),
            mech_strength: 85.0,
            is_mech_plunger: false,
            auto_plunger: false,
            park_position: 0.5 / 3.0,
            scatter_velocity: 0.0,
            momentum_xfer: 1.0,
            timer: TimerData::default(),
            is_visible: true,
            is_reflection_enabled: Some(true),
            surface: "test surface".to_string(),
            name: "test plunger".to_string(),
            tip_shape: "0 .34; 2 .6; 3 .64; 5 .7; 7 .84; 8 .88; 9 .9; 11 .92; 14 .92; 39 .83"
                .to_string(),
            rod_diam: 0.6,
            ring_gap: 2.0,
            ring_diam: 0.94,
            ring_width: 3.0,
            spring_diam: 0.77,
            spring_gauge: 1.38,
            spring_loops: 8.0,
            spring_end_loops: 2.5,
            is_locked: true,
            editor_layer: Some(0),
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(false),
            part_group_name: Some("test group".to_string()),
        };
        let mut writer = BiffWriter::new();
        Plunger::biff_write(&plunger, &mut writer);
        let plunger_read = Plunger::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(plunger, plunger_read);
    }

    #[test]
    fn test_plunger_type_json() {
        let sizing_type = PlungerType::Modern;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"modern\"");
        let sizing_type_read: PlungerType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(3);
        let sizing_type_read: PlungerType = serde_json::from_value(json).unwrap();
        assert_eq!(PlungerType::Custom, sizing_type_read);
    }

    #[test]
    #[should_panic = "unknown variant `foo`, expected one of `unknown`, `modern`, `flat`, `custom`"]
    fn test_plunger_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: PlungerType = serde_json::from_value(json).unwrap();
    }
}
