//! Changes to a table that act on what [`audit`](crate::vpx::audit)
//! reports: repairs, and the size reductions that keep the table rendering
//! and playing the same.
//!
//! Every function here takes a `&mut VPX`, changes it in memory and
//! returns a record of what it changed. None of them does I/O: the caller
//! owns the single write, and rewriting the file is also what compacts
//! it. A function finds what to act on by running the check behind the
//! finding rather than by taking findings as arguments: a list of
//! findings held by a caller goes stale as soon as the first change is
//! made, and a removal is not something to do from a stale list. Reading
//! the findings is the dry run.
//!
//! See <https://github.com/francisdb/vpin/issues/524> for the plan this
//! is part of.

use super::VPX;
use super::audit::{Kind, assets};

/// An embedded font removed from a table by [`drop_unused_fonts`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedFont {
    name: String,
    bytes: usize,
}

impl RemovedFont {
    /// Name of the font in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size of the font data in bytes, what the removal saves
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Removes the embedded fonts no textbox or decal uses and the script
/// never mentions: the fonts the audit reports as `unused-font`. Data
/// that is not a font is left alone, since nothing can tell whether it
/// is used.
///
/// Returns the removed fonts in the order the table listed them, empty
/// when the table is left as it was.
pub fn drop_unused_fonts(vpx: &mut VPX) -> Vec<RemovedFont> {
    let mut findings = Vec::new();
    assets::check_fonts(vpx, &mut findings);
    let unused: Vec<(String, Vec<String>)> = findings
        .into_iter()
        .filter_map(|finding| match finding {
            Kind::UnusedFont { font, faces } => Some((font, faces)),
            _ => None,
        })
        .collect();
    if unused.is_empty() {
        return Vec::new();
    }
    let mut removed = Vec::new();
    vpx.fonts.retain(|font| {
        let unused = unused
            .iter()
            .any(|(name, faces)| *name == font.name && *faces == font.face_names());
        if unused {
            removed.push(RemovedFont {
                name: font.name.clone(),
                bytes: font.data.len(),
            });
        }
        !unused
    });
    vpx.gamedata.fonts_size = vpx.fonts.len() as u32;
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::{audit_kinds, test_support::clean_vpx};
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::font::Font;
    use crate::vpx::gameitem::textbox::TextBox;
    use crate::vpx::pinbinary::PinBinary;
    use crate::vpx::ttf::font_with_names;
    use pretty_assertions::assert_eq;

    fn font(name: &str, family: &str) -> PinBinary {
        PinBinary {
            name: name.to_string(),
            internal_name: None,
            path: format!("{name}.ttf"),
            data: font_with_names(family, family),
        }
    }

    /// The clean table with a font a textbox uses, one the script names,
    /// one nothing uses and a binary that is not a font
    fn table_with_fonts() -> VPX {
        let mut vpx = clean_vpx();
        vpx.fonts = vec![
            font("led", "Advanced LED Board-7"),
            font("script_only", "Emerald Beacon"),
            font("unused", "Nobody Uses This"),
            PinBinary {
                name: "junk".to_string(),
                internal_name: None,
                path: "junk.ttf".to_string(),
                data: vec![1, 2, 3],
            },
        ];
        vpx.gamedata.fonts_size = 4;
        vpx.gameitems.push(GameItemEnum::TextBox(TextBox {
            name: "Score".to_string(),
            font: Font::new(
                0,
                Default::default(),
                400,
                120000,
                "Advanced LED Board-7".to_string(),
            ),
            ..TextBox::default()
        }));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        vpx.gamedata.set_code(
            "Option Explicit\r\n' FlexDMD.NewFont(\"Emerald Beacon\", 1)\r\n".to_string(),
        );
        vpx
    }

    #[test]
    fn unused_fonts_are_dropped_and_the_count_follows() {
        let mut vpx = table_with_fonts();
        let bytes = vpx.fonts[2].data.len();

        let removed = drop_unused_fonts(&mut vpx);

        assert_eq!(
            removed,
            vec![RemovedFont {
                name: "unused".to_string(),
                bytes,
            }]
        );
        let names: Vec<&str> = vpx.fonts.iter().map(|font| font.name.as_str()).collect();
        assert_eq!(names, vec!["led", "script_only", "junk"]);
        assert_eq!(vpx.gamedata.fonts_size, 3);
        assert_eq!(audit_kinds(&vpx), Vec::new());
    }

    #[test]
    fn a_table_without_unused_fonts_is_left_as_it_was() {
        let mut vpx = table_with_fonts();
        vpx.fonts.remove(2);
        vpx.gamedata.fonts_size = 3;
        let before = format!("{vpx:?}");

        assert_eq!(drop_unused_fonts(&mut vpx), Vec::new());

        assert_eq!(format!("{vpx:?}"), before);
    }
}
