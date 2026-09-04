crate::vpx::open_enum::open_enum! {
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
    pub enum RampImageAlignment(u32) {
        /// `ImageAlignWorld`: the image is projected in table space.
        World = 0 => "world",
        /// `ImageAlignTopLeft`: the image is wrapped along the ramp.
        Wrap = 1 => "wrap",
        /// `ImageAlignCenter`, which the ramp editor does not offer.
        /// Found in "Andromeda (Game Plan 1985) v4.vpx". Kept under its historical
        /// name for compatibility with existing expanded tables.
        Unknown = 2 => "unknown",
        ;
        /// A value not known to this library, kept as is.
        Other,
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
