//! How game items and the table's render settings behave at play time:
//! timers, lights, textboxes, primitives and glass.

use super::assets::picture_is_opaque;
use super::references::item_label;
use super::{Kind, VPX};
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::light::Fader;
use crate::vpx::gameitem::textbox::TextBox;
use std::collections::HashMap;

/// two inches in vp units (1 VPU = 0.53975 mm)
const TWO_INCHES_VPU: f32 = 2.0 * 25.4 / 0.539_75;

pub(super) fn check_render_settings(vpx: &VPX, findings: &mut Vec<Kind>) {
    let gamedata = &vpx.gamedata;
    if let Some(bottom) = gamedata.glass_bottom_height
        && bottom > gamedata.glass_top_height
    {
        findings.push(Kind::GlassHeightInvalid {
            detail: "the bottom is higher than the top",
        });
    }
    if gamedata.glass_top_height < TWO_INCHES_VPU
        || gamedata
            .glass_bottom_height
            .is_some_and(|bottom| bottom < TWO_INCHES_VPU)
    {
        findings.push(Kind::GlassHeightInvalid {
            detail: "the glass is below two inches",
        });
    }
    // vpinball defaults to the legacy mapping when the field is absent,
    // which is the case for every table saved before 10.8
    if gamedata.ball_spherical_mapping.unwrap_or(true) {
        findings.push(Kind::BallSphericalMapping);
    }
}

/// A textbox flagged as DMD, or whose text contains "DMD" (the VP 10.0
/// legacy way to flag it), only ever draws the controller's DMD frames.
/// Its text is never rendered, so vpinball never touches its font
/// (textbox.cpp)
pub(super) fn renders_dmd(textbox: &TextBox) -> bool {
    textbox.is_dmd == Some(true) || textbox.text.to_uppercase().contains("DMD")
}

pub(super) fn check_item_behavior(item: &GameItemEnum, findings: &mut Vec<Kind>) {
    if let GameItemEnum::TextBox(textbox) = item
        && renders_dmd(textbox)
    {
        findings.push(Kind::TextboxUsedForDmd {
            item: item_label(item),
        });
    }
    if let GameItemEnum::Light(light) = item
        && light.intensity < 0.0
    {
        findings.push(Kind::NegativeLightIntensity {
            item: item_label(item),
        });
    }
    // an absent fader record means vpinball's default, linear; a fade
    // speed that is not above zero (that includes NaN) never moves the
    // intensity, with either fading fader
    if let GameItemEnum::Light(light) = item
        && !matches!(light.fader, Some(Fader::None))
    {
        // rendered as text so the finding stays comparable
        let unusable = |speed: f32| {
            (speed.is_nan() || speed <= 0.0).then(|| {
                if speed == 0.0 {
                    "0".to_string()
                } else {
                    speed.to_string()
                }
            })
        };
        let up = unusable(light.fade_speed_up);
        let down = unusable(light.fade_speed_down);
        if up.is_some() || down.is_some() {
            findings.push(Kind::LightCannotFade {
                item: item_label(item),
                up,
                down,
                lit: light.intensity > 0.0,
            });
        }
    }
    if let Some(timer) = item.timer()
        && timer.is_enabled
        && timer.interval != -1
        && timer.interval != -2
        && timer.interval < 17
    {
        findings.push(Kind::FastTimer {
            item: item_label(item),
            interval: timer.interval,
        });
    }
}

/// One in a thousand primitives in a corpus of 306 thousand has more
/// vertices than this; the ten above it are whole-playfield bakes of
/// VPW tables
const HUGE_MESH_VERTICES: u32 = 1_000_000;

pub(super) fn check_mesh_size(item: &GameItemEnum, findings: &mut Vec<Kind>) {
    if let GameItemEnum::Primitive(primitive) = item
        && let Some(vertices) = primitive.num_vertices
        && vertices > HUGE_MESH_VERTICES
    {
        findings.push(Kind::HugeMesh {
            item: item_label(item),
            vertices,
            indices: primitive.num_indices.unwrap_or(0),
        });
    }
}

/// vpinball's own audit of the same (pintable.cpp `AuditTable`). The
/// translucency is the effective one: vpinball already switches it off at
/// load for an opaque material in a table saved before 10.8
pub(super) fn check_primitive_translucency(vpx: &VPX, findings: &mut Vec<Kind>) {
    // images repeat across primitives and telling whether one is opaque
    // can mean decoding it
    let mut opaque_images: HashMap<String, bool> = HashMap::new();
    for item in &vpx.gameitems {
        let GameItemEnum::Primitive(primitive) = item else {
            continue;
        };
        if !primitive.is_visible || primitive.static_rendering {
            continue;
        }
        // vpinball resolves a missing material to its default, which is
        // opaque
        let (opacity_active, opacity) =
            crate::vpx::compat::material_opacity(vpx, &primitive.material).unwrap_or((false, 1.0));
        let below = crate::vpx::compat::primitive_disable_lighting_below(
            primitive,
            Some((opacity_active, opacity)),
            &vpx.version,
        );
        if below.unwrap_or(1.0) == 1.0 || (opacity_active && opacity != 1.0) {
            continue;
        }
        let image_is_opaque = *opaque_images
            .entry(primitive.image.to_lowercase())
            .or_insert_with(|| {
                vpx.images
                    .iter()
                    .find(|image| image.name.eq_ignore_ascii_case(&primitive.image))
                    // a picture that does not decode is left alone
                    .is_none_or(|image| picture_is_opaque(image).unwrap_or(false))
            });
        if image_is_opaque {
            findings.push(Kind::OpaquePrimitiveTranslucency {
                item: item_label(item),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use crate::vpx::audit::{Severity, audit_kinds};
    use crate::vpx::gameitem::GameItemEnum;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_wrong_size_color_grade_lut_is_reported() {
        let mut vpx = clean_vpx();
        vpx.images.push(crate::vpx::image::ImageData {
            name: "lut".to_string(),
            width: 512,
            height: 512,
            ..Default::default()
        });
        vpx.gamedata.image_color_grade = "LUT".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::ColorGradeLutUnusualSize {
                image: "lut".to_string(),
                width: 512,
                height: 512,
            }]
        );
    }

    #[test]
    fn a_fast_timer_is_reported() {
        let mut vpx = clean_vpx();
        let mut flasher = crate::vpx::gameitem::flasher::Flasher {
            name: "F1".to_string(),
            ..Default::default()
        };
        flasher.timer.is_enabled = true;
        flasher.timer.interval = 5;
        vpx.add_game_item(crate::vpx::gameitem::GameItemEnum::Flasher(flasher));
        vpx.gamedata
            .set_code("Option Explicit\r\nSub F1_Timer\r\nEnd Sub\r\n".to_string());
        let findings = audit_kinds(&vpx);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(matches!(&findings[0], Kind::FastTimer { interval: 5, .. }));
    }

    #[test]
    fn a_negative_light_intensity_is_an_error() {
        let mut vpx = clean_vpx();
        let light = crate::vpx::gameitem::light::Light {
            name: "L1".to_string(),
            intensity: -1.0,
            ..Default::default()
        };
        vpx.add_game_item(crate::vpx::gameitem::GameItemEnum::Light(light));
        let findings = audit_kinds(&vpx);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].severity(), Severity::Error);
    }

    #[test]
    fn an_upside_down_glass_is_reported() {
        let mut vpx = clean_vpx();
        vpx.gamedata.glass_top_height = 200.0;
        vpx.gamedata.glass_bottom_height = Some(300.0);
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::GlassHeightInvalid {
                detail: "the bottom is higher than the top",
            }]
        );
    }

    #[test]
    fn a_huge_mesh_is_informational() {
        use crate::vpx::gameitem::primitive::Primitive;
        let mut vpx = clean_vpx();
        vpx.collections.clear();
        vpx.gamedata.collections_size = 0;
        vpx.gameitems = vec![
            GameItemEnum::Primitive(Box::new(Primitive {
                name: "Bake".to_string(),
                num_vertices: Some(2_000_000),
                num_indices: Some(2_100_000),
                ..Primitive::default()
            })),
            GameItemEnum::Primitive(Box::new(Primitive {
                name: "Peg".to_string(),
                num_vertices: Some(131),
                num_indices: Some(396),
                ..Primitive::default()
            })),
        ];
        vpx.gamedata.gameitems_size = 2;
        // replacing every item leaves the template's materials unused
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| !matches!(finding, Kind::UnusedMaterials { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![Kind::HugeMesh {
                item: "Primitive \"Bake\"".to_string(),
                vertices: 2_000_000,
                indices: 2_100_000,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
    }

    fn translucent_primitives(vpx: &VPX) -> Vec<Kind> {
        audit_kinds(vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Kind::OpaquePrimitiveTranslucency { .. }))
            .collect()
    }

    #[test]
    fn translucency_on_an_opaque_primitive_is_reported() {
        use crate::vpx::gameitem::primitive::Primitive;
        use crate::vpx::image::ImageData;
        use crate::vpx::pinbinary::PinBinary;
        let image = |name: &str, alpha: u8, is_opaque: Option<bool>| {
            let mut png = Vec::new();
            ::image::RgbaImage::from_pixel(2, 2, ::image::Rgba([1, 2, 3, alpha]))
                .write_to(
                    &mut std::io::Cursor::new(&mut png),
                    ::image::ImageFormat::Png,
                )
                .expect("encodes");
            ImageData {
                name: name.to_string(),
                path: format!("{name}.png"),
                width: 2,
                height: 2,
                is_opaque,
                jpeg: Some(PinBinary {
                    path: format!("{name}.png"),
                    name: name.to_string(),
                    internal_name: None,
                    data: png,
                }),
                ..Default::default()
            }
        };
        let translucent = |name: &str| Primitive {
            name: name.to_string(),
            disable_lighting_below: Some(0.5),
            ..Primitive::default()
        };
        let mut vpx = clean_vpx();
        vpx.version = crate::vpx::version::Version::new(1080);
        vpx.images = vec![
            image("solid", 255, None),
            image("cutout", 128, None),
            // the stored flag wins over the pixels
            image("flagged", 255, Some(false)),
        ];
        let mut clear = crate::vpx::material::Material::default();
        clear.name = "Clear".to_string();
        clear.opacity_active = true;
        clear.opacity = 0.5;
        vpx.gamedata
            .materials
            .get_or_insert_with(Vec::new)
            .push(clear);
        vpx.gameitems = vec![
            translucent("Bare"),
            Primitive {
                image: "Solid".to_string(),
                ..translucent("Textured")
            },
            Primitive {
                disable_lighting_below: None,
                ..translucent("Default")
            },
            Primitive {
                static_rendering: true,
                ..translucent("Static")
            },
            Primitive {
                is_visible: false,
                ..translucent("Hidden")
            },
            Primitive {
                material: "clear".to_string(),
                ..translucent("Glass")
            },
            Primitive {
                image: "cutout".to_string(),
                ..translucent("Cutout")
            },
            Primitive {
                image: "flagged".to_string(),
                ..translucent("Flagged")
            },
        ]
        .into_iter()
        .map(|primitive| GameItemEnum::Primitive(Box::new(primitive)))
        .collect();
        let findings = translucent_primitives(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::OpaquePrimitiveTranslucency {
                    item: "Primitive \"Bare\"".to_string(),
                },
                Kind::OpaquePrimitiveTranslucency {
                    item: "Primitive \"Textured\"".to_string(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
    }

    #[test]
    fn a_table_from_before_10_8_has_its_translucency_switched_off_at_load() {
        use crate::vpx::gameitem::primitive::Primitive;
        let mut vpx = clean_vpx();
        let mut plastic = crate::vpx::material::Material::default();
        plastic.name = "Plastic".to_string();
        vpx.gamedata
            .materials
            .get_or_insert_with(Vec::new)
            .push(plastic);
        vpx.gameitems = vec![GameItemEnum::Primitive(Box::new(Primitive {
            name: "Ramp".to_string(),
            material: "Plastic".to_string(),
            disable_lighting_below: Some(0.5),
            ..Primitive::default()
        }))];
        vpx.gameitems
            .push(GameItemEnum::Primitive(Box::new(Primitive {
                name: "Bare".to_string(),
                disable_lighting_below: Some(0.5),
                ..Primitive::default()
            })));
        vpx.version = crate::vpx::version::Version::new(1080);
        assert_eq!(translucent_primitives(&vpx).len(), 2);
        vpx.version = crate::vpx::version::Version::new(1072);
        assert_eq!(translucent_primitives(&vpx), vec![]);
    }

    #[test]
    fn a_light_with_a_zero_fade_speed_cannot_fade() {
        use crate::vpx::gameitem::light::Light;
        let lit = |name: &str, fader: Option<Fader>, up: f32, down: f32, intensity: f32| {
            GameItemEnum::Light(Light {
                name: name.to_string(),
                fader,
                fade_speed_up: up,
                fade_speed_down: down,
                intensity,
                ..Light::default()
            })
        };
        let light =
            |name: &str, fader: Option<Fader>, up: f32, down: f32| lit(name, fader, up, down, 1.0);
        let mut vpx = clean_vpx();
        vpx.collections.clear();
        vpx.gamedata.collections_size = 0;
        vpx.gameitems = vec![
            light("fine", Some(Fader::Linear), 0.2, 0.2),
            light("instant", Some(Fader::None), 0.0, 0.0),
            light("stuck", None, 0.0, 0.0),
            light("halfway", Some(Fader::Incandescent), 0.2, -1.0),
            light("nan", Some(Fader::Linear), f32::NAN, 0.2),
            light("inf", Some(Fader::Linear), f32::INFINITY, 0.2),
        ];
        // saved dark, with the zero the editor writes in that case
        vpx.gameitems
            .push(lit("dark", Some(Fader::Linear), 0.0, 0.0, 0.0));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        // replacing every item leaves the template's materials unused
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| !matches!(finding, Kind::UnusedMaterials { .. }))
            .collect();
        let messages: Vec<(String, Severity)> = findings
            .iter()
            .map(|finding| (finding.to_string(), finding.severity()))
            .collect();
        assert_eq!(
            messages,
            vec![
                (
                    "Light \"stuck\": fade speeds are 0, state changes never show".to_string(),
                    Severity::Warning
                ),
                (
                    "Light \"halfway\": fade down speed is -1, state changes never show"
                        .to_string(),
                    Severity::Warning
                ),
                (
                    "Light \"nan\": fade up speed is NaN, state changes never show".to_string(),
                    Severity::Warning
                ),
                (
                    "Light \"dark\": fade speeds are 0, state changes never show".to_string(),
                    Severity::Suggestion
                ),
            ]
        );
        assert_eq!(
            findings[0],
            Kind::LightCannotFade {
                item: "Light \"stuck\"".to_string(),
                up: Some("0".to_string()),
                down: Some("0".to_string()),
                lit: true,
            }
        );
    }
}
