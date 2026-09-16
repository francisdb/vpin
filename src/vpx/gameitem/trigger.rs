use super::{dragpoint::DragPoint, vertex2d::Vertex2D};
use crate::vpx::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The visual shape of a trigger.
///
/// Values this library does not know are kept in [`TriggerShape::Other`]
/// so the table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone, Default)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum TriggerShape {
    /// No visible mesh (invisible trigger): 0
    None,
    /// Simple wire trigger: 1
    #[default]
    WireA,
    /// Star-shaped trigger (uses `radius` for scaling): 2
    Star,
    /// Wire trigger rotated -23° around X axis: 3
    WireB,
    /// Button trigger (uses `radius` for scaling, z offset +5): 4
    Button,
    /// Wire trigger rotated 140° around X axis, z offset -19: 5
    WireC,
    /// D-shaped wire trigger: 6
    WireD,
    /// Inder-style trigger: 7
    Inder,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(#[cfg_attr(test, proptest(strategy = "8..=u32::MAX"))] u32),
}

#[cfg(test)]
mod trigger_shape_open_enum_tests {
    use super::TriggerShape;

    #[test]
    fn unknown_value_round_trips() {
        let value = TriggerShape::from(4_000_000_000);
        assert_eq!(value, TriggerShape::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: TriggerShape = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<TriggerShape>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

impl From<u32> for TriggerShape {
    fn from(value: u32) -> Self {
        match value {
            0 => TriggerShape::None,
            1 => TriggerShape::WireA,
            2 => TriggerShape::Star,
            3 => TriggerShape::WireB,
            4 => TriggerShape::Button,
            5 => TriggerShape::WireC,
            6 => TriggerShape::WireD,
            7 => TriggerShape::Inder,
            other => {
                warn!("Unknown TriggerShape value {other}, keeping it as is");
                TriggerShape::Other(other)
            }
        }
    }
}

impl From<&TriggerShape> for u32 {
    fn from(value: &TriggerShape) -> Self {
        match value {
            TriggerShape::None => 0,
            TriggerShape::WireA => 1,
            TriggerShape::Star => 2,
            TriggerShape::WireB => 3,
            TriggerShape::Button => 4,
            TriggerShape::WireC => 5,
            TriggerShape::WireD => 6,
            TriggerShape::Inder => 7,
            TriggerShape::Other(value) => *value,
        }
    }
}

/// Serialize as lowercase string
impl Serialize for TriggerShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TriggerShape::None => serializer.serialize_str("none"),
            TriggerShape::WireA => serializer.serialize_str("wire_a"),
            TriggerShape::Star => serializer.serialize_str("star"),
            TriggerShape::WireB => serializer.serialize_str("wire_b"),
            TriggerShape::Button => serializer.serialize_str("button"),
            TriggerShape::WireC => serializer.serialize_str("wire_c"),
            TriggerShape::WireD => serializer.serialize_str("wire_d"),
            TriggerShape::Inder => serializer.serialize_str("inder"),
            TriggerShape::Other(value) => serializer.serialize_u32(*value),
        }
    }
}

/// Deserialize from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for TriggerShape {
    fn deserialize<D>(deserializer: D) -> Result<TriggerShape, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TriggerShapeVisitor;

        impl serde::de::Visitor<'_> for TriggerShapeVisitor {
            type Value = TriggerShape;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TriggerShape")
            }

            fn visit_u64<E>(self, value: u64) -> Result<TriggerShape, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in a u32",
                    )
                })?;
                Ok(TriggerShape::from(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<TriggerShape, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "none" => Ok(TriggerShape::None),
                    "wire_a" => Ok(TriggerShape::WireA),
                    "star" => Ok(TriggerShape::Star),
                    "wire_b" => Ok(TriggerShape::WireB),
                    "button" => Ok(TriggerShape::Button),
                    "wire_c" => Ok(TriggerShape::WireC),
                    "wire_d" => Ok(TriggerShape::WireD),
                    "inder" => Ok(TriggerShape::Inder),

                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "none", "wire_a", "star", "wire_b", "button", "wire_c", "wire_d",
                            "inder",
                        ],
                    )),
                }
            }
        }

        deserializer.deserialize_any(TriggerShapeVisitor)
    }
}

/// A trigger: a switch on the playfield that fires `_Hit` when the ball
/// rolls onto it and `_Unhit` when it leaves, with an optional animated
/// wire, star or button mesh.
///
/// The hit area is the polygon of [`drag_points`](Self::drag_points) (line
/// segments plus a polygon collider) or, for the Star and Button shapes, a
/// circle of [`radius`](Self::radius); it starts at the height of
/// [`surface`](Self::surface) and is [`hit_height`](Self::hit_height) VPU
/// high.
///
/// The record is written by `Trigger::Save` and read by `Trigger::Load` in
/// vpinball's `src/parts/trigger.cpp`.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Trigger {
    /// Name of the trigger, its identifier in the editor and in scripts.
    /// Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Position of the trigger in table coordinates (VPU).
    ///
    /// The mesh and the circular hit area are centered here; the drag
    /// points of the polygon hit area are absolute table coordinates.
    ///
    /// BIFF tag `VCEN`
    pub center: Vertex2D,

    /// Radius of the trigger in VPU (Visual Pinball Units).
    ///
    /// Used for:
    /// - Hit detection: defines the circular collision area
    /// - Mesh scaling for Button and Star shapes: the mesh is uniformly scaled
    ///   by this value in x, y, and z directions
    /// - Animation limits for Button and Star shapes
    ///
    /// For wire-type triggers (WireA, WireB, WireC, WireD, Inder), use `scale_x`
    /// and `scale_y` instead for mesh scaling.
    ///
    /// Default: 25.0
    ///
    /// BIFF tag: RADI
    pub radius: f32,
    /// Rotation of the mesh around the Z axis, in degrees. Has no effect on
    /// the hit area. Default: `0.0`.
    ///
    /// BIFF tag `ROTA`
    pub rotation: f32,
    /// Wire thickness for wire-type triggers (WireA, WireB, WireC, WireD, Inder).
    ///
    /// This value (in VPU) is added to each vertex position along its normal direction,
    /// effectively making the wire mesh thicker or thinner.
    /// Only applies to wire-based trigger shapes, ignored for Star and Button.
    ///
    /// A value of 0.0 means the default (no vertex offset).
    ///
    /// Default: 0.0 (normal size based on the mesh)
    ///
    /// BIFF tag: WITI (was missing in 10.01)
    pub wire_thickness: Option<f32>,
    /// Scale of the wire meshes (WireA, WireB, WireC, WireD, Inder) along
    /// X; the Star and Button shapes use [`radius`](Self::radius) instead.
    /// Has no effect on the hit area. Default: `1.0`.
    ///
    /// BIFF tag `SCAX`
    pub scale_x: f32,
    /// Scale of the wire meshes along Y, see [`scale_x`](Self::scale_x).
    /// Default: `1.0`.
    ///
    /// BIFF tag `SCAY`
    pub scale_y: f32,
    /// Name of the table material used to render the mesh. Empty selects
    /// the default material.
    ///
    /// BIFF tag `MATR`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub material: String,
    /// Name of the surface (ramp or wall top) this trigger sits on.
    /// Used to determine the trigger's base height (z position).
    /// If empty, the trigger sits on the playfield.
    /// BIFF tag: SURF
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub surface: String,

    /// Whether the mesh is rendered; the hit area is not affected.
    /// Default: `true`.
    ///
    /// BIFF tag `VSBL`
    pub is_visible: bool,
    /// Whether the trigger reacts to the ball at table start (`Enabled`
    /// in script). Default: `true`.
    ///
    /// BIFF tag `EBLD`
    pub is_enabled: bool,
    /// Height of the hit volume above its base, in VPU.
    ///
    /// The line segments of a polygon trigger use `hit_height - 8` so they
    /// trigger at the same height as the circular shapes. Default: `50.0`.
    ///
    /// BIFF tag `THOT`
    pub hit_height: f32,

    /// The visual shape of the trigger.
    ///
    /// Determines which mesh is used for rendering and how scaling is applied.
    /// Wire-type triggers (WireA/B/C/D/Inder) use `scale_x`/`scale_y` for scaling
    /// and support `wire_thickness`. Star and Button use `radius` for uniform scaling.
    ///
    /// Default: TriggerShape::WireA
    ///
    /// BIFF tag: SHAP
    pub shape: TriggerShape,

    /// Animation speed multiplier for the trigger's hit/unhit animation.
    ///
    /// Controls how fast the trigger moves when activated. The animation offset
    /// is updated each frame by: `offset += delta_time_ms * anim_speed`
    ///
    /// Higher values make the trigger animate faster. A value of 1.0 is the
    /// default speed.
    ///
    /// Default: 1.0
    ///
    /// BIFF tag: ANSP
    pub anim_speed: f32,

    /// Whether this trigger appears in playfield reflections.
    ///
    /// When `true`, the ball is rendered in the reflection pass.
    /// When `false`, the ball won't appear as a reflection on the playfield.
    ///
    /// BIFF tag: `REEN` (was missing in 10.01)
    pub is_reflection_enabled: Option<bool>,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    /// Whether the item is locked in the editor to prevent accidental
    /// moving or editing. Editor-only; has no runtime effect.
    ///
    /// BIFF tag `LOCK`
    pub is_locked: bool,
    /// Legacy editor layer index (0-based, at most 11). Editor-only.
    ///
    /// Superseded by part groups in 10.8.1, see
    /// [`part_group_name`](Self::part_group_name); vpinball still writes
    /// it, as the index of the item's root group among the root groups, so
    /// older versions can open the file. `None` when the record is absent.
    ///
    /// BIFF tag `LAYR`
    pub editor_layer: Option<u32>,
    /// Name of the editor layer (10.7 named layers). Editor-only.
    ///
    /// Defaults to `"Layer_{editor_layer + 1}"`. Since 10.8.1 vpinball
    /// writes the name of the item's root part group here, for older
    /// versions, and on read maps it to a group when no `GRUP` record
    /// follows. `None` when the record is absent.
    ///
    /// BIFF tag `LANR`
    #[cfg_attr(
        test,
        proptest(strategy = "proptest::option::of(crate::vpx::test_support::latin1_string())")
    )]
    pub editor_layer_name: Option<String>,
    /// Whether the item is shown in the editor (the 10.7 layer visibility,
    /// stored per item). Editor-only; has no runtime effect. `None` when
    /// the record is absent.
    ///
    /// BIFF tag `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Name of the part group the item belongs to. Added in 10.8.1.
    ///
    /// Part groups replace the editor layers; see
    /// [`PartGroup`](crate::vpx::gameitem::partgroup::PartGroup). `None`
    /// when the record is absent (file older than 10.8.1, or an item that
    /// is not in a group).
    ///
    /// BIFF tag `GRUP`
    #[cfg_attr(
        test,
        proptest(strategy = "proptest::option::of(crate::vpx::test_support::latin1_string())")
    )]
    pub part_group_name: Option<String>,

    /// Control points of the polygon that forms the hit area of the wire
    /// shapes, in table coordinates (VPU); the Star and Button shapes use a
    /// circle of [`radius`](Self::radius) instead. The rendered mesh does
    /// not follow these points. Written last in the record, after the
    /// shared attributes.
    ///
    /// BIFF tag `DPNT` (one record per point)
    pub drag_points: Vec<DragPoint>,
}
impl_shared_attributes!(Trigger);

#[derive(Serialize, Deserialize)]
struct TriggerJson {
    center: Vertex2D,
    radius: f32,
    rotation: f32,
    wire_thickness: Option<f32>,
    scale_x: f32,
    scale_y: f32,
    #[serde(flatten)]
    pub timer: TimerData,
    material: String,
    surface: String,
    is_visible: bool,
    is_enabled: bool,
    hit_height: f32,
    name: String,
    shape: TriggerShape,
    anim_speed: f32,
    is_reflection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
    drag_points: Vec<DragPoint>,
}

impl TriggerJson {
    pub fn from_trigger(trigger: &Trigger) -> Self {
        Self {
            center: trigger.center,
            radius: trigger.radius,
            rotation: trigger.rotation,
            wire_thickness: trigger.wire_thickness,
            scale_x: trigger.scale_x,
            scale_y: trigger.scale_y,
            timer: trigger.timer.clone(),
            material: trigger.material.clone(),
            surface: trigger.surface.clone(),
            is_visible: trigger.is_visible,
            is_enabled: trigger.is_enabled,
            hit_height: trigger.hit_height,
            name: trigger.name.clone(),
            shape: trigger.shape.clone(),
            anim_speed: trigger.anim_speed,
            is_reflection_enabled: trigger.is_reflection_enabled,
            part_group_name: trigger.part_group_name.clone(),
            drag_points: trigger.drag_points.clone(),
        }
    }
    pub fn to_trigger(&self) -> Trigger {
        Trigger {
            center: self.center,
            radius: self.radius,
            rotation: self.rotation,
            wire_thickness: self.wire_thickness,
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            timer: self.timer.clone(),
            material: self.material.clone(),
            surface: self.surface.clone(),
            is_visible: self.is_visible,
            is_enabled: self.is_enabled,
            hit_height: self.hit_height,
            name: self.name.clone(),
            shape: self.shape.clone(),
            anim_speed: self.anim_speed,
            is_reflection_enabled: self.is_reflection_enabled,
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

impl Serialize for Trigger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TriggerJson::from_trigger(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Trigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let trigger_json = TriggerJson::deserialize(deserializer)?;
        Ok(trigger_json.to_trigger())
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Trigger {
            center: Default::default(),
            radius: 25.0,
            rotation: Default::default(),
            wire_thickness: Default::default(),
            scale_x: Default::default(),
            scale_y: Default::default(),
            timer: TimerData::default(),
            material: Default::default(),
            surface: Default::default(),
            is_visible: true,
            is_enabled: true,
            hit_height: 50.0,
            name: Default::default(),
            shape: TriggerShape::WireA,
            anim_speed: Default::default(),
            is_reflection_enabled: None, //true,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
            drag_points: Default::default(),
        }
    }
}

impl BiffRead for Trigger {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut trigger = Trigger::default();
        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                // tag_str: SHAP
                // tag_str: ANSP
                // tag_str: REEN
                "VCEN" => {
                    trigger.center = Vertex2D::biff_read(reader)?;
                }
                "RADI" => {
                    trigger.radius = reader.get_f32()?;
                }
                "ROTA" => {
                    trigger.rotation = reader.get_f32()?;
                }
                "WITI" => {
                    trigger.wire_thickness = Some(reader.get_f32()?);
                }
                "SCAX" => {
                    trigger.scale_x = reader.get_f32()?;
                }
                "SCAY" => {
                    trigger.scale_y = reader.get_f32()?;
                }
                "MATR" => {
                    trigger.material = reader.get_string()?;
                }
                "SURF" => {
                    trigger.surface = reader.get_string()?;
                }
                "VSBL" => {
                    trigger.is_visible = reader.get_bool()?;
                }
                "EBLD" => {
                    trigger.is_enabled = reader.get_bool()?;
                }
                "THOT" => {
                    trigger.hit_height = reader.get_f32()?;
                }
                "NAME" => {
                    trigger.name = reader.get_wide_string()?;
                }
                "SHAP" => {
                    trigger.shape = reader.get_u32()?.into();
                }
                "ANSP" => {
                    trigger.anim_speed = reader.get_f32()?;
                }
                "REEN" => {
                    trigger.is_reflection_enabled = Some(reader.get_bool()?);
                }
                "DPNT" => {
                    let point = DragPoint::biff_read(reader);
                    trigger.drag_points.push(point?);
                }
                _ => {
                    if !trigger.timer.biff_read_tag(tag_str, reader)?
                        && !trigger.read_shared_attribute(tag_str, reader)?
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
        Ok(trigger)
    }
}

impl BiffWrite for Trigger {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("RADI", self.radius);
        writer.write_tagged_f32("ROTA", self.rotation);
        if let Some(wire_thickness) = self.wire_thickness {
            writer.write_tagged_f32("WITI", wire_thickness);
        }
        writer.write_tagged_f32("SCAX", self.scale_x);
        writer.write_tagged_f32("SCAY", self.scale_y);
        self.timer.biff_write(writer);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_bool("EBLD", self.is_enabled);
        writer.write_tagged_bool("VSBL", self.is_visible);
        writer.write_tagged_f32("THOT", self.hit_height);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("SHAP", (&self.shape).into());
        writer.write_tagged_f32("ANSP", self.anim_speed);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }

        self.write_shared_attributes(writer);

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
        fn any_trigger_round_trips_through_its_records(trigger in any::<Trigger>()) {
            let mut writer = BiffWriter::new();
            Trigger::biff_write(&trigger, &mut writer);
            let read = Trigger::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
            prop_assert_eq!(debug(&trigger), debug(&read));
        }
    }

    #[test]
    fn test_trigger_shape_json() {
        let sizing_type = TriggerShape::Inder;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"inder\"");
        let sizing_type_read: TriggerShape = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(2);
        let sizing_type_read: TriggerShape = serde_json::from_value(json).unwrap();
        assert_eq!(TriggerShape::Star, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `none`, `wire_a`, `star`, `wire_b`, `button`, `wire_c`, `wire_d`, `inder`\", line: 0, column: 0)"]
    fn test_trigger_shape_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: TriggerShape = serde_json::from_value(json).unwrap();
    }
}
