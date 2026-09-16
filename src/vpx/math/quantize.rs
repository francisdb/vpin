//! Quantization: storing a fractional value as a small whole number.
//!
//! Many table properties are fractions from 0 to 1 (an opacity, a color
//! channel, a reflection strength). Storing each as a full 32-bit float would
//! waste space, so the file format keeps them as small integers instead.
//! Quantizing maps the fraction onto an integer range, and dequantizing maps
//! it back.
//!
//! For example an 8-bit quantization spreads `0.0..=1.0` across `0..=255`:
//! `0.0` becomes `0`, `1.0` becomes `255`, and `0.5` becomes `128`. The
//! reverse divides by 255. The round trip is lossy, since only 256 distinct
//! values survive, but for a color channel that is imperceptible and saves
//! three quarters of the storage.
//!
//! The `*_percent` variants use a 0..=100 range (a stored percentage), and the
//! const-generic `quantize_unsigned::<BITS>` picks the range from the bit
//! width. `precise_divide` matches how VPinball computes the reverse division.
//!
//! Ported from VPinball's `math/math.h`:
//! <https://github.com/vpinball/vpinball/blob/034f9408539c/src/math/math.h>
//!
//! The `quantize_*` functions keep VPinball's `assert(x >= 0)` precondition as
//! a `debug_assert!`, matching the C++ `assert` which is a debug-only guard
//! against the undefined behavior of casting a negative float to an unsigned
//! integer. Rust's cast is defined to saturate, so release builds are safe.

/// Perform a precise floating-point division.
///
/// VPinball uses SSE intrinsics for this when available (`_mm_div_ss`),
/// otherwise falls back to regular division. We use f64 intermediate
/// precision to approximate the SSE behavior.
///
/// From VPinball `src/math/math.h`:
/// ```cpp
/// #ifdef ENABLE_SSE_OPTIMIZATIONS
/// __forceinline float precise_divide(const float a, const float b)
/// {
///     return _mm_cvtss_f32(_mm_div_ss(_mm_set_ss(a), _mm_set_ss(b)));
/// }
/// #else
/// #define precise_divide(a,b) ((a)/(b))
/// #endif
/// ```
///
/// TODO we might want to also implement an SSE version of this for x86 targets,
///   but for now the f64 approach should be sufficient.
#[inline(always)]
fn precise_divide(a: f32, b: f32) -> f32 {
    (a as f64 / b as f64) as f32
}

/// Turn a stored percentage back into a fraction: `min(i / 100, 1.0)`.
///
/// vpinball's `dequantizeUnsignedPercent`. Values above 100 clamp to 1.0.
/// Inverse of [`quantize_unsigned_percent`] for the 101 integers it can
/// produce; for an arbitrary fraction the round trip is lossy.
#[inline]
pub fn dequantize_unsigned_percent(i: u32) -> f32 {
    const N: f32 = 100.0;
    precise_divide(i as f32, N).min(1.0)
}

/// Store a fraction as a percentage: `min(trunc(x * 101), 100)`.
///
/// vpinball's `quantizeUnsignedPercent`. The multiplier is 101 rather than
/// 100 so that 1.0 lands on 100 after truncation instead of needing a
/// rounding step; the `min` keeps anything above 1.0 at 100. `x` must not
/// be negative: debug builds assert on it, release builds saturate it to 0.
#[inline]
pub fn quantize_unsigned_percent(x: f32) -> u32 {
    const N: f32 = 100.0;
    const NP1: f32 = 101.0;
    // vpinball guards this with a debug-only assert; the release cast
    // saturates a negative value to zero, matching its saturating min
    debug_assert!(x >= 0.0);
    (x * NP1).min(N) as u32
}

/// Store a fraction in `bits` bits, chosen at run time:
/// `min(trunc(x * 2^bits), 2^bits - 1)`.
///
/// Same formula as [`quantize_unsigned`] but with the width as a value, for
/// records that pack several fields of different widths into one byte, such
/// as the material `edge_alpha` (7 bits, shifted next to the opacity flag)
/// and `thickness` and `glossy_image_lerp` (8 bits). `bits` must be at most
/// 8 for the result to fit; `x` must not be negative (debug assert, release
/// builds saturate to 0). Only 2^bits distinct fractions survive the
/// round trip through [`dequantize_u8`].
#[inline]
pub fn quantize_u8(bits: u8, x: f32) -> u8 {
    let n = (1 << bits) - 1;
    let np1 = 1 << bits;
    debug_assert!(x >= 0.0);
    ((x * (np1 as f32)).min(n as f32)) as u8
}

/// Turn a `bits`-wide stored value back into a fraction:
/// `min(i / (2^bits - 1), 1.0)`.
///
/// Inverse of [`quantize_u8`] for the values it produces (that direction of
/// the round trip is exact); see there for which records use it. `bits`
/// must be at most 8 for `i` to hold the full range.
#[inline]
pub fn dequantize_u8(bits: u8, i: u8) -> f32 {
    let n = (1 << bits) - 1;
    precise_divide(i as f32, n as f32).min(1.0)
}

/// Turn a `BITS`-wide stored value back into a fraction:
/// `min(i / (2^BITS - 1), 1.0)`.
///
/// vpinball's `dequantizeUnsigned<bits>` from `src/math/math.h`. Values at
/// or above `2^BITS - 1` come back as 1.0. `BITS` must be below 32. Inverse
/// of [`quantize_unsigned`] for the integers it produces; that direction of
/// the round trip is exact (`i / (2^BITS - 1) * 2^BITS` truncates back to
/// `i`), while a fraction that did not come out of a dequantize does not
/// survive quantizing in general.
#[inline]
pub fn dequantize_unsigned<const BITS: u8>(i: u32) -> f32 {
    let n = (1u32 << BITS) - 1;
    precise_divide(i as f32, n as f32).min(1.0)
}

/// Store a fraction in `BITS` bits: `min(trunc(x * 2^BITS), 2^BITS - 1)`.
///
/// vpinball's `quantizeUnsigned<bits>` from `src/math/math.h`. The
/// multiplier is `2^BITS` rather than `2^BITS - 1` so that 1.0 lands on the
/// maximum after truncation (0.5 gives 128 for 8 bits, not 127); the `min`
/// keeps anything at or above 1.0 at the maximum. `BITS` must be below 32
/// and `x` must not be negative: debug builds assert on it, release builds
/// saturate it to 0. Only `2^BITS` distinct fractions survive the round trip
/// through [`dequantize_unsigned`].
///
/// Used with 8 bits for the `DILI` (disable lighting top) record of walls,
/// primitives and hit targets and for the playfield reflection strength.
#[inline]
pub fn quantize_unsigned<const BITS: u8>(x: f32) -> u32 {
    let n = (1u32 << BITS) - 1;
    let np1 = 1u32 << BITS;
    debug_assert!(x >= 0.0);
    (x * (np1 as f32)).min(n as f32) as u32
}

/// [`dequantize_unsigned`] fixed at 8 bits: `min(i / 255, 1.0)`, so 255 is
/// 1.0 and 0 is 0.0.
#[inline]
pub fn dequantize_unsigned_8(i: u8) -> f32 {
    dequantize_unsigned::<8>(i as u32)
}

/// [`quantize_unsigned`] fixed at 8 bits: `min(trunc(x * 256), 255)`, so
/// 1.0 is 255, 0.5 is 128 and 0.0 is 0. `x` must not be negative (debug
/// assert, release builds saturate to 0).
#[inline]
pub fn quantize_unsigned_8(x: f32) -> u8 {
    quantize_unsigned::<8>(x) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_u8_8() {
        assert_eq!(quantize_u8(8, 0.0), 0);
        assert_eq!(quantize_u8(8, 1.0), 255);
        assert_eq!(quantize_u8(8, 0.5), 128);
    }

    #[test]
    fn test_dequantize_u8_8() {
        assert_eq!(dequantize_u8(8, 0), 0.0);
        assert_eq!(dequantize_u8(8, 255), 1.0);
        assert_eq!(dequantize_u8(8, 128), 0.5019608);
    }

    #[test]
    fn test_dequantize_quantize_u8() {
        assert_eq!(quantize_u8(8, dequantize_u8(8, 0)), 0);
        assert_eq!(quantize_u8(8, dequantize_u8(8, 100)), 100);
        assert_eq!(quantize_u8(8, dequantize_u8(8, 50)), 50);
    }

    #[test]
    fn test_quantize_u8_7() {
        assert_eq!(quantize_u8(7, 0.0), 0);
        assert_eq!(quantize_u8(7, 1.0), 127);
        assert_eq!(quantize_u8(7, 0.5), 64);
    }

    #[test]
    fn test_dequantize_u8_7() {
        assert_eq!(dequantize_u8(7, 0), 0.0);
        assert_eq!(dequantize_u8(7, 127), 1.0);
        assert_eq!(dequantize_u8(7, 64), 0.503937);
    }

    #[test]
    fn test_quantize_unsigned_8() {
        assert_eq!(quantize_unsigned_8(0.0), 0);
        assert_eq!(quantize_unsigned_8(1.0), 255);
        assert_eq!(quantize_unsigned_8(0.5), 128);
    }

    #[test]
    fn test_dequantize_unsigned_8() {
        assert_eq!(dequantize_unsigned_8(0), 0.0);
        assert_eq!(dequantize_unsigned_8(255), 1.0);
        assert_eq!(dequantize_unsigned_8(128), 0.5019608);
    }

    #[test]
    fn test_dequantize_quantize_unsigned_8() {
        assert_eq!(quantize_unsigned_8(dequantize_unsigned_8(0)), 0);
        assert_eq!(quantize_unsigned_8(dequantize_unsigned_8(100)), 100);
        assert_eq!(quantize_unsigned_8(dequantize_unsigned_8(50)), 50);
    }

    #[test]
    fn test_quantize_dequantize_unsigned_8() {
        assert_eq!(dequantize_unsigned_8(quantize_unsigned_8(0.0)), 0.0);
        assert_eq!(dequantize_unsigned_8(quantize_unsigned_8(1.0)), 1.0);
        assert_eq!(dequantize_unsigned_8(quantize_unsigned_8(0.5)), 0.5019608);
    }
}
