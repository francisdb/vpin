//! Backwards-compatibility shims for tables saved with older
//! `.vpx` file versions.
//!
//! VPinball applies a batch of load-time patches to old tables to
//! preserve their original rendering. Our `extract` / `assemble`
//! flows deliberately don't run these patches - they need to
//! round-trip raw bytes - but the OBJ / glTF exporters want the
//! same view the runtime renderer sees, otherwise pre-10.8 tables
//! drop their playfield mesh (and other quirks).
//!
//! All current rules come from vpinball's `PinTable::LoadGameFromStorage`
//! in `src/parts/pintable.cpp:1977-2025`, gated on
//! `loadfileversion < 1080`. Add new version-gated rules here as
//! they're discovered.
//!
//! Each rule has a helper function below so the call site doesn't
//! have to remember the version check; helpers fall through to the
//! stored value for 10.8+ tables. Adding a helper without a
//! consumer is fine - clippy will flag it as dead but the
//! module-level `#[allow(dead_code)]` keeps things quiet until the
//! consumer arrives.
//!
//! # Pre-10.8 (`< 1080`) rules
//!
//! See vpinball `pintable.cpp:1977-2025`.
//!
//! - **Playfield primitives** (any [`Primitive`] whose name matches
//!   `playfield_mesh`): forced `is_visible = true`,
//!   `static_rendering = true`, `disable_lighting_below = 1.0`,
//!   `depth_bias = 100000.0`, `backfaces_enabled = false`.
//! - **Any primitive with an opaque material**
//!   (`!opacity_active || opacity == 1.0`): forced
//!   `disable_lighting_below = 1.0` to compensate for vpinball
//!   discarding the texture alpha channel under those material
//!   conditions pre-10.8.
//! - **All lights**:
//!     - `reflection_enabled = false` (lights were never reflected
//!       pre-10.8). The field isn't modelled on our [`Light`]
//!       struct yet, so no helper.
//!     - `height = is_bulb_light ? bulb_halo_height : 0.0`
//!       (pre-10.8 lights had no explicit z; this preserves the
//!       original falloff curve).
//!     - If `!is_bulb_light`: `show_bulb_mesh = false`,
//!       `show_reflection_on_ball = false`.
//!     - If `!visible`: `show_bulb_mesh = false`.
//! - **Table glass**: `glass_bottom_height = glass_top_height`
//!   (glass was horizontal until 10.8).

use crate::vpx::gameitem::light::Light;
use crate::vpx::gameitem::primitive::Primitive;
use crate::vpx::version::Version;

/// File version cutoff (`major*100 + minor*10`) for the 10.8
/// compatibility rules. Vpinball stores this as `loadfileversion`
/// in `PinTable::LoadGameFromStorage`.
const VPX_10_8: u32 = 1080;

#[inline]
fn is_pre_10_8(version: &Version) -> bool {
    version.u32() < VPX_10_8
}

// ---------------------------------------------------------------------------
// Primitive rules
// ---------------------------------------------------------------------------

/// Effective visibility for an exported primitive, applying the
/// pre-10.8 playfield-force-visible rule. VPinball ignored
/// `is_visible` on the playfield mesh until 10.8; any value stored
/// on a pre-10.8 table is therefore meaningless and we treat the
/// playfield as visible.
pub fn primitive_is_visible(primitive: &Primitive, version: &Version) -> bool {
    if is_pre_10_8(version) && primitive.is_playfield() {
        return true;
    }
    primitive.is_visible
}

/// Effective `static_rendering` flag. Pre-10.8 vpinball always
/// rendered the playfield mesh into the static buffer regardless
/// of this flag.
#[allow(dead_code)] // no exporter currently reads `static_rendering`
pub fn primitive_static_rendering(primitive: &Primitive, version: &Version) -> bool {
    if is_pre_10_8(version) && primitive.is_playfield() {
        return true;
    }
    primitive.static_rendering
}

/// Effective `depth_bias`. Pre-10.8 vpinball rendered the
/// playfield before everything else, equivalent to a very large
/// depth bias.
#[allow(dead_code)] // no exporter currently reads `depth_bias`
pub fn primitive_depth_bias(primitive: &Primitive, version: &Version) -> f32 {
    if is_pre_10_8(version) && primitive.is_playfield() {
        return 100_000.0;
    }
    primitive.depth_bias
}

/// Effective `backfaces_enabled`. Pre-10.8 playfield meshes did
/// not handle back faces.
#[allow(dead_code)] // no exporter currently reads `backfaces_enabled`
pub fn primitive_backfaces_enabled(primitive: &Primitive, version: &Version) -> Option<bool> {
    if is_pre_10_8(version) && primitive.is_playfield() {
        return Some(false);
    }
    primitive.backfaces_enabled
}

/// Effective `disable_lighting_below`. Two pre-10.8 rules combine:
/// the playfield always blocks under-lighting (since playfields
/// rendered before the bulb-light buffer), and any primitive with
/// a fully-opaque material also blocks it (vpinball discarded the
/// texture alpha when the material had no active opacity).
///
/// `material_opacity` is `Some((opacity_active, opacity))` when the
/// caller resolved the primitive's material, `None` when no
/// material was found - matching vpinball's `if (mat && ...)`
/// null check, which skips the force when the material is missing.
pub fn primitive_disable_lighting_below(
    primitive: &Primitive,
    material_opacity: Option<(bool, f32)>,
    version: &Version,
) -> Option<f32> {
    if is_pre_10_8(version) {
        if primitive.is_playfield() {
            return Some(1.0);
        }
        if let Some((opacity_active, opacity)) = material_opacity
            && (!opacity_active || opacity == 1.0)
        {
            return Some(1.0);
        }
    }
    primitive.disable_lighting_below
}

// ---------------------------------------------------------------------------
// Light rules
// ---------------------------------------------------------------------------

/// Effective light height. Pre-10.8 lights had no z coordinate;
/// the runtime applied `is_bulb_light ? bulb_halo_height : 0.0`
/// so the falloff curve matched the historical render offset.
/// Any stored `height` on an old table is therefore meaningless.
pub fn light_height(light: &Light, version: &Version) -> Option<f32> {
    if is_pre_10_8(version) {
        return Some(if light.is_bulb_light {
            light.bulb_halo_height
        } else {
            0.0
        });
    }
    light.height
}

/// Effective `show_bulb_mesh` flag. Two pre-10.8 rules combine:
/// classic (non-bulb) lights could not have a bulb mesh, and any
/// light explicitly marked invisible did not render its bulb
/// mesh either.
pub fn light_show_bulb_mesh(light: &Light, version: &Version) -> bool {
    if is_pre_10_8(version) {
        if !light.is_bulb_light {
            return false;
        }
        if !light.visible.unwrap_or(true) {
            return false;
        }
    }
    light.show_bulb_mesh
}

/// Effective `show_reflection_on_ball`. Pre-10.8 classic lights
/// could not have ball reflections.
#[allow(dead_code)] // no exporter currently reads `show_reflection_on_ball`
pub fn light_show_reflection_on_ball(light: &Light, version: &Version) -> bool {
    if is_pre_10_8(version) && !light.is_bulb_light {
        return false;
    }
    light.show_reflection_on_ball
}

// ---------------------------------------------------------------------------
// Table-level rules
// ---------------------------------------------------------------------------

/// Effective glass bottom height. The glass was horizontal until
/// 10.8, so for older tables vpinball collapses
/// `glass_bottom_height` onto `glass_top_height`. From 10.8 the
/// table author can tilt the glass and the stored field is honoured
/// (falling back to the top height when absent for very old tables
/// that don't carry the field at all).
#[allow(dead_code)] // no exporter currently reads `glass_bottom_height`
pub fn glass_bottom_height(
    glass_top_height: f32,
    glass_bottom_height: Option<f32>,
    version: &Version,
) -> f32 {
    if is_pre_10_8(version) {
        return glass_top_height;
    }
    glass_bottom_height.unwrap_or(glass_top_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(v: &str) -> Version {
        Version::parse(v).unwrap()
    }

    // --- Primitive ----------------------------------------------------------

    fn primitive(name: &str) -> Primitive {
        Primitive {
            name: name.to_string(),
            ..Primitive::default()
        }
    }

    #[test]
    fn pre_10_8_forces_playfield_visible_only() {
        let mut p = primitive("playfield_mesh");
        p.is_visible = false;
        assert!(primitive_is_visible(&p, &version("1072")));

        let mut other = primitive("SomePrimitive");
        other.is_visible = false;
        assert!(!primitive_is_visible(&other, &version("1072")));
    }

    #[test]
    fn post_10_8_honours_stored_visibility() {
        let mut p = primitive("playfield_mesh");
        p.is_visible = false;
        for v in ["1080", "1081"] {
            assert!(!primitive_is_visible(&p, &version(v)));
        }
    }

    #[test]
    fn playfield_match_is_case_insensitive() {
        let p = primitive("Playfield_Mesh");
        assert!(primitive_is_visible(&p, &version("1072")));
    }

    #[test]
    fn pre_10_8_forces_playfield_static_depth_backfaces() {
        let p = primitive("playfield_mesh");
        assert!(primitive_static_rendering(&p, &version("1072")));
        assert_eq!(primitive_depth_bias(&p, &version("1072")), 100_000.0);
        assert_eq!(
            primitive_backfaces_enabled(&p, &version("1072")),
            Some(false)
        );
    }

    #[test]
    fn post_10_8_keeps_stored_static_depth_backfaces() {
        let mut p = primitive("playfield_mesh");
        p.static_rendering = false;
        p.depth_bias = 42.0;
        p.backfaces_enabled = Some(true);
        assert!(!primitive_static_rendering(&p, &version("1080")));
        assert_eq!(primitive_depth_bias(&p, &version("1080")), 42.0);
        assert_eq!(
            primitive_backfaces_enabled(&p, &version("1080")),
            Some(true)
        );
    }

    #[test]
    fn pre_10_8_disable_lighting_below_playfield() {
        let p = primitive("playfield_mesh");
        assert_eq!(
            primitive_disable_lighting_below(&p, Some((true, 0.5)), &version("1072")),
            Some(1.0)
        );
    }

    #[test]
    fn pre_10_8_disable_lighting_below_opaque_material() {
        // Material with opacity 1.0 -> forced 1.0.
        let p = primitive("Other");
        assert_eq!(
            primitive_disable_lighting_below(&p, Some((true, 1.0)), &version("1072")),
            Some(1.0)
        );
        // Material with opacity_active=false -> also forced.
        assert_eq!(
            primitive_disable_lighting_below(&p, Some((false, 0.5)), &version("1072")),
            Some(1.0)
        );
        // Active translucent material -> stored value passes through.
        let mut p2 = primitive("Other");
        p2.disable_lighting_below = Some(0.25);
        assert_eq!(
            primitive_disable_lighting_below(&p2, Some((true, 0.5)), &version("1072")),
            Some(0.25)
        );
    }

    #[test]
    fn pre_10_8_missing_material_does_not_force() {
        // Mirrors vpinball's `if (mat && ...)` null check - missing
        // material means the opaque-material rule doesn't fire.
        let p = primitive("Other");
        assert_eq!(
            primitive_disable_lighting_below(&p, None, &version("1072")),
            None
        );
    }

    #[test]
    fn post_10_8_disable_lighting_below_passes_through() {
        let mut p = primitive("playfield_mesh");
        p.disable_lighting_below = Some(0.5);
        // Even for the playfield, 10.8+ trusts the stored value.
        assert_eq!(
            primitive_disable_lighting_below(&p, Some((true, 1.0)), &version("1080")),
            Some(0.5)
        );
    }

    // --- Light --------------------------------------------------------------

    fn light_with(is_bulb: bool, visible: Option<bool>) -> Light {
        Light {
            is_bulb_light: is_bulb,
            visible,
            show_bulb_mesh: true,
            show_reflection_on_ball: true,
            bulb_halo_height: 50.0,
            height: Some(99.0),
            ..Light::default()
        }
    }

    #[test]
    fn pre_10_8_light_height_classic_is_zero() {
        let l = light_with(false, Some(true));
        assert_eq!(light_height(&l, &version("1072")), Some(0.0));
    }

    #[test]
    fn pre_10_8_light_height_bulb_uses_halo_height() {
        let l = light_with(true, Some(true));
        assert_eq!(light_height(&l, &version("1072")), Some(50.0));
    }

    #[test]
    fn post_10_8_light_height_passes_through() {
        let l = light_with(false, Some(true));
        assert_eq!(light_height(&l, &version("1080")), Some(99.0));
    }

    #[test]
    fn pre_10_8_classic_lights_drop_bulb_mesh_and_reflection() {
        let l = light_with(false, Some(true));
        assert!(!light_show_bulb_mesh(&l, &version("1072")));
        assert!(!light_show_reflection_on_ball(&l, &version("1072")));
    }

    #[test]
    fn pre_10_8_invisible_lights_drop_bulb_mesh() {
        let l = light_with(true, Some(false));
        assert!(!light_show_bulb_mesh(&l, &version("1072")));
        // Bulb light reflection isn't gated on visibility, so it survives.
        assert!(light_show_reflection_on_ball(&l, &version("1072")));
    }

    #[test]
    fn post_10_8_lights_pass_through() {
        let l = light_with(false, Some(false));
        assert!(light_show_bulb_mesh(&l, &version("1080")));
        assert!(light_show_reflection_on_ball(&l, &version("1080")));
    }

    // --- Glass --------------------------------------------------------------

    #[test]
    fn pre_10_8_glass_is_horizontal() {
        assert_eq!(
            glass_bottom_height(210.0, Some(100.0), &version("1072")),
            210.0
        );
        assert_eq!(glass_bottom_height(210.0, None, &version("1072")), 210.0);
    }

    #[test]
    fn post_10_8_glass_uses_stored_bottom() {
        assert_eq!(
            glass_bottom_height(210.0, Some(100.0), &version("1080")),
            100.0
        );
        // Missing field falls back to top height.
        assert_eq!(glass_bottom_height(210.0, None, &version("1080")), 210.0);
    }
}
