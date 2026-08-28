//! Unit conversion utilities for VPinball units
//!
//! VPinball uses "VP Units" (VPU) as its internal coordinate system.
//! This module provides conversion functions between VPU and real-world units.
//!
//! ## VP Units (VPU)
//!
//! From VPinball's `def.h`:
//! - 50 VPU = 1.0625 inches (the diameter of a standard pinball)
//! - 1 inch = 25.4 mm
//!
//! Therefore:
//! - 1 VPU = (25.4 * 1.0625) / 50 mm = 0.539750 mm
//! - 1 VPU = 0.000539750 meters
//! - 1 VPU ≈ 0.054 cm
//!
//! ## Conversion Reference
//!
//! | From | To | Multiply by |
//! |------|----|-------------|
//! | VPU | mm | 0.539750 |
//! | VPU | cm | 0.0539750 |
//! | VPU | m | 0.000539750 |
//! | mm | VPU | 1.8527 |
//! | cm | VPU | 18.527 |
//! | m | VPU | 1852.7 |

/// Conversion factor: 1 VPU in millimeters
/// 50 VPU = 1.0625 inches, 1 inch = 25.4mm
/// So 1 VPU = (25.4 * 1.0625) / 50 mm = 0.539750 mm
const VPU_TO_MM: f32 = (25.4 * 1.0625) / 50.0;

/// Conversion factor: 1 VPU in centimeters
const VPU_TO_CM: f32 = VPU_TO_MM / 10.0;

/// Conversion factor: 1 VPU in meters
const VPU_TO_M: f32 = VPU_TO_MM / 1000.0;

/// Convert VP Units to millimeters
#[inline]
pub fn vpu_to_mm(vpu: f32) -> f32 {
    vpu * VPU_TO_MM
}

/// Convert VP Units to centimeters
#[inline]
pub fn vpu_to_cm(vpu: f32) -> f32 {
    vpu * VPU_TO_CM
}

/// Convert VP Units to meters
#[inline]
pub fn vpu_to_m(vpu: f32) -> f32 {
    vpu * VPU_TO_M
}

/// Convert millimeters to VP Units
#[inline]
pub fn mm_to_vpu(mm: f32) -> f32 {
    mm / VPU_TO_MM
}

/// Convert centimeters to VP Units
#[inline]
pub fn cm_to_vpu(cm: f32) -> f32 {
    cm / VPU_TO_CM
}

/// Convert meters to VP Units
#[inline]
pub fn m_to_vpu(m: f32) -> f32 {
    m / VPU_TO_M
}

/// Output unit for table exports (OBJ, glTF/GLB).
///
/// VPinball internally uses "VP Units" (VPU): 50 VPU = 1.0625 inches
/// (one pinball diameter), so a standard table around 20 inches wide
/// is roughly 950 VPU. Loaded straight into a tool that treats
/// numbers as metres, that becomes a ~950 m table. Pick a metric
/// variant to scale positions on the way out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportUnits {
    /// Raw VPinball Units (no scaling).
    #[default]
    Vpu,
    /// Millimetres.
    Mm,
    /// Centimetres.
    Cm,
    /// Metres.
    M,
}

impl ExportUnits {
    /// Multiplier to apply to a VPU position to obtain the value in
    /// this unit.
    #[inline]
    pub fn scale(self) -> f32 {
        match self {
            ExportUnits::Vpu => 1.0,
            ExportUnits::Mm => VPU_TO_MM,
            ExportUnits::Cm => VPU_TO_CM,
            ExportUnits::M => VPU_TO_M,
        }
    }
}

/// Convert a VPU value to the requested export unit.
#[inline]
pub fn vpu_to_units(vpu: f32, units: ExportUnits) -> f32 {
    vpu * units.scale()
}

/// Axis convention for 3D mesh interchange (OBJ, glTF/GLB), in either
/// direction: exporting VPX geometry or importing meshes back in.
///
/// VPX space is left-handed with Z up: X right (across the playfield),
/// Y towards the player (down the playfield), Z up (towards the glass).
/// [`Self::ZUpLeftHanded`] is that internal frame verbatim; the other
/// variants negate or swap exactly one axis pair, so each produces a
/// right-handed frame from the left-handed one. A conversion between two
/// frames of different [`Self::handedness`] must pair with a reversed
/// triangle winding (matching vpinball's `ObjLoader`); one between frames
/// of equal handedness keeps the winding, since such frames differ only
/// by a pure rotation (the two right-handed variants: 90 degrees about
/// X).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AxisConvention {
    /// VPX's internal coordinate system: left-handed with Z up, inherited
    /// from DirectX. X runs right across the playfield, Y down the
    /// playfield towards the player, Z up towards the glass.
    ///
    /// Converting to or from this convention leaves coordinates untouched
    /// (vertices keep their vpx-internal values). Only meaningful for
    /// tooling that works on the raw vpx data; not what any viewer or DCC
    /// tool expects.
    ZUpLeftHanded,
    /// VPinball's exported OBJ convention: `(x, y, -z)`.
    ///
    /// Right-handed like [`Self::YUpRightHanded`], but keeping VPX's axis
    /// roles: Y still runs down the playfield and Z, now negated, points
    /// down through the table instead of up. This is what vpinball itself
    /// writes when exporting an OBJ and expects when importing one, so a
    /// table shows up rotated 90 degrees about X in a Y-up viewer.
    #[default]
    ZDownRightHanded,
    /// Y up, right-handed: `(x, z, y)`. The glTF and Wavefront OBJ
    /// convention, and what Blender / Maya / 3ds Max assume with their
    /// default import settings (`Scale 1.0, Forward -Z, Up Y`).
    YUpRightHanded,
}

impl AxisConvention {
    /// Map a position or normal from VPX axes to this convention.
    ///
    /// Positions should already be scaled to the target unit; normals use
    /// the same mapping (axis maps are orthogonal, so normals transform
    /// like positions) but must never get the unit scale applied.
    #[inline]
    pub fn from_vpx(self, x: f32, y: f32, z: f32) -> [f32; 3] {
        match self {
            AxisConvention::ZUpLeftHanded => [x, y, z],
            AxisConvention::ZDownRightHanded => [x, y, -z],
            AxisConvention::YUpRightHanded => [x, z, y],
        }
    }

    /// Inverse of [`Self::from_vpx`], mapping back to VPX axes.
    /// Every variant is an involution (each map is its own inverse), so
    /// this intentionally equals `from_vpx`; see the round-trip test.
    #[inline]
    pub fn to_vpx(self, x: f32, y: f32, z: f32) -> [f32; 3] {
        self.from_vpx(x, y, z)
    }

    /// The handedness of this frame (also spelled out in the variant
    /// names).
    #[inline]
    pub fn handedness(self) -> Handedness {
        match self {
            AxisConvention::ZUpLeftHanded => Handedness::Left,
            AxisConvention::ZDownRightHanded | AxisConvention::YUpRightHanded => Handedness::Right,
        }
    }

    /// Whether converting between this convention and `other` flips
    /// handedness. When true, the conversion must pair with a reversed
    /// triangle winding to keep front faces front; when false the winding
    /// stays untouched.
    #[inline]
    pub fn flips_handedness(self, other: AxisConvention) -> bool {
        self.handedness() != other.handedness()
    }
}

/// Handedness of a 3D coordinate frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handedness {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpu_to_mm() {
        // 50 VPU should be 1.0625 inches = 26.9875 mm
        let mm = vpu_to_mm(50.0);
        assert!(
            (mm - 26.9875).abs() < 0.001,
            "50 VPU should be ~26.99 mm, got {}",
            mm
        );
    }

    #[test]
    fn test_vpu_to_m() {
        // 1000 VPU should be about 0.54 meters
        let m = vpu_to_m(1000.0);
        assert!(
            (m - 0.53975).abs() < 0.001,
            "1000 VPU should be ~0.54 m, got {}",
            m
        );
    }

    #[test]
    fn test_round_trip_mm() {
        let original = 100.0;
        let converted = mm_to_vpu(vpu_to_mm(original));
        assert!(
            (converted - original).abs() < 0.001,
            "Round trip failed: {} -> {}",
            original,
            converted
        );
    }

    #[test]
    fn test_round_trip_cm() {
        let original = 100.0;
        let converted = cm_to_vpu(vpu_to_cm(original));
        assert!(
            (converted - original).abs() < 0.001,
            "Round trip failed: {} -> {}",
            original,
            converted
        );
    }

    #[test]
    fn test_round_trip_m() {
        let original = 100.0;
        let converted = m_to_vpu(vpu_to_m(original));
        assert!(
            (converted - original).abs() < 0.001,
            "Round trip failed: {} -> {}",
            original,
            converted
        );
    }

    #[test]
    fn test_export_axes_maps() {
        assert_eq!(
            AxisConvention::ZUpLeftHanded.from_vpx(1.0, 2.0, 3.0),
            [1.0, 2.0, 3.0]
        );
        assert_eq!(
            AxisConvention::ZDownRightHanded.from_vpx(1.0, 2.0, 3.0),
            [1.0, 2.0, -3.0]
        );
        assert_eq!(
            AxisConvention::YUpRightHanded.from_vpx(1.0, 2.0, 3.0),
            [1.0, 3.0, 2.0]
        );
    }

    /// All axis maps are involutions, which is why `to_vpx` equals
    /// `vertex`. This test exists so nobody "fixes" that later.
    #[test]
    fn test_export_axes_are_involutions() {
        for axes in [
            AxisConvention::ZUpLeftHanded,
            AxisConvention::ZDownRightHanded,
            AxisConvention::YUpRightHanded,
        ] {
            let [x, y, z] = axes.from_vpx(1.0, 2.0, 3.0);
            assert_eq!(
                axes.to_vpx(x, y, z),
                [1.0, 2.0, 3.0],
                "{axes:?} should round trip"
            );
        }
    }

    #[test]
    fn test_axis_convention_handedness() {
        use AxisConvention::*;
        assert_eq!(ZUpLeftHanded.handedness(), Handedness::Left);
        assert_eq!(ZDownRightHanded.handedness(), Handedness::Right);
        assert_eq!(YUpRightHanded.handedness(), Handedness::Right);

        // converting between frames of different handedness reverses winding
        assert!(ZUpLeftHanded.flips_handedness(ZDownRightHanded));
        assert!(ZDownRightHanded.flips_handedness(ZUpLeftHanded));
        assert!(ZUpLeftHanded.flips_handedness(YUpRightHanded));
        // equal handedness keeps winding
        assert!(!ZDownRightHanded.flips_handedness(YUpRightHanded));
        assert!(!ZUpLeftHanded.flips_handedness(ZUpLeftHanded));
    }
}
