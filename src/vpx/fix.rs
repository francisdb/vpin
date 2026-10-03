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
use super::image::ImageData;
use super::images::Webp;
use super::pinbinary::PinBinary;
use log::warn;
use std::collections::HashSet;
use std::io;

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

/// Why a converter left an image alone
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// The script hands the image to FlexDMD as `VPX.name`, and FlexDMD
    /// decodes it itself and cannot read webp
    FlexDmd,
    /// The webp would not be smaller than what is stored
    NotSmaller,
    /// The pixels are deeper than 8 bits, which webp cannot hold
    TooDeep,
    /// The image does not decode, or webp cannot encode it; the error
    Unreadable(String),
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::FlexDmd => write!(f, "FlexDMD reads it and cannot read webp"),
            SkipReason::NotSmaller => write!(f, "the webp would not be smaller"),
            SkipReason::TooDeep => write!(f, "deeper than 8 bits, which webp cannot hold"),
            SkipReason::Unreadable(error) => write!(f, "does not decode: {error}"),
        }
    }
}

/// An image a converter left alone, with the reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedImage {
    name: String,
    reason: SkipReason,
}

impl SkippedImage {
    /// Name of the image in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Why the converter left it alone
    pub fn reason(&self) -> &SkipReason {
        &self.reason
    }
}

/// What a run of [`bitmaps_to_webp`], [`pngs_to_webp`] or
/// [`tgas_to_webp`] did: the images it re-encoded and the ones it would
/// have but left alone, each with the reason. Images that were never
/// candidates, a jpeg for the png converter, are in neither list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageConversion {
    converted: Vec<ConvertedImage>,
    skipped: Vec<SkippedImage>,
}

impl ImageConversion {
    /// The re-encoded images, in the order the table lists them
    pub fn converted(&self) -> &[ConvertedImage] {
        &self.converted
    }

    /// The candidates left alone, in the order the table lists them
    pub fn skipped(&self) -> &[SkippedImage] {
        &self.skipped
    }

    /// Whether the table is left as it was
    pub fn is_empty(&self) -> bool {
        self.converted.is_empty()
    }
}

/// An image re-encoded by [`bitmaps_to_webp`], [`pngs_to_webp`] or
/// [`tgas_to_webp`]
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

    /// Size of the stored image data before the conversion, the png or
    /// the LZW compressed bitmap
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
/// Returns what was converted and what was left alone, with the reason.
pub fn bitmaps_to_webp(vpx: &mut VPX) -> ImageConversion {
    let mut findings = Vec::new();
    assets::check_image_storage(vpx, &mut findings);
    let bitmaps: HashSet<String> = findings
        .into_iter()
        .filter_map(|finding| match finding {
            Kind::BmpImage { image } => Some(image),
            _ => None,
        })
        .collect();
    convert_images(
        vpx.images
            .iter_mut()
            .filter(|image| bitmaps.contains(&image.name)),
        &HashSet::new(),
        |_| true,
        ImageData::bitmap_webp,
    )
}

/// Re-encodes every png image as lossless webp where that is smaller,
/// which it is for most; the picture stays the same. This goes beyond
/// what vpinball does on its own, it reads pngs as they are, so the audit
/// has no finding for it: it is a size lever. Images the script hands to
/// FlexDMD as `VPX.name` are left alone, since FlexDMD decodes them
/// itself and cannot read webp; so are pngs deeper than 8 bits, which
/// webp cannot hold, and any png that does not decode, with a warning.
/// The table screenshot is the stored bytes of the image that links to
/// it, so a png screenshot is converted like any other png, and the
/// linked image follows.
///
/// Returns what was converted and what was left alone, with the reason.
pub fn pngs_to_webp(vpx: &mut VPX) -> ImageConversion {
    let flexdmd = assets::flexdmd_image_names(&vpx.gamedata.code.string);
    lend_screenshot(vpx);
    let conversion = convert_images(
        vpx.images.iter_mut(),
        &flexdmd,
        ImageData::is_stored_png,
        ImageData::png_webp,
    );
    reclaim_screenshot(vpx);
    conversion
}

/// Moves the screenshot bytes onto the image that links to them, so the
/// converters see the picture as they see any other; undone by
/// [`reclaim_screenshot`]. Nothing moves when no image links to the
/// screenshot, which vpinball tolerates too
fn lend_screenshot(vpx: &mut VPX) {
    if let Some(link) = vpx.images.iter_mut().find(|image| image.is_link())
        && let Some(data) = vpx.info.screenshot.take()
    {
        link.jpeg = Some(PinBinary {
            path: link.path.clone(),
            name: link.name.clone(),
            internal_name: None,
            data,
        });
    }
}

/// Moves the bytes lent by [`lend_screenshot`], converted or not, back
/// to the screenshot
fn reclaim_screenshot(vpx: &mut VPX) {
    if let Some(link) = vpx.images.iter_mut().find(|image| image.is_link())
        && let Some(jpeg) = link.jpeg.take()
    {
        vpx.info.screenshot = Some(jpeg.data);
    }
}

/// Re-encodes every tga image as lossless webp where that is smaller,
/// which it is for the tga vpinball tables carry; the picture stays the
/// same. This goes beyond what vpinball does on its own, so the audit has
/// no finding for it: it is a size lever. A tga is found by its content,
/// the TRUEVISION-XFILE footer, since the format has no signature at the
/// start; vpin can read the footer because it holds the whole image in
/// memory. The older footerless tga has no content signal, so a `.tga`
/// extension is the fallback that still converts it. An image that is
/// neither footered nor named `.tga`, one deeper than 8 bits, or one that
/// does not decode is left alone (the last with a warning).
///
/// Images the script hands to FlexDMD as `VPX.name` are left alone:
/// FlexDMD decodes them itself and cannot read webp.
///
/// Returns what was converted and what was left alone, with the reason.
pub fn tgas_to_webp(vpx: &mut VPX) -> ImageConversion {
    let flexdmd = assets::flexdmd_image_names(&vpx.gamedata.code.string);
    convert_images(
        vpx.images.iter_mut(),
        &flexdmd,
        ImageData::is_stored_tga,
        ImageData::tga_webp,
    )
}

/// Runs a conversion over images, recording the ones it changed and the
/// candidates it left alone. `flexdmd` holds the lower case names of the
/// images FlexDMD reads, which are never converted; `is_candidate` tells
/// which of those would have been, so they are reported as skipped and
/// the rest not at all. `convert` answers `None` for an image that is
/// not its kind.
fn convert_images<'a>(
    images: impl Iterator<Item = &'a mut ImageData>,
    flexdmd: &HashSet<String>,
    is_candidate: impl Fn(&ImageData) -> bool,
    convert: fn(&mut ImageData) -> io::Result<Option<Webp>>,
) -> ImageConversion {
    let mut conversion = ImageConversion::default();
    for image in images {
        let reason = if flexdmd.contains(&image.name.to_lowercase()) {
            if !is_candidate(image) {
                continue;
            }
            SkipReason::FlexDmd
        } else {
            let bytes_before = stored_bytes(image);
            match convert(image) {
                Ok(Some(Webp::Converted)) => {
                    conversion.converted.push(ConvertedImage {
                        name: image.name.clone(),
                        bytes_before,
                        bytes_after: stored_bytes(image),
                    });
                    continue;
                }
                Ok(Some(Webp::NotSmaller)) => SkipReason::NotSmaller,
                Ok(Some(Webp::TooDeep)) => SkipReason::TooDeep,
                Ok(None) => continue,
                Err(e) => {
                    warn!("Skipping image {}: {e}", image.name);
                    SkipReason::Unreadable(e.to_string())
                }
            }
        };
        conversion.skipped.push(SkippedImage {
            name: image.name.clone(),
            reason,
        });
    }
    conversion
}

/// Size of the image data as the file holds it: the encoded bytes, or the
/// LZW compressed bitmap
fn stored_bytes(image: &ImageData) -> usize {
    image
        .jpeg
        .as_ref()
        .map(|jpeg| jpeg.data.len())
        .or_else(|| {
            image
                .bits
                .as_ref()
                .map(|bits| bits.lzw_compressed_data.len())
        })
        .unwrap_or(0)
}

/// Why [`wavs_to_flac`] left a sound alone
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SoundSkipReason {
    /// The WAV is not plain PCM this converter re-encodes: a `format_tag`
    /// other than 1 (such as ADPCM or float), or a sample depth other
    /// than 8, 16 or 24 bits
    NotPcm,
    /// The FLAC would not be smaller than the stored WAV samples
    NotSmaller,
    /// The samples could not be encoded as FLAC; the error
    Unencodable(String),
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Display for SoundSkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SoundSkipReason::NotPcm => {
                write!(f, "not plain 8, 16 or 24-bit PCM, which is not re-encoded")
            }
            SoundSkipReason::NotSmaller => write!(f, "the flac would not be smaller"),
            SoundSkipReason::Unencodable(error) => write!(f, "does not encode: {error}"),
        }
    }
}

/// A sound [`wavs_to_flac`] left alone, with the reason
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedSound {
    name: String,
    reason: SoundSkipReason,
}

#[cfg(not(target_family = "wasm"))]
impl SkippedSound {
    /// Name of the sound in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Why the converter left it alone
    pub fn reason(&self) -> &SoundSkipReason {
        &self.reason
    }
}

/// A sound re-encoded by [`wavs_to_flac`]
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedSound {
    name: String,
    bytes_before: usize,
    bytes_after: usize,
}

#[cfg(not(target_family = "wasm"))]
impl ConvertedSound {
    /// Name of the sound in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size of the stored WAV samples before the conversion
    pub fn bytes_before(&self) -> usize {
        self.bytes_before
    }

    /// Size of the stored FLAC file after it
    pub fn bytes_after(&self) -> usize {
        self.bytes_after
    }
}

/// What a run of [`wavs_to_flac`] did: the sounds it re-encoded and the
/// WAVs it would have but left alone, each with the reason. Sounds that
/// were never candidates, a sound already in a file format, are in
/// neither list.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SoundConversion {
    converted: Vec<ConvertedSound>,
    skipped: Vec<SkippedSound>,
}

#[cfg(not(target_family = "wasm"))]
impl SoundConversion {
    /// The re-encoded sounds, in the order the table lists them
    pub fn converted(&self) -> &[ConvertedSound] {
        &self.converted
    }

    /// The candidates left alone, in the order the table lists them
    pub fn skipped(&self) -> &[SkippedSound] {
        &self.skipped
    }

    /// Whether the table is left as it was
    pub fn is_empty(&self) -> bool {
        self.converted.is_empty()
    }
}

/// Re-encodes the PCM WAV sounds of a table as FLAC where that is
/// smaller. FLAC is lossless, so the sounds play back the same, but only
/// vpinball builds with the miniaudio sound engine (10.8.1 and later)
/// decode FLAC. Unlike the webp conversions this changes which builds
/// play the table, so it is an opt-in size lever: a caller enables it
/// knowingly, as `vpxtool optimize` does behind a flag.
///
/// A WAV that is not PCM is left alone and reported, as is one the FLAC
/// would not shrink or that does not encode. A sound already stored as a
/// file (ogg, mp3, an existing flac) was never a candidate and is in
/// neither list.
///
/// Returns what was converted and what was left alone, with the reason.
#[cfg(not(target_family = "wasm"))]
pub fn wavs_to_flac(vpx: &mut VPX) -> SoundConversion {
    let mut conversion = SoundConversion::default();
    for sound in &mut vpx.sounds {
        if !sound.is_wav() {
            // a file in some other format, never a candidate
            continue;
        }
        let bytes_before = sound.data.len();
        let reason = match sound.wav_to_flac() {
            Ok(Some(crate::vpx::sound::Flac::Converted)) => {
                conversion.converted.push(ConvertedSound {
                    name: sound.name.clone(),
                    bytes_before,
                    bytes_after: sound.data.len(),
                });
                continue;
            }
            Ok(Some(crate::vpx::sound::Flac::NotSmaller)) => SoundSkipReason::NotSmaller,
            // a wav that is not PCM: wav_to_flac declines it
            Ok(None) => SoundSkipReason::NotPcm,
            Err(e) => {
                warn!("Skipping sound {}: {e}", sound.name);
                SoundSkipReason::Unencodable(e.to_string())
            }
        };
        conversion.skipped.push(SkippedSound {
            name: sound.name.clone(),
            reason,
        });
    }
    conversion
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::{audit_kinds, test_support::clean_vpx};
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::font::Font;
    use crate::vpx::gameitem::textbox::TextBox;
    use crate::vpx::images::tests::{
        bitmap_image, deep_png, encoded_image, loose_png, noise_png, tga_image,
    };
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

        let conversion = bitmaps_to_webp(&mut vpx);

        let bytes_after = vpx.images[0]
            .jpeg
            .as_ref()
            .map_or(0, |jpeg| jpeg.data.len());
        assert_eq!(
            conversion,
            ImageConversion {
                converted: vec![ConvertedImage {
                    name: "bmp".to_string(),
                    bytes_before,
                    bytes_after,
                }],
                skipped: Vec::new(),
            }
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
        assert_eq!(bitmaps_to_webp(&mut vpx), ImageConversion::default());
        Ok(())
    }

    #[test]
    fn pngs_are_converted_except_what_flexdmd_reads() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(loose_png("logo", 64, 64)?);
        vpx.add_or_replace_image(loose_png("apron", 64, 64)?);
        vpx.add_or_replace_image(encoded_image("jpg", "jpg", 8, 8)?);
        vpx.gamedata.set_code(
            "Option Explicit\r\nSet img = FlexDMD.NewImage(\"logo\", \"VPX.Logo\")\r\n".to_string(),
        );

        let conversion = pngs_to_webp(&mut vpx);

        let converted = conversion.converted();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].name(), "apron");
        assert!(converted[0].bytes_after() < converted[0].bytes_before());
        assert_eq!(
            conversion.skipped(),
            [SkippedImage {
                name: "logo".to_string(),
                reason: SkipReason::FlexDmd,
            }]
        );
        assert_eq!(vpx.images[0].ext(), "png");
        assert_eq!(vpx.images[1].ext(), "webp");
        assert_eq!(vpx.images[2].ext(), "jpg");
        // a second run has nothing to do but report the same skip
        let again = pngs_to_webp(&mut vpx);
        assert!(again.is_empty());
        assert_eq!(again.skipped(), conversion.skipped());
        Ok(())
    }

    #[test]
    fn the_png_screenshot_is_converted_with_its_linked_image() -> TestResult {
        use crate::vpx::images::tests::link_image;
        let mut vpx = VPX::default();
        let png = loose_png("Capture", 64, 64)?;
        let png_bytes = png.jpeg.as_ref().map(|jpeg| jpeg.data.clone());
        vpx.info.screenshot = png_bytes.clone();
        let mut link = link_image("Capture");
        link.width = 64;
        link.height = 64;
        vpx.add_or_replace_image(link);
        vpx.gamedata.screen_shot = "Capture".to_string();

        let conversion = pngs_to_webp(&mut vpx);

        assert_eq!(conversion.converted().len(), 1);
        assert_eq!(conversion.converted()[0].name(), "Capture");
        assert!(conversion.skipped().is_empty());
        let screenshot = vpx.info.screenshot.as_deref().unwrap_or_default();
        assert!(screenshot.starts_with(b"RIFF"), "screenshot is not a webp");
        assert!(screenshot.len() < png_bytes.map_or(0, |bytes| bytes.len()));
        // the linked image follows the bytes, and keeps linking
        assert_eq!(vpx.images[0].ext(), "webp");
        assert!(vpx.images[0].is_link());
        assert!(vpx.images[0].jpeg.is_none());
        assert_eq!((vpx.images[0].width, vpx.images[0].height), (64, 64));
        // a second run has nothing to do
        assert!(pngs_to_webp(&mut vpx).is_empty());
        assert!(vpx.info.screenshot.is_some());

        // a screenshot nothing links to is left where it is
        let mut orphan = VPX::default();
        orphan.info.screenshot = Some(b"\x89PNG\r\n\x1a\nnot really".to_vec());
        assert!(pngs_to_webp(&mut orphan).is_empty());
        assert!(orphan.info.screenshot.is_some());
        Ok(())
    }

    #[test]
    fn pngs_left_alone_are_reported_with_the_reason() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(deep_png("deep")?);
        vpx.add_or_replace_image(noise_png("noise", 64, 64)?);
        // a png whose data is cut short does not decode
        let mut broken = loose_png("broken", 64, 64)?;
        if let Some(jpeg) = &mut broken.jpeg {
            jpeg.data.truncate(40);
        }
        vpx.add_or_replace_image(broken);
        vpx.add_or_replace_image(encoded_image("jpg", "jpg", 8, 8)?);

        let conversion = pngs_to_webp(&mut vpx);

        assert!(conversion.is_empty());
        assert_eq!(conversion.skipped().len(), 3, "{conversion:#?}");
        assert_eq!(
            conversion.skipped()[0],
            SkippedImage {
                name: "deep".to_string(),
                reason: SkipReason::TooDeep,
            }
        );
        assert_eq!(
            conversion.skipped()[1],
            SkippedImage {
                name: "noise".to_string(),
                reason: SkipReason::NotSmaller,
            }
        );
        assert_eq!(conversion.skipped()[2].name(), "broken");
        assert!(matches!(
            conversion.skipped()[2].reason(),
            SkipReason::Unreadable(_)
        ));
        assert_eq!(
            conversion.skipped()[0].reason().to_string(),
            "deeper than 8 bits, which webp cannot hold"
        );
        assert!(vpx.images.iter().all(|image| image.ext() != "webp"));
        Ok(())
    }

    #[test]
    fn tgas_are_converted_except_what_flexdmd_reads() -> TestResult {
        let mut vpx = VPX::default();
        // a tga under a png name (found by footer), an unmarked one (no
        // .tga name, no footer), and a FlexDMD tga
        vpx.add_or_replace_image(tga_image("art", "png", 64, 64, true)?);
        vpx.add_or_replace_image(tga_image("unmarked", "png", 64, 64, false)?);
        vpx.add_or_replace_image(tga_image("dmd", "tga", 64, 64, true)?);
        vpx.gamedata.set_code(
            "Option Explicit\r\nSet img = FlexDMD.NewImage(\"d\", \"VPX.dmd\")\r\n".to_string(),
        );

        let conversion = tgas_to_webp(&mut vpx);

        let converted = conversion.converted();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].name(), "art");
        assert!(converted[0].bytes_after() < converted[0].bytes_before());
        // the unmarked tga is no candidate, so it is not a skip either
        assert_eq!(
            conversion.skipped(),
            [SkippedImage {
                name: "dmd".to_string(),
                reason: SkipReason::FlexDmd,
            }]
        );
        assert_eq!(vpx.images[0].ext(), "webp"); // tga by footer, converted
        assert_eq!(vpx.images[1].ext(), "png"); // unmarked, left alone
        assert_eq!(vpx.images[2].ext(), "tga"); // FlexDMD, left alone
        // a second run has nothing to do but report the same skip
        let again = tgas_to_webp(&mut vpx);
        assert!(again.is_empty());
        assert_eq!(again.skipped(), conversion.skipped());
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn wavs_are_converted_and_the_rest_reported_or_ignored() {
        use crate::vpx::sound::flac_tests::pcm_wav;
        let mut vpx = VPX::default();
        // a PCM wav (converts), a non-PCM wav (reported NotPcm), and an ogg
        // file (never a candidate, ignored)
        vpx.sounds.push(pcm_wav("music", 2, 16, 4096));
        let mut adpcm = pcm_wav("voice", 1, 16, 64);
        adpcm.wave_form.format_tag = 2;
        vpx.sounds.push(adpcm);
        let mut ogg = pcm_wav("song", 2, 16, 64);
        ogg.path = "C:\\sounds\\song.ogg".to_string();
        vpx.sounds.push(ogg);

        let conversion = wavs_to_flac(&mut vpx);

        let converted = conversion.converted();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].name(), "music");
        assert!(converted[0].bytes_after() < converted[0].bytes_before());
        // only the non-PCM wav is reported; the ogg is no candidate
        assert_eq!(
            conversion.skipped(),
            [SkippedSound {
                name: "voice".to_string(),
                reason: SoundSkipReason::NotPcm,
            }]
        );
        assert_eq!(vpx.sounds[0].ext(), "flac"); // converted
        assert_eq!(vpx.sounds[1].ext(), "wav"); // non-PCM, left alone
        assert_eq!(vpx.sounds[2].ext(), "ogg"); // not a candidate
        // a second run has nothing to convert but reports the same skip
        let again = wavs_to_flac(&mut vpx);
        assert!(again.is_empty());
        assert_eq!(again.skipped(), conversion.skipped());
    }
}
