use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Clone, Dummy)]
pub enum RampImageAlignment {
    World = 0,
    Wrap = 1,
    /// non-official, found in Andromeda (Game Plan 1985) v4.vpx
    /// This is not in the official VPX documentation
    Unknown = 2,
}

impl From<u32> for RampImageAlignment {
    fn from(value: u32) -> Self {
        match value {
            0 => RampImageAlignment::World,
            1 => RampImageAlignment::Wrap,
            2 => RampImageAlignment::Unknown,
            _ => panic!("Invalid RampImageAlignment {value}"),
        }
    }
}

impl From<&RampImageAlignment> for u32 {
    fn from(value: &RampImageAlignment) -> Self {
        match value {
            RampImageAlignment::World => 0,
            RampImageAlignment::Wrap => 1,
            RampImageAlignment::Unknown => 2,
        }
    }
}

/// Serializes RampImageAlignment to lowercase string
impl Serialize for RampImageAlignment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            RampImageAlignment::World => serializer.serialize_str("world"),
            RampImageAlignment::Wrap => serializer.serialize_str("wrap"),
            RampImageAlignment::Unknown => serializer.serialize_str("unknown"),
        }
    }
}

/// Deserializes RampImageAlignment from lowercase string
/// or number for backwards compatibility
impl<'de> Deserialize<'de> for RampImageAlignment {
    fn deserialize<D>(deserializer: D) -> Result<RampImageAlignment, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RampImageAlignmentVisitor;

        impl serde::de::Visitor<'_> for RampImageAlignmentVisitor {
            type Value = RampImageAlignment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or number representing a TargetType")
            }

            fn visit_u64<E>(self, value: u64) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(RampImageAlignment::World),
                    1 => Ok(RampImageAlignment::Wrap),
                    2 => Ok(RampImageAlignment::Unknown),
                    _ => Err(serde::de::Error::unknown_variant(
                        &value.to_string(),
                        &["0", "1"],
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<RampImageAlignment, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "world" => Ok(RampImageAlignment::World),
                    "wrap" => Ok(RampImageAlignment::Wrap),
                    "unknown" => Ok(RampImageAlignment::Unknown),
                    _ => Err(serde::de::Error::unknown_variant(value, &["world", "wrap"])),
                }
            }
        }

        deserializer.deserialize_any(RampImageAlignmentVisitor)
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
    #[should_panic = "Error(\"unknown variant `foo`, expected `world` or `wrap`\", line: 0, column: 0)"]
    fn test_alignment_json_fail() {
        let json = serde_json::Value::from("foo");
        let _: RampImageAlignment = serde_json::from_value(json).unwrap();
    }
}
