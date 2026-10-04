//! JSON text as vpinball writes it: nlohmann's `dump(2)` layout, which
//! matches serde_json's pretty printer, and nlohmann's float formatting,
//! which differs from serde_json's in the last digit of some values.
//!
//! The float formatting is a port of `nlohmann::detail::to_chars` (Grisu2,
//! nlohmann/json 3.12.0, MIT license, based on the reference
//! implementation by Florian Loitsch).

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter, Serializer};
use std::io;

/// The document as `dump(2)` text
pub(super) fn to_vec<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut serializer = Serializer::with_formatter(&mut out, NlohmannFormatter::default());
    value.serialize(&mut serializer)?;
    Ok(out)
}

#[derive(Default)]
struct NlohmannFormatter {
    pretty: PrettyFormatter<'static>,
}

impl Formatter for NlohmannFormatter {
    fn write_f32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        writer.write_all(format_f64(f64::from(value)).as_bytes())
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(format_f64(value).as_bytes())
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object_value(writer)
    }
}

/// A finite double as nlohmann prints it: the Grisu2 digits, fixed point
/// for decimal exponents in `[-4, 15)` with `.0` for whole numbers, else
/// `d.ddde+XX`. serde_json writes non-finite values as `null` before a
/// formatter sees them.
pub(super) fn format_f64(value: f64) -> String {
    let mut out = String::with_capacity(24);
    if value.is_sign_negative() {
        out.push('-');
    }
    let value = value.abs();
    if value == 0.0 {
        out.push_str("0.0");
        return out;
    }
    let (digits, decimal_exponent) = grisu2(value);
    format_buffer(&mut out, &digits, decimal_exponent, -4, 15);
    out
}

#[derive(Clone, Copy)]
struct DiyFp {
    f: u64,
    e: i32,
}

impl DiyFp {
    fn sub(x: DiyFp, y: DiyFp) -> DiyFp {
        DiyFp {
            f: x.f - y.f,
            e: x.e,
        }
    }

    /// The upper 64 bits of the product, rounded with ties up
    fn mul(x: DiyFp, y: DiyFp) -> DiyFp {
        let (u_lo, u_hi) = (x.f & 0xFFFF_FFFF, x.f >> 32);
        let (v_lo, v_hi) = (y.f & 0xFFFF_FFFF, y.f >> 32);
        let p0 = u_lo * v_lo;
        let p1 = u_lo * v_hi;
        let p2 = u_hi * v_lo;
        let p3 = u_hi * v_hi;
        let mut q = (p0 >> 32) + (p1 & 0xFFFF_FFFF) + (p2 & 0xFFFF_FFFF);
        q += 1 << 31;
        let h = p3 + (p2 >> 32) + (p1 >> 32) + (q >> 32);
        DiyFp {
            f: h,
            e: x.e + y.e + 64,
        }
    }

    fn normalize(mut x: DiyFp) -> DiyFp {
        while x.f >> 63 == 0 {
            x.f <<= 1;
            x.e -= 1;
        }
        x
    }

    fn normalize_to(x: DiyFp, target_exponent: i32) -> DiyFp {
        DiyFp {
            f: x.f << (x.e - target_exponent),
            e: target_exponent,
        }
    }
}

/// The normalized value and its rounding boundaries, for a positive finite double
fn compute_boundaries(value: f64) -> (DiyFp, DiyFp, DiyFp) {
    const PRECISION: i32 = 53;
    const BIAS: i32 = 1024 - 1 + (PRECISION - 1);
    const MIN_EXP: i32 = 1 - BIAS;
    const HIDDEN_BIT: u64 = 1 << (PRECISION - 1);
    let bits = value.to_bits();
    let biased_exponent = bits >> (PRECISION - 1);
    let fraction = bits & (HIDDEN_BIT - 1);
    let v = if biased_exponent == 0 {
        DiyFp {
            f: fraction,
            e: MIN_EXP,
        }
    } else {
        DiyFp {
            f: fraction + HIDDEN_BIT,
            e: biased_exponent as i32 - BIAS,
        }
    };
    let lower_boundary_is_closer = fraction == 0 && biased_exponent > 1;
    let m_plus = DiyFp {
        f: 2 * v.f + 1,
        e: v.e - 1,
    };
    let m_minus = if lower_boundary_is_closer {
        DiyFp {
            f: 4 * v.f - 1,
            e: v.e - 2,
        }
    } else {
        DiyFp {
            f: 2 * v.f - 1,
            e: v.e - 1,
        }
    };
    let w_plus = DiyFp::normalize(m_plus);
    let w_minus = DiyFp::normalize_to(m_minus, w_plus.e);
    (DiyFp::normalize(v), w_minus, w_plus)
}

const ALPHA: i32 = -60;

/// `(f, e, k)` with `f * 2^e ~= 10^k`
const CACHED_POWERS: [(u64, i32, i32); 79] = [
    (0xAB70FE17C79AC6CA, -1060, -300),
    (0xFF77B1FCBEBCDC4F, -1034, -292),
    (0xBE5691EF416BD60C, -1007, -284),
    (0x8DD01FAD907FFC3C, -980, -276),
    (0xD3515C2831559A83, -954, -268),
    (0x9D71AC8FADA6C9B5, -927, -260),
    (0xEA9C227723EE8BCB, -901, -252),
    (0xAECC49914078536D, -874, -244),
    (0x823C12795DB6CE57, -847, -236),
    (0xC21094364DFB5637, -821, -228),
    (0x9096EA6F3848984F, -794, -220),
    (0xD77485CB25823AC7, -768, -212),
    (0xA086CFCD97BF97F4, -741, -204),
    (0xEF340A98172AACE5, -715, -196),
    (0xB23867FB2A35B28E, -688, -188),
    (0x84C8D4DFD2C63F3B, -661, -180),
    (0xC5DD44271AD3CDBA, -635, -172),
    (0x936B9FCEBB25C996, -608, -164),
    (0xDBAC6C247D62A584, -582, -156),
    (0xA3AB66580D5FDAF6, -555, -148),
    (0xF3E2F893DEC3F126, -529, -140),
    (0xB5B5ADA8AAFF80B8, -502, -132),
    (0x87625F056C7C4A8B, -475, -124),
    (0xC9BCFF6034C13053, -449, -116),
    (0x964E858C91BA2655, -422, -108),
    (0xDFF9772470297EBD, -396, -100),
    (0xA6DFBD9FB8E5B88F, -369, -92),
    (0xF8A95FCF88747D94, -343, -84),
    (0xB94470938FA89BCF, -316, -76),
    (0x8A08F0F8BF0F156B, -289, -68),
    (0xCDB02555653131B6, -263, -60),
    (0x993FE2C6D07B7FAC, -236, -52),
    (0xE45C10C42A2B3B06, -210, -44),
    (0xAA242499697392D3, -183, -36),
    (0xFD87B5F28300CA0E, -157, -28),
    (0xBCE5086492111AEB, -130, -20),
    (0x8CBCCC096F5088CC, -103, -12),
    (0xD1B71758E219652C, -77, -4),
    (0x9C40000000000000, -50, 4),
    (0xE8D4A51000000000, -24, 12),
    (0xAD78EBC5AC620000, 3, 20),
    (0x813F3978F8940984, 30, 28),
    (0xC097CE7BC90715B3, 56, 36),
    (0x8F7E32CE7BEA5C70, 83, 44),
    (0xD5D238A4ABE98068, 109, 52),
    (0x9F4F2726179A2245, 136, 60),
    (0xED63A231D4C4FB27, 162, 68),
    (0xB0DE65388CC8ADA8, 189, 76),
    (0x83C7088E1AAB65DB, 216, 84),
    (0xC45D1DF942711D9A, 242, 92),
    (0x924D692CA61BE758, 269, 100),
    (0xDA01EE641A708DEA, 295, 108),
    (0xA26DA3999AEF774A, 322, 116),
    (0xF209787BB47D6B85, 348, 124),
    (0xB454E4A179DD1877, 375, 132),
    (0x865B86925B9BC5C2, 402, 140),
    (0xC83553C5C8965D3D, 428, 148),
    (0x952AB45CFA97A0B3, 455, 156),
    (0xDE469FBD99A05FE3, 481, 164),
    (0xA59BC234DB398C25, 508, 172),
    (0xF6C69A72A3989F5C, 534, 180),
    (0xB7DCBF5354E9BECE, 561, 188),
    (0x88FCF317F22241E2, 588, 196),
    (0xCC20CE9BD35C78A5, 614, 204),
    (0x98165AF37B2153DF, 641, 212),
    (0xE2A0B5DC971F303A, 667, 220),
    (0xA8D9D1535CE3B396, 694, 228),
    (0xFB9B7CD9A4A7443C, 720, 236),
    (0xBB764C4CA7A44410, 747, 244),
    (0x8BAB8EEFB6409C1A, 774, 252),
    (0xD01FEF10A657842C, 800, 260),
    (0x9B10A4E5E9913129, 827, 268),
    (0xE7109BFBA19C0C9D, 853, 276),
    (0xAC2820D9623BF429, 880, 284),
    (0x80444B5E7AA7CF85, 907, 292),
    (0xBF21E44003ACDD2D, 933, 300),
    (0x8E679C2F5E44FF8F, 960, 308),
    (0xD433179D9C8CB841, 986, 316),
    (0x9E19DB92B4E31BA9, 1013, 324),
];

fn cached_power_for_binary_exponent(e: i32) -> (u64, i32, i32) {
    const MIN_DEC_EXP: i32 = -300;
    const DEC_STEP: i32 = 8;
    let f = ALPHA - e - 1;
    let k = (f * 78913) / (1 << 18) + i32::from(f > 0);
    let index = (-MIN_DEC_EXP + k + (DEC_STEP - 1)) / DEC_STEP;
    CACHED_POWERS[index as usize]
}

/// `(k, 10^(k-1))` with `10^(k-1) <= n < 10^k`
fn find_largest_pow10(n: u32) -> (i32, u32) {
    let mut pow10 = 1_000_000_000;
    let mut k = 10;
    while k > 1 && n < pow10 {
        pow10 /= 10;
        k -= 1;
    }
    (k, pow10)
}

fn grisu2_round(buffer: &mut [u8], dist: u64, delta: u64, mut rest: u64, ten_k: u64) {
    let last = buffer.len() - 1;
    while rest < dist
        && delta - rest >= ten_k
        && (rest + ten_k < dist || dist - rest > rest + ten_k - dist)
    {
        buffer[last] -= 1;
        rest += ten_k;
    }
}

fn grisu2_digit_gen(
    buffer: &mut Vec<u8>,
    decimal_exponent: &mut i32,
    m_minus: DiyFp,
    w: DiyFp,
    m_plus: DiyFp,
) {
    let mut delta = DiyFp::sub(m_plus, m_minus).f;
    let mut dist = DiyFp::sub(m_plus, w).f;
    let one_e = m_plus.e;
    let one_f = 1u64 << -one_e;
    let mut p1 = (m_plus.f >> -one_e) as u32;
    let mut p2 = m_plus.f & (one_f - 1);
    let (k, mut pow10) = find_largest_pow10(p1);
    let mut n = k;
    while n > 0 {
        let d = p1 / pow10;
        let r = p1 % pow10;
        buffer.push(b'0' + d as u8);
        p1 = r;
        n -= 1;
        let rest = (u64::from(p1) << -one_e) + p2;
        if rest <= delta {
            *decimal_exponent += n;
            let ten_n = u64::from(pow10) << -one_e;
            grisu2_round(buffer, dist, delta, rest, ten_n);
            return;
        }
        pow10 /= 10;
    }
    let mut m = 0;
    loop {
        p2 *= 10;
        let d = p2 >> -one_e;
        let r = p2 & (one_f - 1);
        buffer.push(b'0' + d as u8);
        p2 = r;
        m += 1;
        delta *= 10;
        dist *= 10;
        if p2 <= delta {
            break;
        }
    }
    *decimal_exponent -= m;
    grisu2_round(buffer, dist, delta, p2, one_f);
}

/// The digits and decimal exponent of a positive finite double
fn grisu2(value: f64) -> (Vec<u8>, i32) {
    let (v, m_minus, m_plus) = compute_boundaries(value);
    let (cached_f, cached_e, cached_k) = cached_power_for_binary_exponent(m_plus.e);
    let c_minus_k = DiyFp {
        f: cached_f,
        e: cached_e,
    };
    let w = DiyFp::mul(v, c_minus_k);
    let w_minus = DiyFp::mul(m_minus, c_minus_k);
    let w_plus = DiyFp::mul(m_plus, c_minus_k);
    let m_minus = DiyFp {
        f: w_minus.f + 1,
        e: w_minus.e,
    };
    let m_plus = DiyFp {
        f: w_plus.f - 1,
        e: w_plus.e,
    };
    let mut digits = Vec::with_capacity(17);
    let mut decimal_exponent = -cached_k;
    grisu2_digit_gen(&mut digits, &mut decimal_exponent, m_minus, w, m_plus);
    (digits, decimal_exponent)
}

fn format_buffer(
    out: &mut String,
    digits: &[u8],
    decimal_exponent: i32,
    min_exp: i32,
    max_exp: i32,
) {
    let digits = std::str::from_utf8(digits).unwrap_or_default();
    let k = digits.len() as i32;
    let n = k + decimal_exponent;
    if k <= n && n <= max_exp {
        // digits followed by zeros: 1234e7 -> 12340000000.0
        out.push_str(digits);
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
        out.push_str(".0");
    } else if 0 < n && n <= max_exp {
        // dig.its: 1234e-2 -> 12.34
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if min_exp < n && n <= 0 {
        // 0.[000]digits: 1234e-6 -> 0.001234
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-n) as usize));
        out.push_str(digits);
    } else {
        // d.igitsE+-dd: 1234e-14 -> 1.234e-11
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        let exponent = n - 1;
        out.push(if exponent < 0 { '-' } else { '+' });
        let exponent = exponent.unsigned_abs();
        if exponent < 10 {
            out.push('0');
        }
        out.push_str(&exponent.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn floats_print_like_nlohmann() {
        // nlohmann's own expectations (tests/src/unit-conversions.cpp,
        // unit-to_chars.cpp) and values seen in vpinball packs
        for (value, expected) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (100.0, "100.0"),
            (0.5, "0.5"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1e15, "1e+15"),
            (1e16, "1e+16"),
            (123456789012345.0, "123456789012345.0"),
            (1234567890123456.0, "1.234567890123456e+15"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (5e-324, "5e-324"),
            (f64::from(0.3f32), "0.30000001192092896"),
            (f64::from(964.4859f32), "964.4859008789063"),
            (f64::from(1.4099996f32), "1.4099996089935303"),
            (f64::from(220.34254f32), "220.34254455566406"),
            (f64::from(-12.585114f32), "-12.585113525390625"),
            (f64::from(0.999f32), "0.9990000128746033"),
        ] {
            assert_eq!(format_f64(value), expected, "{value:e}");
        }
    }

    #[test]
    fn printed_floats_parse_back_to_the_same_value() {
        let mut bits = 0x9E37_79B9_7F4A_7C15_u64;
        for _ in 0..100_000 {
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            let value = f64::from_bits(bits);
            if !value.is_finite() {
                continue;
            }
            let text = format_f64(value);
            assert_eq!(text.parse::<f64>().ok(), Some(value), "{text}");
            let single = f64::from(f32::from_bits(bits as u32));
            if single.is_finite() {
                let text = format_f64(single);
                assert_eq!(text.parse::<f64>().ok(), Some(single), "{text}");
            }
        }
    }
}
