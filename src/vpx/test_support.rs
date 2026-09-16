//! Strategies and helpers shared by the property based round trip tests.

use proptest::prelude::*;
use std::fmt::Debug;

/// A string the Latin-1 records can carry unchanged: printable ASCII and
/// the Latin-1 supplement, at most 32 characters. Anything else is written
/// as `?` and would not round trip.
pub(crate) fn latin1_string() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[ -~\u{A0}-\u{FF}]{0,32}").unwrap()
}

/// A fraction that survives the 8 bit quantization of the legacy records:
/// one of the 256 values the dequantizer produces.
pub(crate) fn quantized_u8() -> impl Strategy<Value = f32> {
    (0..=255u32).prop_map(crate::vpx::math::dequantize_unsigned::<8>)
}

/// The debug rendering of a value, which is what the round trip tests
/// compare: unlike `PartialEq` it treats a NaN as equal to itself, so the
/// floats can take every value a file can hold.
pub(crate) fn debug<T: Debug>(value: &T) -> String {
    format!("{value:#?}")
}
