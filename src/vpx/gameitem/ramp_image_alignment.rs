use log::warn;
use serde::{Deserialize, Serialize};

/// How the ramp image is mapped onto the ramp, stored in the `ALGN` tag.
///
/// Mirrors vpinball's `ImageAlignment` enum (`ImageAlignWorld`,
/// `ImageAlignTopLeft`, `ImageAlignCenter`) as used by ramps, where the
/// editor only offers the first two as "World" and "Wrap".
///
/// Values this library does not know are kept in [`RampImageAlignment::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum RampImageAlignment {
    /// `ImageAlignWorld`: the image is projected in table space.
    World,
    /// `ImageAlignTopLeft`: the image is wrapped along the ramp.
    Wrap,
    /// `ImageAlignCenter`, which the ramp editor does not offer.
    /// Found in "Andromeda (Game Plan 1985) v4.vpx". Kept under its historical
    /// name for compatibility with existing expanded tables.
    Unknown,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant:
    /// it would write the same bytes as the named variant and read back as
    /// it, breaking round-trip equality. The library itself never does
    /// (`From` normalizes known values to their named variants).
    Other(u32),
}
impl From<u32> for RampImageAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => RampImageAlignment::World,
            1 => RampImageAlignment::Wrap,
            2 => RampImageAlignment::Unknown,
            other => {
                warn!("Unknown RampImageAlignment value {other}, keeping it as is");
                RampImageAlignment::Other(other)
            }
        }
    }
}
impl From<&RampImageAlignment> for u32 {
    fn from(value: &RampImageAlignment) -> Self {
        match value {
            RampImageAlignment::World => 0,
            RampImageAlignment::Wrap => 1,
            RampImageAlignment::Unknown => 2,
            RampImageAlignment::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`RampImageAlignment::Other`]
impl Serialize for RampImageAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RampImageAlignment::World => serializer.serialize_str("world"),
            RampImageAlignment::Wrap => serializer.serialize_str("wrap"),
            RampImageAlignment::Unknown => serializer.serialize_str("unknown"),
            RampImageAlignment::Other(value) => serializer.serialize_u32(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for RampImageAlignment {
    fn deserialize<D>(deserializer: D) -> Result<RampImageAlignment, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RampImageAlignmentVisitor;
        impl serde::de::Visitor<'_> for RampImageAlignmentVisitor {
            type Value = RampImageAlignment;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a RampImageAlignment as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                let value = u32::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u32",
                    )
                })?;
                Ok(RampImageAlignment::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "world" => Ok(RampImageAlignment::World),
                    "wrap" => Ok(RampImageAlignment::Wrap),
                    "unknown" => Ok(RampImageAlignment::Unknown),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["world", "wrap", "unknown"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(RampImageAlignmentVisitor)
    }
}
#[cfg(test)]
mod ramp_image_alignment_open_enum_tests {
    /// The legacy raw value 2 must normalize to the named Unknown
    /// variant, never to Other(2): both write the same bytes, so two
    /// representations would break round-trip equality.
    #[test]
    fn legacy_unknown_value_normalizes() {
        assert_eq!(RampImageAlignment::from(2), RampImageAlignment::Unknown);
    }

    use super::RampImageAlignment;

    #[test]
    fn unknown_value_round_trips() {
        let value = RampImageAlignment::from(4_000_000_000);
        assert_eq!(value, RampImageAlignment::Other(4_000_000_000));
        assert_eq!(u32::from(&value), 4_000_000_000);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(4_000_000_000u32));
        let back: RampImageAlignment = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<RampImageAlignment>(serde_json::json!("no_such_variant"))
                .is_err()
        );
    }
}

#[cfg(test)]
mod test {
    use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;

    #[test]
    fn test_alignment_json() {
        let sizing_type = RampImageAlignment::Wrap;
        let json = serde_json::to_string(&sizing_type).unwrap();
        pretty_assertions::assert_eq!(json, "\"wrap\"");
        let sizing_type_read: RampImageAlignment = serde_json::from_str(&json).unwrap();
        pretty_assertions::assert_eq!(sizing_type, sizing_type_read);
        let json = serde_json::Value::from(0);
        let sizing_type_read: RampImageAlignment = serde_json::from_value(json).unwrap();
        pretty_assertions::assert_eq!(RampImageAlignment::World, sizing_type_read);
    }

    #[test]
    #[should_panic = "unknown variant `foo`, expected one of `world`, `wrap`, `unknown`"]
    fn test_alignment_json_fail() {
        let json = serde_json::Value::from("foo");
        let _: RampImageAlignment = serde_json::from_value(json).unwrap();
    }
}
