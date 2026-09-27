//! References from game items and table settings to images, materials,
//! surfaces, part groups and collection items that do not exist, and
//! references the file cannot store.

use super::names::{material_names, name_set};
use super::{Kind, VPX};
use crate::vpx::gameitem::GameItemEnum;
use std::collections::HashSet;

/// The sentinel vpinball's editor writes for "no image selected"
const NONE_SELECTION: &str = "<None>";

/// The references of the table settings and of every game item
pub(super) fn check_references(vpx: &VPX, findings: &mut Vec<Kind>) {
    let images = name_set(vpx.images.iter().map(|image| image.name.as_str()));
    let materials = material_names(vpx);
    // surfaces resolve case sensitively, vpinball's GetSurfaceHeight
    // compares the name exactly while image lookups are case insensitive
    let surfaces: HashSet<&str> = vpx
        .gameitems
        .iter()
        .filter_map(|item| match item {
            GameItemEnum::Wall(wall) => Some(wall.name.as_str()),
            GameItemEnum::Ramp(ramp) => Some(ramp.name.as_str()),
            _ => None,
        })
        .collect();
    let part_groups = name_set(vpx.gameitems.iter().filter_map(|item| match item {
        GameItemEnum::PartGroup(group) => Some(group.name.as_str()),
        _ => None,
    }));
    check_table_settings(vpx, &images, &materials, findings);
    for item in &vpx.gameitems {
        check_item_references(item, &images, &materials, &surfaces, findings);
        check_part_group(item, &part_groups, findings);
    }
}

/// Collection entries that name a game item the table does not have
pub(super) fn check_collections(vpx: &VPX, findings: &mut Vec<Kind>) {
    let item_names = name_set(vpx.gameitems.iter().map(|item| item.name()));
    for collection in &vpx.collections {
        for item in &collection.items {
            if !item_names.contains(item.to_lowercase().as_str()) {
                findings.push(Kind::MissingCollectionItem {
                    collection: collection.name.clone(),
                    item: item.clone(),
                });
            }
        }
    }
}

pub(super) fn item_label(item: &GameItemEnum) -> String {
    format!("{} {:?}", item.type_name(), item.name())
}

pub(super) struct References<'a> {
    pub(super) images: Vec<(&'static str, &'a str)>,
    pub(super) materials: Vec<(&'static str, &'a str)>,
    pub(super) surface: Option<&'a str>,
}

/// A primitive named `playfield_mesh` replaces the built-in playfield
/// geometry, and vpinball copies the playfield image and material from
/// the table settings onto it when it renders. With that primitive
/// hidden the playfield never renders at all. That is how VLM baked
/// tables ship: bake primitives draw the playfield and the table
/// settings still name the image from before the bake. A dangling
/// playfield image or material is then a leftover without effect.
fn playfield_hidden(vpx: &VPX) -> bool {
    vpx.gameitems.iter().any(|item| match item {
        GameItemEnum::Primitive(primitive) => {
            primitive.name.eq_ignore_ascii_case("playfield_mesh") && !primitive.is_visible
        }
        _ => false,
    })
}

fn check_table_settings(
    vpx: &VPX,
    images: &HashSet<String>,
    materials: &HashSet<String>,
    findings: &mut Vec<Kind>,
) {
    let gamedata = &vpx.gamedata;
    let missing = |image: &str| {
        !image.is_empty()
            && !image.eq_ignore_ascii_case(NONE_SELECTION)
            && !images.contains(image.to_lowercase().as_str())
    };
    let playfield_hidden = playfield_hidden(vpx);
    let image_fields: [(&'static str, &str); 4] = [
        (
            "playfield image",
            if playfield_hidden {
                ""
            } else {
                &gamedata.image
            },
        ),
        (
            "desktop backglass image",
            &gamedata.backglass_image_full_desktop,
        ),
        (
            "fullscreen backglass image",
            &gamedata.backglass_image_full_fullscreen,
        ),
        (
            "single screen backglass image",
            gamedata
                .backglass_image_full_single_screen
                .as_deref()
                .unwrap_or(""),
        ),
    ];
    for (field, image) in image_fields {
        if missing(image) {
            findings.push(Kind::MissingImage {
                item: "table settings".to_string(),
                field,
                image: image.to_string(),
            });
        }
    }
    // these render with a built-in when the image is missing
    let fallback_fields: [(&'static str, &str, &'static str); 3] = [
        (
            "environment image",
            gamedata.env_image.as_deref().unwrap_or(""),
            "its built-in environment map",
        ),
        (
            "ball image",
            &gamedata.ball_image,
            "its built-in ball image",
        ),
        ("ball decal image", &gamedata.ball_image_front, "no decal"),
    ];
    for (field, image, fallback) in fallback_fields {
        if missing(image) {
            findings.push(Kind::MissingImageWithFallback {
                field,
                image: image.to_string(),
                fallback,
            });
        }
    }
    if missing(&gamedata.image_color_grade) {
        findings.push(Kind::MissingColorGradeImage {
            image: gamedata.image_color_grade.clone(),
        });
    }
    if !gamedata.image_color_grade.is_empty()
        && let Some(image) = vpx
            .images
            .iter()
            .find(|image| image.name.to_lowercase() == gamedata.image_color_grade.to_lowercase())
        && (image.width, image.height) != (256, 16)
    {
        findings.push(Kind::ColorGradeLutUnusualSize {
            image: image.name.clone(),
            width: image.width,
            height: image.height,
        });
    }
    if !playfield_hidden
        && !gamedata.playfield_material.is_empty()
        && !materials.contains(gamedata.playfield_material.to_lowercase().as_str())
    {
        findings.push(Kind::MissingMaterial {
            item: "table settings".to_string(),
            field: "playfield material",
            material: gamedata.playfield_material.clone(),
        });
    }
}

fn check_part_group(item: &GameItemEnum, part_groups: &HashSet<String>, findings: &mut Vec<Kind>) {
    if let Some(part_group) = item.part_group_name()
        && !part_group.is_empty()
        && !part_groups.contains(part_group.to_lowercase().as_str())
    {
        findings.push(Kind::MissingPartGroup {
            item: item_label(item),
            part_group: part_group.to_string(),
        });
    }
}

fn check_item_references(
    item: &GameItemEnum,
    images: &HashSet<String>,
    materials: &HashSet<String>,
    surfaces: &HashSet<&str>,
    findings: &mut Vec<Kind>,
) {
    let refs = item_references(item);
    for (field, image) in &refs.images {
        if !image.is_empty()
            && !image.eq_ignore_ascii_case(NONE_SELECTION)
            && !images.contains(image.to_lowercase().as_str())
        {
            findings.push(Kind::MissingImage {
                item: item_label(item),
                field,
                image: (*image).to_string(),
            });
        }
    }
    for (field, material) in &refs.materials {
        if !material.is_empty() && !materials.contains(material.to_lowercase().as_str()) {
            findings.push(Kind::MissingMaterial {
                item: item_label(item),
                field,
                material: (*material).to_string(),
            });
        }
    }
    if let Some(surface) = refs.surface
        && !surface.is_empty()
        && !surfaces.contains(surface)
    {
        findings.push(Kind::MissingSurface {
            item: item_label(item),
            surface: surface.to_string(),
        });
    }
    let texts = refs.images.iter().chain(&refs.materials).copied();
    for (field, text) in texts.chain(refs.surface.map(|surface| ("surface", surface))) {
        if text.chars().any(|c| u32::from(c) > 0xFF) {
            findings.push(Kind::UnstorableText {
                item: item_label(item),
                field,
                text: text.to_string(),
            });
        }
    }
}

/// The image, material and surface references an item carries, straight
/// from the per type fields
pub(super) fn item_references(item: &GameItemEnum) -> References<'_> {
    let mut refs = References {
        images: Vec::new(),
        materials: Vec::new(),
        surface: None,
    };
    match item {
        GameItemEnum::Wall(wall) => {
            refs.images.push(("image", &wall.image));
            refs.images.push(("side image", &wall.side_image));
            refs.materials.push(("side material", &wall.side_material));
            refs.materials.push(("top material", &wall.top_material));
            refs.materials
                .push(("slingshot material", &wall.slingshot_material));
            push_optional(
                &mut refs.materials,
                "physics material",
                &wall.physics_material,
            );
        }
        GameItemEnum::Flipper(flipper) => {
            push_optional_image(&mut refs.images, "image", &flipper.image);
            refs.materials.push(("material", &flipper.material));
            refs.materials
                .push(("rubber material", &flipper.rubber_material));
            refs.surface = Some(&flipper.surface);
        }
        GameItemEnum::Bumper(bumper) => {
            refs.materials.push(("cap material", &bumper.cap_material));
            refs.materials
                .push(("base material", &bumper.base_material));
            refs.materials
                .push(("socket material", &bumper.socket_material));
            push_optional(&mut refs.materials, "ring material", &bumper.ring_material);
            refs.surface = Some(&bumper.surface);
        }
        GameItemEnum::Ball(ball) => {
            refs.images.push(("image", &ball.image));
            refs.images.push(("decal image", &ball.image_decal));
        }
        GameItemEnum::Decal(decal) => {
            refs.images.push(("image", &decal.image));
            refs.materials.push(("material", &decal.material));
            // a backglass decal is not placed on a surface
            if !decal.backglass {
                refs.surface = Some(&decal.surface);
            }
        }
        GameItemEnum::Flasher(flasher) => {
            refs.images.push(("image a", &flasher.image_a));
            refs.images.push(("image b", &flasher.image_b));
        }
        GameItemEnum::Gate(gate) => {
            refs.materials.push(("material", &gate.material));
            refs.surface = Some(&gate.surface);
        }
        GameItemEnum::HitTarget(hittarget) => {
            refs.images.push(("image", &hittarget.image));
            refs.materials.push(("material", &hittarget.material));
            push_optional(
                &mut refs.materials,
                "physics material",
                &hittarget.physics_material,
            );
        }
        GameItemEnum::Kicker(kicker) => {
            refs.materials.push(("material", &kicker.material));
            refs.surface = Some(&kicker.surface);
        }
        GameItemEnum::Light(light) => {
            refs.images.push(("image", &light.image));
            // a backglass light is not placed on a surface
            if !light.is_backglass {
                refs.surface = Some(&light.surface);
            }
        }
        GameItemEnum::Plunger(plunger) => {
            refs.images.push(("image", &plunger.image));
            refs.materials.push(("material", &plunger.material));
            refs.surface = Some(&plunger.surface);
        }
        GameItemEnum::Primitive(primitive) => {
            refs.images.push(("image", &primitive.image));
            push_optional_image(&mut refs.images, "normal map", &primitive.normal_map);
            refs.materials.push(("material", &primitive.material));
            push_optional(
                &mut refs.materials,
                "physics material",
                &primitive.physics_material,
            );
        }
        GameItemEnum::Ramp(ramp) => {
            refs.images.push(("image", &ramp.image));
            refs.materials.push(("material", &ramp.material));
            push_optional(
                &mut refs.materials,
                "physics material",
                &ramp.physics_material,
            );
        }
        GameItemEnum::Reel(reel) => {
            refs.images.push(("image", &reel.image));
        }
        GameItemEnum::Rubber(rubber) => {
            refs.images.push(("image", &rubber.image));
            refs.materials.push(("material", &rubber.material));
            push_optional(
                &mut refs.materials,
                "physics material",
                &rubber.physics_material,
            );
        }
        GameItemEnum::Spinner(spinner) => {
            refs.images.push(("image", &spinner.image));
            refs.materials.push(("material", &spinner.material));
            refs.surface = Some(&spinner.surface);
        }
        GameItemEnum::Trigger(trigger) => {
            refs.materials.push(("material", &trigger.material));
            refs.surface = Some(&trigger.surface);
        }
        GameItemEnum::Timer(_)
        | GameItemEnum::TextBox(_)
        | GameItemEnum::LightSequencer(_)
        | GameItemEnum::PartGroup(_)
        | GameItemEnum::Generic(_, _) => {}
    }
    refs
}

fn push_optional<'a>(
    refs: &mut Vec<(&'static str, &'a str)>,
    field: &'static str,
    value: &'a Option<String>,
) {
    if let Some(value) = value {
        refs.push((field, value));
    }
}

fn push_optional_image<'a>(
    refs: &mut Vec<(&'static str, &'a str)>,
    field: &'static str,
    value: &'a Option<String>,
) {
    if let Some(value) = value {
        refs.push((field, value));
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
    fn a_missing_image_reference_is_reported() {
        let mut vpx = clean_vpx();
        vpx.gamedata.image = "no_such_image".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::MissingImage {
                item: "table settings".to_string(),
                field: "playfield image",
                image: "no_such_image".to_string(),
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
    }

    /// A missing playfield image only matters while the playfield renders
    fn with_playfield_mesh(vpx: &mut VPX, is_visible: bool) {
        use crate::vpx::gameitem::primitive::Primitive;
        vpx.gameitems
            .push(GameItemEnum::Primitive(Box::new(Primitive {
                name: "playfield_mesh".to_string(),
                is_visible,
                ..Primitive::default()
            })));
        vpx.gamedata.gameitems_size += 1;
    }

    fn table_setting_references(findings: Vec<Kind>) -> Vec<Kind> {
        findings
            .into_iter()
            .filter(|f| {
                matches!(
                    f,
                    Kind::MissingImage { item, .. } | Kind::MissingMaterial { item, .. }
                        if item == "table settings"
                )
            })
            .collect()
    }

    #[test]
    fn a_hidden_playfield_mesh_silences_dangling_playfield_references() {
        let mut vpx = clean_vpx();
        vpx.gamedata.image = "NGG PF Scan".to_string();
        vpx.gamedata.playfield_material = "no_such_material".to_string();
        with_playfield_mesh(&mut vpx, false);
        assert_eq!(table_setting_references(audit_kinds(&vpx)), vec![]);
    }

    #[test]
    fn a_visible_playfield_mesh_renders_the_playfield_references() {
        let mut vpx = clean_vpx();
        vpx.gamedata.image = "no_such_image".to_string();
        vpx.gamedata.playfield_material = "no_such_material".to_string();
        with_playfield_mesh(&mut vpx, true);
        assert_eq!(
            table_setting_references(audit_kinds(&vpx)),
            vec![
                Kind::MissingImage {
                    item: "table settings".to_string(),
                    field: "playfield image",
                    image: "no_such_image".to_string(),
                },
                Kind::MissingMaterial {
                    item: "table settings".to_string(),
                    field: "playfield material",
                    material: "no_such_material".to_string(),
                },
            ]
        );
    }

    #[test]
    fn a_missing_image_with_a_built_in_fallback_is_a_suggestion() {
        let mut vpx = clean_vpx();
        vpx.gamedata.ball_image = "no_such_image".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::MissingImageWithFallback {
                field: "ball image",
                image: "no_such_image".to_string(),
                fallback: "its built-in ball image",
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "table settings: ball image references missing image \"no_such_image\", vpinball uses its built-in ball image"
        );
    }

    #[test]
    fn a_missing_color_grade_image_is_a_warning() {
        let mut vpx = clean_vpx();
        vpx.gamedata.image_color_grade = "ColorGradeLUT256x16_1to1".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::MissingColorGradeImage {
                image: "ColorGradeLUT256x16_1to1".to_string(),
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
    }

    #[test]
    fn image_lookups_are_case_insensitive() {
        let mut vpx = clean_vpx();
        vpx.images.push(crate::vpx::image::ImageData {
            name: "Playfield".to_string(),
            ..Default::default()
        });
        vpx.gamedata.images_size = 1;
        vpx.gamedata.image = "PLAYFIELD".to_string();
        assert_eq!(audit_kinds(&vpx), Vec::new());
    }

    #[test]
    fn a_missing_collection_item_is_reported() {
        let mut vpx = clean_vpx();
        vpx.collections.push(crate::vpx::collection::Collection {
            name: "Bumpers".to_string(),
            items: vec!["Bumper1".to_string()],
            fire_events: false,
            stop_single_events: false,
            group_elements: false,
        });
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::MissingCollectionItem {
                collection: "Bumpers".to_string(),
                item: "Bumper1".to_string(),
            }]
        );
    }

    #[test]
    fn references_the_file_cannot_store_are_reported() {
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = clean_vpx();
        // Latin-1 text fits one byte per character, Cyrillic does not
        for (name, surface_of_other) in [("fits", "Ap\u{e9}ro"), ("lost", "\u{41c}\u{435}\u{442}")]
        {
            vpx.gameitems.push(GameItemEnum::Wall(Wall {
                name: name.to_string(),
                ..Default::default()
            }));
            vpx.gameitems
                .push(GameItemEnum::Bumper(crate::vpx::gameitem::bumper::Bumper {
                    name: format!("bumper_{name}"),
                    surface: surface_of_other.to_string(),
                    ..Default::default()
                }));
        }
        let unstorable: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|kind| matches!(kind, Kind::UnstorableText { .. }))
            .collect();
        assert_eq!(
            unstorable,
            vec![Kind::UnstorableText {
                item: "Bumper \"bumper_lost\"".to_string(),
                field: "surface",
                text: "\u{41c}\u{435}\u{442}".to_string(),
            }]
        );
        assert_eq!(unstorable[0].code(), "unstorable-text");
        assert_eq!(
            unstorable[0].to_string(),
            "Bumper \"bumper_lost\": surface \"\u{41c}\u{435}\u{442}\" will be saved to the vpx file as \"???\", which stores one byte per character"
        );
    }
}
