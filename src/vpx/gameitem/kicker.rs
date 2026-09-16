use super::vertex2d::Vertex2D;
use crate::vpx::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Visual style of a kicker, mirroring vpinball's `KickerType`.
///
/// Values this library does not know are kept in [`KickerType::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum KickerType {
    /// `KickerInvisible`: no mesh, only the hit area.
    Invisible,
    /// `KickerHole`: hole with a rim.
    Hole,
    /// `KickerCup`: cup shaped kicker.
    Cup,
    /// `KickerHoleSimple`: plain hole.
    HoleSimple,
    /// `KickerWilliams`: Williams style kicker.
    Williams,
    /// `KickerGottlieb`: Gottlieb style kicker.
    Gottlieb,
    /// `KickerCup2`: second cup shape.
    Cup2,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(#[cfg_attr(test, proptest(strategy = "7..=u32::MAX"))] u32),
}
impl From<u32> for KickerType {
    fn from(value: u32) -> Self {
        match value {
            0 => KickerType::Invisible,
            1 => KickerType::Hole,
            2 => KickerType::Cup,
            3 => KickerType::HoleSimple,
            4 => KickerType::Williams,
            5 => KickerType::Gottlieb,
            6 => KickerType::Cup2,
            other => {
                warn!("Unknown KickerType value {other}, keeping it as is");
                KickerType::Other(other)
            }
        }
    }
}
impl From<&KickerType> for u32 {
    fn from(value: &KickerType) -> Self {
        match value {
            KickerType::Invisible => 0,
            KickerType::Hole => 1,
            KickerType::Cup => 2,
            KickerType::HoleSimple => 3,
            KickerType::Williams => 4,
            KickerType::Gottlieb => 5,
            KickerType::Cup2 => 6,
            KickerType::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`KickerType::Other`]
impl Serialize for KickerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            KickerType::Invisible => serializer.serialize_str("invisible"),
            KickerType::Hole => serializer.serialize_str("hole"),
            KickerType::Cup => serializer.serialize_str("cup"),
            KickerType::HoleSimple => serializer.serialize_str("hole_simple"),
            KickerType::Williams => serializer.serialize_str("williams"),
            KickerType::Gottlieb => serializer.serialize_str("gottlieb"),
            KickerType::Cup2 => serializer.serialize_str("cup2"),
            KickerType::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for KickerType {
    fn deserialize<D>(deserializer: D) -> Result<KickerType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KickerTypeVisitor;
        impl serde::de::Visitor<'_> for KickerTypeVisitor {
            type Value = KickerType;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a KickerType as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<KickerType, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(KickerType::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<KickerType, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "invisible" => Ok(KickerType::Invisible),
                    "hole" => Ok(KickerType::Hole),
                    "cup" => Ok(KickerType::Cup),
                    "hole_simple" => Ok(KickerType::HoleSimple),
                    "williams" => Ok(KickerType::Williams),
                    "gottlieb" => Ok(KickerType::Gottlieb),
                    "cup2" => Ok(KickerType::Cup2),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &[
                            "invisible",
                            "hole",
                            "cup",
                            "hole_simple",
                            "williams",
                            "gottlieb",
                            "cup2",
                        ],
                    )),
                }
            }
        }
        deserializer.deserialize_any(KickerTypeVisitor)
    }
}
#[cfg(test)]
mod kicker_type_open_enum_tests {
    use super::KickerType;

    #[test]
    fn unknown_value_round_trips() {
        let value = KickerType::from(4_000_000_000);
        assert_eq!(value, KickerType::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: KickerType = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<KickerType>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

/// A kicker: a hole or cup in the playfield that captures the ball and
/// ejects it under script control (`Kick`, `KickZ`, `KickXYZ`).
///
/// The hit area is a cylinder of [`radius`](Self::radius) at the height of
/// [`surface`](Self::surface), [`hit_height`](Self::hit_height) VPU high.
/// A ball entering it fires `_Hit`, is locked in the kicker (unless
/// [`fall_through`](Self::fall_through) is set) and stays there until the
/// script kicks or destroys it; leaving the area fires `_Unhit`. The visible
/// part is one of the built-in meshes selected by
/// [`kicker_type`](Self::kicker_type), scaled by the radius and rotated by
/// [`orientation`](Self::orientation).
///
/// The record is written by `Kicker::Save` and read by `Kicker::Load` in
/// vpinball's `src/parts/kicker.cpp`; the physics is `KickerHitCircle` in
/// the same file.
#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Kicker {
    /// Position of the kicker in table coordinates (VPU); both the hit
    /// circle and the mesh are centered here.
    ///
    /// BIFF tag `VCEN`
    pub center: Vertex2D,
    /// Radius of the kicker in VPU, used for the hit circle and to scale the
    /// mesh.
    ///
    /// In [`legacy_mode`](Self::legacy_mode) the hit circle is reduced to
    /// 60% of the radius (75% with [`fall_through`](Self::fall_through)) so
    /// only the inner part of the kicker starts a hit; otherwise the full
    /// radius is used. Default: `25.0`.
    ///
    /// BIFF tag `RADI`
    pub radius: f32,
    /// Name of the table material used to render the kicker mesh. Empty
    /// selects the default material.
    ///
    /// BIFF tag `MATR`
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub material: String,
    /// Name of the surface (ramp or wall top) this kicker sits on.
    /// Used to determine the kicker's base height (z position).
    /// If empty, the kicker sits on the playfield.
    /// BIFF tag: SURF
    #[cfg_attr(test, proptest(strategy = "crate::vpx::test_support::latin1_string()"))]
    pub surface: String,
    /// Whether the kicker's collider is active at table start (`Enabled`
    /// in script). A disabled kicker neither captures the ball nor fires
    /// hit events. Default: `true`.
    ///
    /// BIFF tag `EBLD`
    pub is_enabled: bool,
    /// Name of the kicker, its identifier in the editor and in scripts.
    /// Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Mesh used for the visible kicker, see [`KickerType`]. vpinball reads
    /// values above `KickerCup2` as `KickerInvisible`. Default:
    /// [`KickerType::Hole`].
    ///
    /// BIFF tag `TYPE`
    pub kicker_type: KickerType,
    /// Random deviation of the kick direction, in degrees.
    ///
    /// On `Kick` the yaw angle is offset by a random amount within this
    /// angle (a shaped, non uniform distribution), scaled by the table's
    /// global difficulty. A negative value selects the table-wide hard
    /// scatter constant; near-zero angles are ignored. Default: `0.0`.
    ///
    /// BIFF tag `KSCT`
    pub scatter: f32,
    /// Fraction of the capture height below which a ball is grabbed, in
    /// the range `0.0` to `1.0` (the script property clamps it).
    ///
    /// Outside [`legacy_mode`](Self::legacy_mode), a ball entering the hit
    /// cylinder is only captured when its center is below
    /// `(hit area bottom + ball radius) x hit_accuracy`; a faster or higher
    /// ball is deflected by the kicker rim instead. Legacy mode always
    /// captures. Default: `0.5`.
    ///
    /// BIFF tag `KHAC`
    pub hit_accuracy: f32,
    /// Height of the hit cylinder above its base, in VPU. Default: `35.0`.
    ///
    /// `None` when the record is absent (10.01 files); vpinball then uses
    /// the default.
    ///
    /// BIFF tag `KHHI` (was missing in 10.01)
    pub hit_height: Option<f32>,
    /// Rotation of the mesh around the Z axis, in degrees. The Williams and
    /// Gottlieb meshes get an extra 90 degrees. Has no effect on the hit
    /// area. Default: `0.0`.
    ///
    /// BIFF tag `KORI`
    pub orientation: f32,
    /// When `true` a captured ball falls through the kicker instead of
    /// being held: it is moved 5 VPU below the hit area and is not locked,
    /// which is used for drains and subways feeding another kicker. In
    /// [`legacy_mode`](Self::legacy_mode) it also widens the hit circle to
    /// 75% of the radius. Default: `false`.
    ///
    /// BIFF tag `FATH`
    pub fall_through: bool,
    /// Selects the older kicker physics: a plain, reduced hit circle that
    /// captures every ball entering it.
    ///
    /// When `false` the kicker uses the hit mesh: the ball rolls over the
    /// rim and is only captured when it is low and slow enough (see
    /// [`hit_accuracy`](Self::hit_accuracy)), giving realistic bounce-outs.
    /// Default: `true`.
    ///
    /// BIFF tag `LEMO`
    pub legacy_mode: bool,

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
}
impl_shared_attributes!(Kicker);

#[derive(Serialize, Deserialize)]
struct KickerJson {
    center: Vertex2D,
    radius: f32,
    #[serde(flatten)]
    pub timer: TimerData,
    material: String,
    surface: String,
    is_enabled: bool,
    name: String,
    kicker_type: KickerType,
    scatter: f32,
    hit_accuracy: f32,
    hit_height: Option<f32>,
    orientation: f32,
    fall_through: bool,
    legacy_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl KickerJson {
    fn from_kicker(kicker: &Kicker) -> Self {
        Self {
            center: kicker.center,
            radius: kicker.radius,
            timer: kicker.timer.clone(),
            material: kicker.material.clone(),
            surface: kicker.surface.clone(),
            is_enabled: kicker.is_enabled,
            name: kicker.name.clone(),
            kicker_type: kicker.kicker_type.clone(),
            scatter: kicker.scatter,
            hit_accuracy: kicker.hit_accuracy,
            hit_height: kicker.hit_height,
            orientation: kicker.orientation,
            fall_through: kicker.fall_through,
            legacy_mode: kicker.legacy_mode,
            part_group_name: kicker.part_group_name.clone(),
        }
    }

    fn into_kicker(self) -> Kicker {
        Kicker {
            center: self.center,
            radius: self.radius,
            timer: self.timer.clone(),
            material: self.material,
            surface: self.surface,
            is_enabled: self.is_enabled,
            name: self.name,
            kicker_type: self.kicker_type,
            scatter: self.scatter,
            hit_accuracy: self.hit_accuracy,
            hit_height: self.hit_height,
            orientation: self.orientation,
            fall_through: self.fall_through,
            legacy_mode: self.legacy_mode,
            part_group_name: self.part_group_name,
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

impl Serialize for Kicker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        KickerJson::from_kicker(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Kicker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kicker_json = KickerJson::deserialize(deserializer)?;
        Ok(kicker_json.into_kicker())
    }
}

impl Default for Kicker {
    fn default() -> Self {
        Self {
            center: Default::default(),
            radius: 25.0,
            timer: TimerData::default(),
            material: Default::default(),
            surface: Default::default(),
            is_enabled: true,
            name: Default::default(),
            kicker_type: KickerType::Hole,
            scatter: 0.0,
            hit_accuracy: 0.7,
            hit_height: None, //40.0,
            orientation: 0.0,
            fall_through: false,
            legacy_mode: true,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl BiffRead for Kicker {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut kicker = Kicker::default();

        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    kicker.center = Vertex2D::biff_read(reader)?;
                }
                "RADI" => {
                    kicker.radius = reader.get_f32()?;
                }
                "MATR" => {
                    kicker.material = reader.get_string()?;
                }
                "SURF" => {
                    kicker.surface = reader.get_string()?;
                }
                "EBLD" => {
                    kicker.is_enabled = reader.get_bool()?;
                }
                "NAME" => {
                    kicker.name = reader.get_wide_string()?;
                }
                "TYPE" => {
                    kicker.kicker_type = reader.get_u32()?.into();
                }
                "KSCT" => {
                    kicker.scatter = reader.get_f32()?;
                }
                "KHAC" => {
                    kicker.hit_accuracy = reader.get_f32()?;
                }
                "KHHI" => {
                    kicker.hit_height = Some(reader.get_f32()?);
                }
                "KORI" => {
                    kicker.orientation = reader.get_f32()?;
                }
                "FATH" => {
                    kicker.fall_through = reader.get_bool()?;
                }
                "LEMO" => {
                    kicker.legacy_mode = reader.get_bool()?;
                }
                _ => {
                    if !kicker.timer.biff_read_tag(tag_str, reader)?
                        && !kicker.read_shared_attribute(tag_str, reader)?
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
        Ok(kicker)
    }
}

impl BiffWrite for Kicker {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("RADI", self.radius);
        self.timer.biff_write(writer);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_bool("EBLD", self.is_enabled);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_u32("TYPE", (&self.kicker_type).into());
        writer.write_tagged_f32("KSCT", self.scatter);
        writer.write_tagged_f32("KHAC", self.hit_accuracy);
        if let Some(hit_height) = self.hit_height {
            writer.write_tagged_f32("KHHI", hit_height);
        }
        writer.write_tagged_f32("KORI", self.orientation);
        writer.write_tagged_bool("FATH", self.fall_through);
        writer.write_tagged_bool("LEMO", self.legacy_mode);

        self.write_shared_attributes(writer);

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
        fn any_kicker_round_trips_through_its_records(kicker in any::<Kicker>()) {
            let mut writer = BiffWriter::new();
            Kicker::biff_write(&kicker, &mut writer);
            let read = Kicker::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
            prop_assert_eq!(debug(&kicker), debug(&read));
        }
    }

    #[test]
    fn test_kicker_type_json() {
        let sizing_type = KickerType::Cup;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"cup\"");
        let sizing_type_read: KickerType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(1);
        let sizing_type_read: KickerType = serde_json::from_value(json).unwrap();
        assert_eq!(KickerType::Hole, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected one of `invisible`, `hole`, `cup`, `hole_simple`, `williams`, `gottlieb`, `cup2`\", line: 0, column: 0)"]
    fn test_kicker_type_json_fail_string() {
        let json = serde_json::Value::from("foo");
        let _: KickerType = serde_json::from_value(json).unwrap();
    }
}
