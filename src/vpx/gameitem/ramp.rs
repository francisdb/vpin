use super::dragpoint::DragPoint;
use crate::vpx::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use crate::vpx::latin1::Latin1String;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Shape of a ramp, mirroring vpinball's `RampType`.
///
/// Values this library does not know are kept in [`RampType::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum RampType {
    /// `RampTypeFlat`: solid ramp with a floor and walls.
    Flat,
    /// `RampType4Wire`: wire ramp with four wires.
    FourWire,
    /// `RampType2Wire`: wire ramp with two wires.
    TwoWire,
    /// `RampType3WireLeft`: wire ramp with a third wire on the left.
    ThreeWireLeft,
    /// `RampType3WireRight`: wire ramp with a third wire on the right.
    ThreeWireRight,
    /// `RampType1Wire`: wire ramp with a single wire.
    OneWire,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(#[cfg_attr(test, proptest(strategy = "6..=u32::MAX"))] u32),
}
impl From<u32> for RampType {
    fn from(value: u32) -> Self {
        match value {
            0 => RampType::Flat,
            1 => RampType::FourWire,
            2 => RampType::TwoWire,
            3 => RampType::ThreeWireLeft,
            4 => RampType::ThreeWireRight,
            5 => RampType::OneWire,
            other => {
                warn!("Unknown RampType value {other}, keeping it as is");
                RampType::Other(other)
            }
        }
    }
}
impl From<&RampType> for u32 {
    fn from(value: &RampType) -> Self {
        match value {
            RampType::Flat => 0,
            RampType::FourWire => 1,
            RampType::TwoWire => 2,
            RampType::ThreeWireLeft => 3,
            RampType::ThreeWireRight => 4,
            RampType::OneWire => 5,
            RampType::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`RampType::Other`]
impl Serialize for RampType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RampType::Flat => serializer.serialize_str("flat"),
            RampType::FourWire => serializer.serialize_str("four_wire"),
            RampType::TwoWire => serializer.serialize_str("two_wire"),
            RampType::ThreeWireLeft => serializer.serialize_str("three_wire_left"),
            RampType::ThreeWireRight => serializer.serialize_str("three_wire_right"),
            RampType::OneWire => serializer.serialize_str("one_wire"),
            RampType::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for RampType {
    fn deserialize<D>(deserializer: D) -> Result<RampType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RampTypeVisitor;
        impl serde::de::Visitor<'_> for RampTypeVisitor {
            type Value = RampType;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a RampType as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<RampType, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(RampType::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<RampType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "flat" => Ok(RampType::Flat),
                    "four_wire" => Ok(RampType::FourWire),
                    "two_wire" => Ok(RampType::TwoWire),
                    "three_wire_left" => Ok(RampType::ThreeWireLeft),
                    "three_wire_right" => Ok(RampType::ThreeWireRight),
                    "one_wire" => Ok(RampType::OneWire),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "flat",
                            "four_wire",
                            "two_wire",
                            "three_wire_left",
                            "three_wire_right",
                            "one_wire",
                        ],
                    )),
                }
            }
        }
        deserializer.deserialize_any(RampTypeVisitor)
    }
}
#[cfg(test)]
mod ramp_type_open_enum_tests {
    use super::RampType;

    #[test]
    fn unknown_value_round_trips() {
        let value = RampType::from(4_000_000_000);
        assert_eq!(value, RampType::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: RampType = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(serde_json::from_value::<RampType>(serde_json::json!("no_such_variant")).is_err());
    }
}

/// A ramp: a raised path the ball travels on, either a flat ramp with a
/// floor and optional side walls or a wire (habitrail) ramp of one to four
/// wires, chosen by [`ramp_type`](Self::ramp_type).
///
/// The path follows [`drag_points`](Self::drag_points), rising from
/// [`height_bottom`](Self::height_bottom) to
/// [`height_top`](Self::height_top) and going from
/// [`width_bottom`](Self::width_bottom) to [`width_top`](Self::width_top)
/// along its length. The wall fields (`left_wall_height`,
/// `right_wall_height`, the `*_visible` heights and
/// [`image_walls`](Self::image_walls)) only apply to a flat ramp; a wire
/// ramp uses [`wire_diameter`](Self::wire_diameter),
/// [`wire_distance_x`](Self::wire_distance_x) and
/// [`wire_distance_y`](Self::wire_distance_y) and gets fixed collision
/// walls. Physically every ramp is a flat floor with two side walls.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Ramp {
    /// Height of the ramp floor at its start (the first drag point), in VPU
    /// above the playfield.
    ///
    /// The floor rises linearly along the path length to
    /// [`height_top`](Self::height_top). vpinball default `0.0`.
    ///
    /// BIFF tag `HTBT`
    pub height_bottom: f32,
    /// Height of the ramp floor at its end (the last drag point), in VPU
    /// above the playfield. vpinball default `50.0`.
    ///
    /// BIFF tag `HTTP`
    pub height_top: f32,
    /// Width of a flat ramp at its start, in VPU, interpolated along the
    /// path to [`width_top`](Self::width_top).
    ///
    /// Wire ramps take their width from
    /// [`wire_distance_x`](Self::wire_distance_x) or
    /// [`wire_diameter`](Self::wire_diameter) instead, but a ramp with both
    /// widths `0.0` is not rendered at all. vpinball default `75.0`.
    ///
    /// BIFF tag `WDBT`
    pub width_bottom: f32,
    /// Width of a flat ramp at its end, in VPU; see
    /// [`width_bottom`](Self::width_bottom). vpinball default `60.0`.
    ///
    /// BIFF tag `WDTP`
    pub width_top: f32,
    /// Name of the material rendered on the ramp; empty for the default
    /// material.
    ///
    /// BIFF tag `MATR`
    pub material: Latin1String,
    /// Shape of the ramp, see [`RampType`]: a flat ramp or one of the wire
    /// ramps. vpinball default [`RampType::Flat`].
    ///
    /// BIFF tag `TYPE`
    pub ramp_type: RampType,
    /// Name of the ramp, the identifier used from VBScript. Stored as a
    /// UTF-16 string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Name of the texture on the ramp floor and, with
    /// [`image_walls`](Self::image_walls), on its walls; empty for none.
    /// Mapped as chosen by [`image_alignment`](Self::image_alignment).
    ///
    /// BIFF tag `IMAG`
    pub image: Latin1String,
    /// Controls how the texture is mapped onto the ramp surface.
    /// - [`World`](RampImageAlignment::World): UVs are based on table coordinates.
    /// - [`Wrap`](RampImageAlignment::Wrap): UVs are based on the ramp bounding box
    ///   (texture is stretched to fit).
    ///
    /// Also used on: [`Flasher`](crate::vpx::gameitem::flasher::Flasher).
    /// BIFF tag: `ALGN`
    pub image_alignment: RampImageAlignment,
    /// Whether the [`image`](Self::image) also covers the side walls of a
    /// flat ramp. When `false` the walls only show the material. vpinball
    /// default `true`.
    ///
    /// BIFF tag `IMGW`
    pub image_walls: bool,
    /// Height of the left collision wall of a flat ramp above the floor,
    /// in VPU; `0.0` disables the wall.
    ///
    /// Physics only: the rendered wall uses
    /// [`left_wall_height_visible`](Self::left_wall_height_visible). Wire
    /// ramps ignore it and get fixed walls: `31.0` for one and two wires,
    /// `62.0` for four wires, and `62.0` on the side with the third wire
    /// and `18.5` on the other side for three wires. vpinball default
    /// `62.0`.
    ///
    /// BIFF tag `WLHL`
    pub left_wall_height: f32,
    /// Height of the right collision wall of a flat ramp above the floor,
    /// in VPU; `0.0` disables the wall. See
    /// [`left_wall_height`](Self::left_wall_height). vpinball default
    /// `62.0`.
    ///
    /// BIFF tag `WLHR`
    pub right_wall_height: f32,
    /// Height of the rendered left wall of a flat ramp above the floor, in
    /// VPU; `0.0` draws no wall.
    ///
    /// Rendering only: collision uses
    /// [`left_wall_height`](Self::left_wall_height). vpinball default
    /// `30.0`.
    ///
    /// BIFF tag `WVHL`
    pub left_wall_height_visible: f32,
    /// Height of the rendered right wall of a flat ramp above the floor,
    /// in VPU; `0.0` draws no wall. See
    /// [`left_wall_height_visible`](Self::left_wall_height_visible).
    /// vpinball default `30.0`.
    ///
    /// BIFF tag `WVHR`
    pub right_wall_height_visible: f32,
    /// Whether the ramp fires `Hit` events to the script when the ball hits
    /// it at least as fast as [`threshold`](Self::threshold).
    ///
    /// `None` when the record is absent: it was added in April 2017 (after
    /// 10.3), older tables have neither `HTEV` nor `THRS` and vpinball
    /// then keeps its default `false`.
    ///
    /// BIFF tag `HTEV`
    pub hit_event: Option<bool>,
    /// Minimum ball speed into the ramp surface for a `Hit` event, in
    /// vpinball velocity units.
    ///
    /// Only used with [`hit_event`](Self::hit_event). `None` when the
    /// record is absent (older tables, see `hit_event`); vpinball default
    /// `2.0`.
    ///
    /// BIFF tag `THRS`
    pub threshold: Option<f32>,
    /// Elasticity of the ramp surface, the fraction of the ball speed kept
    /// on a bounce (`0.0` to `1.0`).
    ///
    /// Used when [`overwrite_physics`](Self::overwrite_physics) is `true`
    /// (or absent); otherwise the elasticity of
    /// [`physics_material`](Self::physics_material) applies. vpinball
    /// default `0.3`.
    ///
    /// BIFF tag `ELAS`
    pub elasticity: f32,
    /// Friction of the ramp surface, `0.0` (none) to `1.0`.
    ///
    /// Used when [`overwrite_physics`](Self::overwrite_physics) is `true`
    /// (or absent); otherwise the friction of
    /// [`physics_material`](Self::physics_material) applies. vpinball
    /// default `0.3`.
    ///
    /// BIFF tag `RFCT`
    pub friction: f32,
    /// Scatter angle in degrees, the maximum random deviation of the ball's
    /// bounce direction off the ramp.
    ///
    /// Used when [`overwrite_physics`](Self::overwrite_physics) is `true`
    /// (or absent); otherwise the scatter angle of
    /// [`physics_material`](Self::physics_material) applies. vpinball
    /// default `0.0`.
    ///
    /// BIFF tag `RSCT`
    pub scatter: f32,
    /// Whether the ball collides with the ramp. vpinball default `true`.
    ///
    /// BIFF tag `CLDR`
    pub is_collidable: bool,
    /// Whether the ramp is rendered. vpinball default `true`.
    ///
    /// BIFF tag `RVIS`
    pub is_visible: bool,
    /// Offset applied when depth-sorting transparent and overlapping objects.
    /// Higher values move the object "further away" in the sort order, causing it
    /// to render behind objects with lower bias.
    /// Also used on: [`Flasher`](crate::vpx::gameitem::flasher::Flasher), [`Primitive`](crate::vpx::gameitem::primitive::Primitive), [`Light`](crate::vpx::gameitem::light::Light), [`HitTarget`](crate::vpx::gameitem::hittarget::HitTarget).
    /// BIFF tag: `RADB`
    pub depth_bias: f32,
    /// Diameter of the wires of a wire ramp, in VPU, and the full width of
    /// a [`RampType::OneWire`] ramp. Flat ramps ignore it. vpinball
    /// default `8.0`.
    ///
    /// BIFF tag `RADI`
    pub wire_diameter: f32,
    /// Horizontal distance between the two lower wires of a wire ramp, in
    /// VPU; this is the ramp width of every wire ramp but the one-wire
    /// ramp. Flat ramps ignore it. vpinball default `38.0`.
    ///
    /// BIFF tag `RADX`
    pub wire_distance_x: f32,
    /// Vertical spacing of the upper wires of a three or four wire ramp, in
    /// VPU: the upper wires are drawn `wire_distance_y / 2` above the lower
    /// ones. Other ramp types ignore it. vpinball default `88.0`.
    ///
    /// BIFF tag `RADY`
    pub wire_distance_y: f32,
    /// Whether this ramp appears in playfield reflections.
    ///
    /// When `true`, the ball is rendered in the reflection pass.
    /// When `false`, the ball won't appear as a reflection on the playfield.
    ///
    /// BIFF tag: `REEN` (was missing in 10.01)
    pub is_reflection_enabled: Option<bool>,
    /// Name of the material whose elasticity, friction and scatter angle
    /// apply when [`overwrite_physics`](Self::overwrite_physics) is
    /// `false`; empty for none.
    ///
    /// `None` when the record is absent (older tables, before physics
    /// materials existed).
    ///
    /// BIFF tag `MAPH`
    pub physics_material: Option<Latin1String>,
    /// Whether the ramp's own [`elasticity`](Self::elasticity),
    /// [`friction`](Self::friction) and [`scatter`](Self::scatter) are used
    /// instead of those of [`physics_material`](Self::physics_material).
    ///
    /// `None` when the record is absent (older tables, before physics
    /// materials existed); vpinball then keeps its default `true`.
    ///
    /// BIFF tag `OVPH`
    pub overwrite_physics: Option<bool>,

    /// Control points of the ramp path, in order from the bottom end to
    /// the top end; at least two are needed.
    ///
    /// Each point can be a smooth curve point or a corner, see
    /// [`DragPoint`]. Written after an empty `PNTS` marker as one `DPNT`
    /// record per point.
    pub drag_points: Vec<DragPoint>,

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
    pub editor_layer_name: Option<Latin1String>,
    /// Whether the legacy editor layer is shown in the editor.
    /// Editor-only; has no runtime effect. `None` when absent.
    ///
    /// BIFF tag `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<Latin1String>,
}
impl_shared_attributes!(Ramp);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RampJson {
    height_bottom: f32,
    height_top: f32,
    width_bottom: f32,
    width_top: f32,
    material: Latin1String,
    #[serde(flatten)]
    pub timer: TimerData,
    ramp_type: RampType,
    name: String,
    image: Latin1String,
    image_alignment: RampImageAlignment,
    image_walls: bool,
    left_wall_height: f32,
    right_wall_height: f32,
    left_wall_height_visible: f32,
    right_wall_height_visible: f32,
    hit_event: Option<bool>,
    threshold: Option<f32>,
    elasticity: f32,
    friction: f32,
    scatter: f32,
    is_collidable: bool,
    is_visible: bool,
    depth_bias: f32,
    wire_diameter: f32,
    wire_distance_x: f32,
    wire_distance_y: f32,
    is_reflection_enabled: Option<bool>,
    physics_material: Option<Latin1String>,
    overwrite_physics: Option<bool>,
    drag_points: Vec<DragPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<Latin1String>,
}

impl RampJson {
    fn from_ramp(ramp: &Ramp) -> Self {
        Self {
            height_bottom: ramp.height_bottom,
            height_top: ramp.height_top,
            width_bottom: ramp.width_bottom,
            width_top: ramp.width_top,
            material: ramp.material.clone(),
            timer: ramp.timer.clone(),
            ramp_type: ramp.ramp_type.clone(),
            name: ramp.name.clone(),
            image: ramp.image.clone(),
            image_alignment: ramp.image_alignment.clone(),
            image_walls: ramp.image_walls,
            left_wall_height: ramp.left_wall_height,
            right_wall_height: ramp.right_wall_height,
            left_wall_height_visible: ramp.left_wall_height_visible,
            right_wall_height_visible: ramp.right_wall_height_visible,
            hit_event: ramp.hit_event,
            threshold: ramp.threshold,
            elasticity: ramp.elasticity,
            friction: ramp.friction,
            scatter: ramp.scatter,
            is_collidable: ramp.is_collidable,
            is_visible: ramp.is_visible,
            depth_bias: ramp.depth_bias,
            wire_diameter: ramp.wire_diameter,
            wire_distance_x: ramp.wire_distance_x,
            wire_distance_y: ramp.wire_distance_y,
            is_reflection_enabled: ramp.is_reflection_enabled,
            physics_material: ramp.physics_material.clone(),
            overwrite_physics: ramp.overwrite_physics,
            drag_points: ramp.drag_points.clone(),
            part_group_name: ramp.part_group_name.clone(),
        }
    }

    fn to_ramp(&self) -> Ramp {
        Ramp {
            height_bottom: self.height_bottom,
            height_top: self.height_top,
            width_bottom: self.width_bottom,
            width_top: self.width_top,
            material: self.material.clone(),
            timer: self.timer.clone(),
            ramp_type: self.ramp_type.clone(),
            name: self.name.clone(),
            image: self.image.clone(),
            image_alignment: self.image_alignment.clone(),
            image_walls: self.image_walls,
            left_wall_height: self.left_wall_height,
            right_wall_height: self.right_wall_height,
            left_wall_height_visible: self.left_wall_height_visible,
            right_wall_height_visible: self.right_wall_height_visible,
            hit_event: self.hit_event,
            threshold: self.threshold,
            elasticity: self.elasticity,
            friction: self.friction,
            scatter: self.scatter,
            is_collidable: self.is_collidable,
            is_visible: self.is_visible,
            depth_bias: self.depth_bias,
            wire_diameter: self.wire_diameter,
            wire_distance_x: self.wire_distance_x,
            wire_distance_y: self.wire_distance_y,
            is_reflection_enabled: self.is_reflection_enabled,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            drag_points: self.drag_points.clone(),
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

impl Serialize for Ramp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RampJson::from_ramp(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Ramp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ramp_json = RampJson::deserialize(deserializer)?;
        Ok(ramp_json.to_ramp())
    }
}

impl Default for Ramp {
    fn default() -> Self {
        Self {
            height_bottom: 0.0,
            height_top: 50.0,
            width_bottom: 75.0,
            width_top: 60.0,
            material: Default::default(),
            timer: TimerData::default(),
            ramp_type: RampType::Flat,
            name: Default::default(),
            image: Default::default(),
            image_alignment: RampImageAlignment::World,
            image_walls: true,
            left_wall_height: 62.0,
            right_wall_height: 62.0,
            left_wall_height_visible: 30.0,
            right_wall_height_visible: 30.0,
            hit_event: None,
            threshold: None,
            elasticity: Default::default(),
            friction: Default::default(),
            scatter: Default::default(),
            is_collidable: true,
            is_visible: true,
            depth_bias: 0.0,
            wire_diameter: 8.0,
            wire_distance_x: 38.0,
            wire_distance_y: 88.0,
            is_reflection_enabled: None, // true,
            physics_material: None,
            overwrite_physics: None, // true;
            drag_points: Default::default(),
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl BiffRead for Ramp {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut ramp = Ramp::default();

        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "HTBT" => {
                    ramp.height_bottom = reader.get_f32()?;
                }
                "HTTP" => {
                    ramp.height_top = reader.get_f32()?;
                }
                "WDBT" => {
                    ramp.width_bottom = reader.get_f32()?;
                }
                "WDTP" => {
                    ramp.width_top = reader.get_f32()?;
                }
                "MATR" => {
                    ramp.material = reader.get_latin1_string()?;
                }
                "TYPE" => {
                    ramp.ramp_type = reader.get_u32()?.into();
                }
                "NAME" => {
                    ramp.name = reader.get_wide_string()?;
                }
                "IMAG" => {
                    ramp.image = reader.get_latin1_string()?;
                }
                "ALGN" => {
                    ramp.image_alignment = reader.get_u32()?.into();
                }
                "IMGW" => {
                    ramp.image_walls = reader.get_bool()?;
                }
                "WLHL" => {
                    ramp.left_wall_height = reader.get_f32()?;
                }
                "WLHR" => {
                    ramp.right_wall_height = reader.get_f32()?;
                }
                "WVHL" => {
                    ramp.left_wall_height_visible = reader.get_f32()?;
                }
                "WVHR" => {
                    ramp.right_wall_height_visible = reader.get_f32()?;
                }
                "HTEV" => {
                    ramp.hit_event = Some(reader.get_bool()?);
                }
                "THRS" => {
                    ramp.threshold = Some(reader.get_f32()?);
                }
                "ELAS" => {
                    ramp.elasticity = reader.get_f32()?;
                }
                "RFCT" => {
                    ramp.friction = reader.get_f32()?;
                }
                "RSCT" => {
                    ramp.scatter = reader.get_f32()?;
                }
                "CLDR" => {
                    ramp.is_collidable = reader.get_bool()?;
                }
                "RVIS" => {
                    ramp.is_visible = reader.get_bool()?;
                }
                "RADB" => {
                    ramp.depth_bias = reader.get_f32()?;
                }
                "RADI" => {
                    ramp.wire_diameter = reader.get_f32()?;
                }
                "RADX" => {
                    ramp.wire_distance_x = reader.get_f32()?;
                }
                "RADY" => {
                    ramp.wire_distance_y = reader.get_f32()?;
                }
                "REEN" => {
                    ramp.is_reflection_enabled = Some(reader.get_bool()?);
                }
                "MAPH" => {
                    ramp.physics_material = Some(reader.get_latin1_string()?);
                }
                "OVPH" => {
                    ramp.overwrite_physics = Some(reader.get_bool()?);
                }
                "PNTS" => {
                    // this is just a tag with no data
                }
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    ramp.drag_points.push(point?);
                }
                _ => {
                    if !ramp.timer.biff_read_tag(tag_str, reader)?
                        && !ramp.read_shared_attribute(tag_str, reader)?
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
        Ok(ramp)
    }
}

impl BiffWrite for Ramp {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_f32("HTBT", self.height_bottom);
        writer.write_tagged_f32("HTTP", self.height_top);
        writer.write_tagged_f32("WDBT", self.width_bottom);
        writer.write_tagged_f32("WDTP", self.width_top);
        writer.write_tagged_string("MATR", &self.material);
        self.timer.biff_write(writer);
        writer.write_tagged_u32("TYPE", (&self.ramp_type).into());
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_u32("ALGN", (&self.image_alignment).into());
        writer.write_tagged_bool("IMGW", self.image_walls);
        writer.write_tagged_f32("WLHL", self.left_wall_height);
        writer.write_tagged_f32("WLHR", self.right_wall_height);
        writer.write_tagged_f32("WVHL", self.left_wall_height_visible);
        writer.write_tagged_f32("WVHR", self.right_wall_height_visible);
        if let Some(hit_event) = self.hit_event {
            writer.write_tagged_bool("HTEV", hit_event);
        }
        if let Some(threshold) = self.threshold {
            writer.write_tagged_f32("THRS", threshold);
        }
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("RFCT", self.friction);
        writer.write_tagged_f32("RSCT", self.scatter);
        writer.write_tagged_bool("CLDR", self.is_collidable);
        writer.write_tagged_bool("RVIS", self.is_visible);
        writer.write_tagged_f32("RADB", self.depth_bias);
        writer.write_tagged_f32("RADI", self.wire_diameter);
        writer.write_tagged_f32("RADX", self.wire_distance_x);
        writer.write_tagged_f32("RADY", self.wire_distance_y);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        if let Some(physics_material) = &self.physics_material {
            writer.write_tagged_string("MAPH", physics_material);
        }
        if let Some(overwrite_physics) = self.overwrite_physics {
            writer.write_tagged_bool("OVPH", overwrite_physics);
        }

        self.write_shared_attributes(writer);

        writer.write_marker_tag("PNTS");
        // many of these
        for point in &self.drag_points {
            writer.write_tagged("DPNT", point)
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use crate::vpx::test_support::debug;
    use proptest::prelude::*;

    use super::*;
    use pretty_assertions::assert_eq;

    proptest! {
        #[test]
        fn any_ramp_round_trips_through_its_records(ramp in any::<Ramp>()) {
            let mut writer = BiffWriter::new();
            Ramp::biff_write(&ramp, &mut writer);
            let read = Ramp::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
            prop_assert_eq!(debug(&ramp), debug(&read));
        }
    }

    #[test]
    fn test_ramp_type_json() {
        let sizing_type = RampType::FourWire;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"four_wire\"");
        let sizing_type_read: RampType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RampType = serde_json::from_value(json).unwrap();
        assert_eq!(RampType::Flat, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `flat`, `four_wire`, `two_wire`, `three_wire_left`, `three_wire_right`, `one_wire`\", line: 0, column: 0)"]
    fn test_shadow_mode_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: RampType = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn test_image_alignment_json() {
        let sizing_type = RampImageAlignment::Wrap;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"wrap\"");
        let sizing_type_read: RampImageAlignment = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RampImageAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(RampImageAlignment::World, sizing_type_read);
    }

    #[test]
    #[should_panic = "unknown variant `foo`, expected one of `world`, `wrap`, `unknown`"]
    fn test_image_alignment_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: RampImageAlignment = serde_json::from_value(json).unwrap();
    }
}
