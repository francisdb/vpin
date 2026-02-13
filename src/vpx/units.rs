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
}
