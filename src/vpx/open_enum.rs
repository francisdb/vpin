//! The `open_enum!` macro behind the crate's value enums.
//!
//! Every value enum read from a vpx file follows the same open pattern:
//! named variants for the values this library knows, a catch-all variant
//! holding the raw number for the ones it does not (kept as is with a
//! warning so the table round-trips unchanged), `From` conversions in
//! both directions, string-or-number serde, and normalization of known
//! values to their named variants on every read path.
//!
//! This macro is that pattern's single definition. The per-enum unit
//! tests are deliberately hand-written next to each invocation: they
//! were authored against the previous hand-rolled impls and verify the
//! macro emits identical behavior.
//!
//! Enums without a serde surface (the flasher display-style family:
//! `DmdStyle`, `DisplayStyle`, `SegStyle`, `SegFamily`) stay
//! hand-written - the macro always emits serde impls, and growing a
//! serde surface would be a behavior change, not a refactor.

/// Define an open value enum together with its conversions and serde.
///
/// ```ignore
/// open_enum! {
///     /// What a decal shows, mirroring vpinball's `DecalType`.
///     #[derive(Debug, PartialEq, Clone)]
///     #[cfg_attr(test, derive(fake::Dummy))]
///     pub enum DecalType(u32) {
///         /// `DecalText`: renders the decal text with its font.
///         Text = 0 => "text",
///         /// `DecalImage`: renders the decal image.
///         Image = 1 => "image",
///         ;
///         /// A value not known to this library, kept as is.
///         Other,
///     }
/// }
/// ```
///
/// The repr (`u32`, `u8` or `i32`) is the wire type; each named variant
/// maps a raw value to a serde string; the section after `;` names the
/// catch-all variant (usually `Other`, `Unknown` where history named it
/// so) with its docs and optional field attributes.
///
/// Generated: the enum itself, `From<repr>` (normalizing known values to
/// named variants and warning on unknown ones), `From<&Self> for repr`,
/// `Serialize` (strings for named variants, the raw number for the
/// catch-all) and `Deserialize` (accepting both forms; numbers resolve
/// through `From`, so a value this library later learns deserializes to
/// its named variant).
macro_rules! open_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $name:ident($repr:tt) {
            $(
                $(#[$var_meta:meta])*
                $variant:ident = $value:literal => $str:literal,
            )+
            ;
            $(#[$other_meta:meta])*
            $other:ident $(( $(#[$other_field_meta:meta])* ))?,
        }
    ) => {
        $(#[$enum_meta])*
        $vis enum $name {
            $(
                $(#[$var_meta])*
                $variant,
            )+
            $(#[$other_meta])*
            ///
            /// Must not be constructed with a value that maps to a named
            /// variant: it would write the same bytes as the named variant
            /// and read back as it, breaking round-trip equality. The
            /// library itself never does (`From` normalizes known values
            /// to their named variants).
            $other($($( #[$other_field_meta] )*)? $repr),
        }

        impl From<$repr> for $name {
            fn from(value: $repr) -> Self {
                match value {
                    $( $value => $name::$variant, )+
                    other => {
                        log::warn!(
                            "Unknown {} value {other}, keeping it as is",
                            stringify!($name)
                        );
                        $name::$other(other)
                    }
                }
            }
        }

        impl From<&$name> for $repr {
            fn from(value: &$name) -> Self {
                match value {
                    $( $name::$variant => $value, )+
                    $name::$other(value) => *value,
                }
            }
        }

        #[doc = concat!(
            "Serialize to lowercase string, or the raw number for [`",
            stringify!($name), "::", stringify!($other), "`]"
        )]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                match self {
                    $( $name::$variant => serializer.serialize_str($str), )+
                    $name::$other(value) =>
                        crate::vpx::open_enum::open_enum!(@serialize_num $repr, serializer, value),
                }
            }
        }

        /// Deserialize from lowercase string, or from the raw number
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct OpenEnumVisitor;
                impl serde::de::Visitor<'_> for OpenEnumVisitor {
                    type Value = $name;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(concat!(
                            "a ", stringify!($name), " as lowercase string or number"
                        ))
                    }
                    fn visit_u64<E>(self, value: u64) -> Result<$name, E>
                    where
                        E: serde::de::Error,
                    {
                        let value = <$repr>::try_from(value).map_err(|_| {
                            serde::de::Error::invalid_value(
                                serde::de::Unexpected::Unsigned(value),
                                &concat!("a number that fits in ", stringify!($repr)),
                            )
                        })?;
                        Ok($name::from(value))
                    }
                    fn visit_i64<E>(self, value: i64) -> Result<$name, E>
                    where
                        E: serde::de::Error,
                    {
                        let value = <$repr>::try_from(value).map_err(|_| {
                            serde::de::Error::invalid_value(
                                serde::de::Unexpected::Signed(value),
                                &concat!("a number that fits in ", stringify!($repr)),
                            )
                        })?;
                        Ok($name::from(value))
                    }
                    fn visit_str<E>(self, value: &str) -> Result<$name, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            $( $str => Ok($name::$variant), )+
                            _ => Err(serde::de::Error::unknown_variant(
                                value,
                                &[$($str),+],
                            )),
                        }
                    }
                }
                deserializer.deserialize_any(OpenEnumVisitor)
            }
        }
    };

    (@serialize_num u32, $serializer:expr, $value:expr) => {
        $serializer.serialize_u32(*$value)
    };
    (@serialize_num u8, $serializer:expr, $value:expr) => {
        $serializer.serialize_u8(*$value)
    };
    (@serialize_num i32, $serializer:expr, $value:expr) => {
        $serializer.serialize_i32(*$value)
    };
}

pub(crate) use open_enum;
