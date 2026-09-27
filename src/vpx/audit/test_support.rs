//! Tables the audit tests start from.

use super::{Kind, VPX, audit_kinds};
use crate::vpx;
use crate::vpx::gameitem::GameItemEnum;

pub(super) fn blank_vpx() -> VPX {
    let bytes = include_bytes!("../../../testdata/completely_blank_table_10_7_4.vpx");
    #[allow(clippy::unwrap_used)]
    vpx::from_bytes(bytes).unwrap()
}

/// The blank fixture with its dangling default references cleared
pub(super) fn clean_vpx() -> VPX {
    let mut vpx = blank_vpx();
    // the template ships a decal without a name, like vpinball's own
    // blank table
    for item in &mut vpx.gameitems {
        if let GameItemEnum::Decal(decal) = item
            && decal.name.is_empty()
        {
            decal.name = "Decal1".to_string();
        }
    }
    vpx.gamedata.image.clear();
    vpx.gamedata.image_color_grade.clear();
    vpx.gamedata.ball_image.clear();
    vpx.gamedata.ball_image_front.clear();
    vpx.gamedata.env_image = None;
    vpx.gamedata.ball_spherical_mapping = Some(false);
    // the template's score textbox uses Lucida Sans Unicode, a Windows
    // only font
    for item in &mut vpx.gameitems {
        if let GameItemEnum::TextBox(textbox) = item {
            textbox.font = crate::vpx::gameitem::font::Font::new(
                0,
                Default::default(),
                400,
                120000,
                "Arial".to_string(),
            );
        }
    }
    // the template script draws random numbers without Randomize
    vpx.gamedata.set_code("Option Explicit\r\n".to_string());
    // the template ships an image nothing refers to
    vpx.images.clear();
    vpx.gamedata.images_size = 0;
    // and a script that plays the sample table's sounds
    vpx.gamedata.set_code("Option Explicit\r\n".to_string());
    // and materials nothing uses
    let unused: Vec<String> = audit_kinds(&vpx)
        .into_iter()
        .find_map(|finding| match finding {
            Kind::UnusedMaterials { names, .. } => Some(names),
            _ => None,
        })
        .unwrap_or_default();
    if let Some(materials) = &mut vpx.gamedata.materials {
        materials.retain(|material| !unused.contains(&material.name));
    }
    vpx.gamedata
        .materials_old
        .retain(|material| !unused.contains(&material.name));
    if let Some(physics) = &mut vpx.gamedata.materials_physics_old {
        physics.retain(|material| !unused.contains(&material.name));
    }
    vpx.gamedata.materials_size = vpx.gamedata.materials_old.len() as u32;
    vpx
}
