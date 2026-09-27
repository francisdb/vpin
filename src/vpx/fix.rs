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
use log::warn;
use std::collections::HashSet;

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

/// An image re-encoded by [`bitmaps_to_webp`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedImage {
    name: String,
    bytes_before: usize,
    bytes_after: usize,
}

impl ConvertedImage {
    /// Name of the image in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size of the stored image data before the conversion, the LZW
    /// compressed bitmap
    pub fn bytes_before(&self) -> usize {
        self.bytes_before
    }

    /// Size of the stored image data after it, the webp
    pub fn bytes_after(&self) -> usize {
        self.bytes_after
    }
}

/// Re-encodes every bitmap image as lossless webp, the way vpinball does
/// when it loads one: the images the audit reports as `bmp-image`. The
/// picture stays the same; the hash vpinball keeps of the encoded bytes
/// is dropped since it no longer matches. A bitmap that does not decode
/// is left as it is, with a warning.
///
/// Returns the converted images in the order the table lists them, empty
/// when the table is left as it was.
pub fn bitmaps_to_webp(vpx: &mut VPX) -> Vec<ConvertedImage> {
    let mut findings = Vec::new();
    assets::check_image_storage(vpx, &mut findings);
    let bitmaps: HashSet<String> = findings
        .into_iter()
        .filter_map(|finding| match finding {
            Kind::BmpImage { image } => Some(image),
            _ => None,
        })
        .collect();
    let mut converted = Vec::new();
    for image in vpx
        .images
        .iter_mut()
        .filter(|image| bitmaps.contains(&image.name))
    {
        let bytes_before = image
            .bits
            .as_ref()
            .map_or(0, |bits| bits.lzw_compressed_data.len());
        match image.bitmap_to_webp() {
            Ok(true) => converted.push(ConvertedImage {
                name: image.name.clone(),
                bytes_before,
                bytes_after: image.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len()),
            }),
            Ok(false) => {}
            Err(e) => warn!("Skipping image {}: {e}", image.name),
        }
    }
    converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::{audit_kinds, test_support::clean_vpx};
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::font::Font;
    use crate::vpx::gameitem::textbox::TextBox;
    use crate::vpx::images::tests::{bitmap_image, encoded_image};
    use crate::vpx::pinbinary::PinBinary;
    use crate::vpx::ttf::font_with_names;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

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

    #[test]
    fn bitmaps_are_converted_to_webp() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(bitmap_image("bmp", 8, 8));
        vpx.add_or_replace_image(encoded_image("png", "png", 8, 8)?);
        vpx.add_or_replace_image(encoded_image("jpg", "jpg", 8, 8)?);
        let bytes_before = vpx.images[0]
            .bits
            .as_ref()
            .map_or(0, |bits| bits.lzw_compressed_data.len());

        let converted = bitmaps_to_webp(&mut vpx);

        let bytes_after = vpx.images[0]
            .jpeg
            .as_ref()
            .map_or(0, |jpeg| jpeg.data.len());
        assert_eq!(
            converted,
            vec![ConvertedImage {
                name: "bmp".to_string(),
                bytes_before,
                bytes_after,
            }]
        );
        assert!(bytes_after > 0);
        assert!(vpx.images.iter().all(|image| image.bits.is_none()));
        assert_eq!(vpx.images[0].ext(), "webp");
        assert_eq!(vpx.images[1].ext(), "png");
        assert_eq!(vpx.images[2].ext(), "jpg");
        assert!(
            !audit_kinds(&vpx)
                .iter()
                .any(|finding| matches!(finding, Kind::BmpImage { .. }))
        );
        // a second run has nothing to do
        assert_eq!(bitmaps_to_webp(&mut vpx), Vec::new());
        Ok(())
    }
}
