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
use super::images::{Shrunk, Webp};
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
    /// The script hands the image to FlexDMD as `VPX.name`, and FlexDMD
    /// draws it pixel for pixel on the DMD, so a scaled one renders wrong
    FlexDmdArtwork,
    /// The image is a color grade lookup table, which the shader reads
    /// by pixel position
    ColorGradeLut,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::FlexDmd => write!(f, "FlexDMD reads it and cannot read webp"),
            SkipReason::NotSmaller => write!(f, "the webp would not be smaller"),
            SkipReason::TooDeep => write!(f, "deeper than 8 bits, which webp cannot hold"),
            SkipReason::Unreadable(error) => write!(f, "does not decode: {error}"),
            SkipReason::FlexDmdArtwork => {
                write!(f, "FlexDMD draws it pixel for pixel on the DMD")
            }
            SkipReason::ColorGradeLut => {
                write!(
                    f,
                    "a color grade LUT, which the shader reads by pixel position"
                )
            }
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

/// An image scaled down by [`shrink_images`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrunkImage {
    name: String,
    width_before: u32,
    height_before: u32,
    width_after: u32,
    height_after: u32,
    bytes_before: usize,
    bytes_after: usize,
}

impl ShrunkImage {
    /// Name of the image in the table, as the table spelled it
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Width and height of the picture before
    pub fn size_before(&self) -> (u32, u32) {
        (self.width_before, self.height_before)
    }

    /// Width and height of the picture now
    pub fn size_after(&self) -> (u32, u32) {
        (self.width_after, self.height_after)
    }

    /// Size of the stored image data before
    pub fn bytes_before(&self) -> usize {
        self.bytes_before
    }

    /// Size of the stored image data now
    pub fn bytes_after(&self) -> usize {
        self.bytes_after
    }
}

/// What a run of [`shrink_images`] did: the images it scaled down and the
/// ones over the limit it left alone, each with the reason. Images that
/// already fit, and links, which hold no picture, are in neither list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageShrink {
    shrunk: Vec<ShrunkImage>,
    skipped: Vec<SkippedImage>,
}

impl ImageShrink {
    /// The scaled down images, in the order the table lists them
    pub fn shrunk(&self) -> &[ShrunkImage] {
        &self.shrunk
    }

    /// The images over the limit left alone, in the order the table
    /// lists them
    pub fn skipped(&self) -> &[SkippedImage] {
        &self.skipped
    }

    /// Whether the table is left as it was
    pub fn is_empty(&self) -> bool {
        self.shrunk.is_empty()
    }
}

/// Scales every image with a side over `max_dimension` down to fit it,
/// keeping the aspect ratio. vpinball does the same on load for every
/// image over its "Maximum texture dimension" video setting (1536 by
/// default on mobile, where people with little memory go down to 512;
/// unlimited on desktop), so this stores what such a device would show
/// anyway, resampled once with a better filter than the bilinear one
/// vpinball uses: the file is smaller, loads faster and no longer holds
/// pixels the device decodes only to throw away. It is lossy, a jpeg is
/// re-encoded at quality 90, so it is an opt-in lever for a table meant
/// for such a device, with no audit finding behind it; `vpxtool optimize`
/// has it behind a flag. Each image keeps its format, see
/// [`ImageData::shrink`] for the exceptions.
///
/// Left alone and reported: images the script hands to FlexDMD as
/// `VPX.name`, which FlexDMD draws pixel for pixel; the color grade image
/// and every 256x16 image, the LUT layout the shader reads and the size
/// of the LUTs a script switches between; images whose scaled down
/// encoding would not be smaller, since vpinball scales those down on load
/// anyway; and images that do not decode or have no encoder, with a
/// warning. Images that already fit are not candidates, nor is a link,
/// which holds no picture.
///
/// Returns what was scaled down and what was left alone, with the reason.
pub fn shrink_images(vpx: &mut VPX, max_dimension: u32) -> ImageShrink {
    let flexdmd = assets::flexdmd_image_names(&vpx.gamedata.code.string);
    let lut = vpx.gamedata.image_color_grade.to_lowercase();
    let mut shrink = ImageShrink::default();
    for image in &mut vpx.images {
        if image.is_link() {
            continue;
        }
        // the header says whether the image is over the limit; one whose
        // header does not parse is left to the decoder to report
        let size = if image.bits.is_some() {
            Some((image.width, image.height))
        } else {
            image.dimensions().ok()
        };
        if size.is_some_and(|(width, height)| width <= max_dimension && height <= max_dimension) {
            continue;
        }
        let lower = image.name.to_lowercase();
        let reason = if flexdmd.contains(&lower) {
            SkipReason::FlexDmdArtwork
        } else if (!lut.is_empty() && lower == lut) || size == Some((256, 16)) {
            SkipReason::ColorGradeLut
        } else {
            let bytes_before = stored_bytes(image);
            match image.shrink_within(max_dimension) {
                Ok(Some(Shrunk::Resized { from, to })) => {
                    shrink.shrunk.push(ShrunkImage {
                        name: image.name.clone(),
                        width_before: from.0,
                        height_before: from.1,
                        width_after: to.0,
                        height_after: to.1,
                        bytes_before,
                        bytes_after: stored_bytes(image),
                    });
                    continue;
                }
                Ok(Some(Shrunk::NotSmaller)) => SkipReason::NotSmaller,
                Ok(None) => continue,
                Err(e) => {
                    warn!("Skipping image {}: {e}", image.name);
                    SkipReason::Unreadable(e.to_string())
                }
            }
        };
        shrink.skipped.push(SkippedImage {
            name: image.name.clone(),
            reason,
        });
    }
    shrink
}

/// The path older tables give a sound for the backglass speakers
#[cfg(not(target_family = "wasm"))]
const BACKGLASS_OUTPUT_MARKER: &str = "* Backglass Output *";

/// Why [`wavs_to_flac`] or [`playfield_sounds_to_mono`] left a sound alone
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SoundSkipReason {
    /// The WAV is not plain PCM the fix converts: for [`wavs_to_flac`] a
    /// `format_tag` other than 1 (such as ADPCM or float) or a sample depth
    /// other than 8, 16 or 24 bits; [`playfield_sounds_to_mono`] also takes
    /// 32 bit PCM and 32 bit float
    NotPcm,
    /// The FLAC would not be smaller than the stored WAV samples
    NotSmaller,
    /// The samples could not be encoded as FLAC; the error
    Unencodable(String),
    /// The path is the `* Backglass Output *` marker, which a `.flac`
    /// extension would not replace: [`rename_backglass_marker_sounds`]
    /// renames the sound first
    BackglassMarker,
    /// An MP3 or Ogg file: re-encoding it to mono would degrade it
    Lossy,
    /// A FLAC of 24 bits or more: its exact mono needs one bit more than
    /// the encoder writes
    TooDeep,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Display for SoundSkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SoundSkipReason::NotPcm => {
                write!(f, "not a plain PCM format this converts")
            }
            SoundSkipReason::NotSmaller => write!(f, "the result would not be smaller"),
            SoundSkipReason::Unencodable(error) => write!(f, "does not encode: {error}"),
            SoundSkipReason::Lossy => write!(f, "a lossy mp3 or ogg, not re-encoded"),
            SoundSkipReason::TooDeep => {
                write!(
                    f,
                    "a flac of 24 bits or more, whose exact mono does not fit"
                )
            }
            SoundSkipReason::BackglassMarker => write!(
                f,
                "has the \"{BACKGLASS_OUTPUT_MARKER}\" path, which needs a .wav name first"
            ),
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
/// would not shrink or that does not encode, and one with the `* Backglass
/// Output *` path: run [`rename_backglass_marker_sounds`] first, as
/// `vpxtool optimize` does, so it gets a name to put the `.flac` extension
/// on. A sound already stored as a file (ogg, mp3, an existing flac) was
/// never a candidate and is in neither list.
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
        if sound.path.eq_ignore_ascii_case(BACKGLASS_OUTPUT_MARKER) {
            conversion.skipped.push(SkippedSound {
                name: sound.name.clone(),
                reason: SoundSkipReason::BackglassMarker,
            });
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

/// Downmixes the sounds the audit reports as `stereo-table-sound` to the
/// mono vpinball plays them as: vpinball 10.8.1 and later decode a
/// playfield sound to one channel, averaging the others into it, so the
/// extra channels only take space. The downmix is exactly vpinball's: a wav
/// keeps its sample format, a FLAC gets one bit more to hold the average
/// exactly. 10.8.0 played such sounds in stereo in its two speaker mode,
/// so this is an opt-in size lever, as `vpxtool optimize` has it behind a
/// flag. Run it before [`wavs_to_flac`] so the FLAC only encodes one
/// channel.
///
/// MP3 and Ogg files are left alone and reported, as are wavs in another
/// sample format, FLACs of 24 bits or more or other than two channels, and
/// a FLAC whose mono would not be smaller.
///
/// Returns what was downmixed and what was left alone, with the reason.
#[cfg(not(target_family = "wasm"))]
pub fn playfield_sounds_to_mono(vpx: &mut VPX) -> SoundConversion {
    let mut findings = Vec::new();
    assets::check_stereo_sounds(vpx, &mut findings);
    let stereo: HashSet<String> = findings
        .into_iter()
        .filter_map(|finding| match finding {
            Kind::StereoTableSound { sound } => Some(sound),
            _ => None,
        })
        .collect();
    let mut conversion = SoundConversion::default();
    for sound in &mut vpx.sounds {
        if !stereo.contains(&sound.name) {
            continue;
        }
        let bytes_before = sound.data.len();
        let reason = match sound.downmix_to_mono() {
            Ok(crate::vpx::sound::Mono::Converted) => {
                conversion.converted.push(ConvertedSound {
                    name: sound.name.clone(),
                    bytes_before,
                    bytes_after: sound.data.len(),
                });
                continue;
            }
            Ok(crate::vpx::sound::Mono::NotPcm) => SoundSkipReason::NotPcm,
            Ok(crate::vpx::sound::Mono::Lossy) => SoundSkipReason::Lossy,
            Ok(crate::vpx::sound::Mono::TooDeep) => SoundSkipReason::TooDeep,
            Ok(crate::vpx::sound::Mono::NotSmaller) => SoundSkipReason::NotSmaller,
            Ok(crate::vpx::sound::Mono::Unsupported(why)) => SoundSkipReason::Unencodable(why),
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

/// A sound given a `.wav` path by [`rename_backglass_marker_sounds`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedSound {
    name: String,
    path_before: String,
    path_after: String,
}

impl RenamedSound {
    /// Name of the sound in the table
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The path the sound had
    pub fn path_before(&self) -> &str {
        &self.path_before
    }

    /// The path the sound has now
    pub fn path_after(&self) -> &str {
        &self.path_after
    }
}

/// Gives the sounds with the `* Backglass Output *` path, which the audit
/// reports as `sound-without-extension`, their name with `.wav` as path:
/// they are stored as wavs, which vpinball 10.8.0 reads from such a path but
/// the 10.8.1 pre-releases only from a `.wav` one. In a table older than
/// 1031, where the marker is what sends the sound to the backglass
/// speakers, the sound's output target is set to the backglass instead.
///
/// Any other path without extension is left alone: the marker only comes
/// from versions that stored such a sound as a wav, while a 10.8.1
/// pre-release stores a sound imported without extension as a plain file,
/// which a `.wav` path would break.
///
/// Run it before [`wavs_to_flac`], which leaves a sound with the marker
/// path alone.
///
/// Returns the renamed sounds in table order, empty when the table is left
/// as it was.
pub fn rename_backglass_marker_sounds(vpx: &mut VPX) -> Vec<RenamedSound> {
    let mut findings = Vec::new();
    assets::check_sound_storage(vpx, &mut findings);
    let names: HashSet<String> = findings
        .into_iter()
        .filter_map(|finding| match finding {
            Kind::SoundWithoutExtension {
                sound,
                backglass_marker: true,
            } => Some(sound),
            _ => None,
        })
        .collect();
    let legacy_marker = vpx.version.u32() < 1031;
    let mut renamed = Vec::new();
    for sound in &mut vpx.sounds {
        if !names.contains(&sound.name) {
            continue;
        }
        let path_before = sound.path.clone();
        sound.path = format!("{}.wav", sound.name);
        if legacy_marker {
            sound.output_target = crate::vpx::sound::OutputTarget::Backglass;
        }
        renamed.push(RenamedSound {
            name: sound.name.clone(),
            path_before,
            path_after: sound.path.clone(),
        });
    }
    renamed
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
    fn images_over_the_limit_are_shrunk_and_the_rest_left_alone() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(encoded_image("big", "png", 200, 100)?);
        vpx.add_or_replace_image(encoded_image("small", "png", 40, 40)?);
        vpx.add_or_replace_image(encoded_image("photo", "jpg", 300, 100)?);
        vpx.add_or_replace_image(bitmap_image("old", 120, 60));
        vpx.add_or_replace_image(crate::vpx::images::tests::link_image("Capture"));
        // the stored size of a bitmap can be stale
        let bytes_before: Vec<usize> = vpx.images.iter().map(stored_bytes).collect();

        let shrink = shrink_images(&mut vpx, 50);

        assert_eq!(shrink.skipped(), []);
        let shrunk = shrink.shrunk();
        assert_eq!(shrunk.len(), 3, "{shrunk:#?}");
        assert_eq!(shrunk[0].name(), "big");
        assert_eq!(shrunk[0].size_before(), (200, 100));
        assert_eq!(shrunk[0].size_after(), (50, 25));
        assert_eq!(shrunk[0].bytes_before(), bytes_before[0]);
        assert!(shrunk[0].bytes_after() < shrunk[0].bytes_before());
        assert_eq!(shrunk[1].name(), "photo");
        // the side that lost the most pixels sets the other, vpinball's way
        assert_eq!(shrunk[1].size_after(), (50, 16));
        assert_eq!(shrunk[2].name(), "old");
        assert_eq!(shrunk[2].size_after(), (50, 25));
        assert_eq!(vpx.images[0].ext(), "png");
        assert_eq!(vpx.images[1].ext(), "png");
        assert_eq!((vpx.images[1].width, vpx.images[1].height), (40, 40));
        assert_eq!(vpx.images[2].ext(), "jpg");
        assert_eq!(vpx.images[3].ext(), "webp");
        assert!(vpx.images[3].bits.is_none());
        assert!(vpx.images[4].is_link());
        for image in &vpx.images[..4] {
            let decoded = image.decode()?;
            assert_eq!(
                (decoded.width(), decoded.height()),
                (image.width, image.height)
            );
            assert!(image.width <= 50 && image.height <= 50, "{}", image.name);
        }
        // a second run has nothing to do
        assert_eq!(shrink_images(&mut vpx, 50), ImageShrink::default());
        Ok(())
    }

    #[test]
    fn images_over_the_limit_left_alone_are_reported_with_the_reason() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(encoded_image("dmd", "png", 200, 100)?);
        vpx.add_or_replace_image(encoded_image("Grade", "png", 512, 32)?);
        vpx.add_or_replace_image(encoded_image("LUT2", "png", 256, 16)?);
        // a smooth gradient is a few hundred bytes of lossless webp at
        // any size, so a smaller one buys nothing
        vpx.add_or_replace_image(encoded_image("gradient", "webp", 400, 400)?);
        // a png whose data is cut short does not decode
        let mut broken = encoded_image("broken", "png", 200, 200)?;
        if let Some(jpeg) = &mut broken.jpeg {
            jpeg.data.truncate(40);
        }
        vpx.add_or_replace_image(broken);
        vpx.add_or_replace_image(encoded_image("fits", "png", 64, 64)?);
        vpx.gamedata.image_color_grade = "grade".to_string();
        vpx.gamedata.set_code(
            "Option Explicit\r\nSet img = FlexDMD.NewImage(\"d\", \"VPX.DMD\")\r\n".to_string(),
        );
        let before = format!("{vpx:?}");

        let shrink = shrink_images(&mut vpx, 100);

        assert!(shrink.is_empty());
        assert_eq!(shrink.skipped().len(), 5, "{shrink:#?}");
        assert_eq!(
            shrink.skipped()[..4],
            [
                SkippedImage {
                    name: "dmd".to_string(),
                    reason: SkipReason::FlexDmdArtwork,
                },
                SkippedImage {
                    name: "Grade".to_string(),
                    reason: SkipReason::ColorGradeLut,
                },
                SkippedImage {
                    name: "LUT2".to_string(),
                    reason: SkipReason::ColorGradeLut,
                },
                SkippedImage {
                    name: "gradient".to_string(),
                    reason: SkipReason::NotSmaller,
                },
            ]
        );
        assert_eq!(shrink.skipped()[4].name(), "broken");
        assert!(matches!(
            shrink.skipped()[4].reason(),
            SkipReason::Unreadable(_)
        ));
        assert_eq!(
            shrink.skipped()[0].reason().to_string(),
            "FlexDMD draws it pixel for pixel on the DMD"
        );
        assert_eq!(
            shrink.skipped()[1].reason().to_string(),
            "a color grade LUT, which the shader reads by pixel position"
        );
        assert_eq!(format!("{vpx:?}"), before);
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

    fn sound(name: &str, path: &str) -> crate::vpx::sound::SoundData {
        crate::vpx::sound::SoundData {
            name: name.to_string(),
            path: path.to_string(),
            data: vec![0; 8],
            wave_form: Default::default(),
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: crate::vpx::sound::OutputTarget::Table,
        }
    }

    #[test]
    fn backglass_marker_sounds_get_a_wav_path() {
        let mut vpx = clean_vpx();
        vpx.sounds.push(sound("bell", "* Backglass Output *"));
        vpx.sounds.push(sound("knock", "C:\\sounds\\knock"));
        vpx.sounds.push(sound("hit", "hit.wav"));

        let renamed = rename_backglass_marker_sounds(&mut vpx);

        assert_eq!(
            renamed
                .iter()
                .map(|r| (r.name(), r.path_before(), r.path_after()))
                .collect::<Vec<_>>(),
            vec![("bell", "* Backglass Output *", "bell.wav")]
        );
        // another path without extension may hold a plain file, left alone
        assert_eq!(vpx.sounds[1].path, "C:\\sounds\\knock");
        // a table of 1031 or newer stores the output target itself
        assert_eq!(
            vpx.sounds[0].output_target,
            crate::vpx::sound::OutputTarget::Table
        );
        assert!(!audit_kinds(&vpx).iter().any(|kind| matches!(
            kind,
            Kind::SoundWithoutExtension {
                backglass_marker: true,
                ..
            }
        )));
        assert!(rename_backglass_marker_sounds(&mut vpx).is_empty());
    }

    #[test]
    fn a_legacy_backglass_marker_becomes_the_output_target() {
        let mut vpx = clean_vpx();
        vpx.version = crate::vpx::version::Version::new(1030);
        vpx.sounds.push(sound("bell", "* Backglass Output *"));

        rename_backglass_marker_sounds(&mut vpx);

        assert_eq!(
            vpx.sounds[0].output_target,
            crate::vpx::sound::OutputTarget::Backglass
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn a_backglass_marker_sound_is_converted_only_after_the_rename() {
        let mut vpx = clean_vpx();
        vpx.sounds.push(sound("bell", "* Backglass Output *"));

        let conversion = wavs_to_flac(&mut vpx);
        assert_eq!(
            conversion.skipped()[0].reason(),
            &SoundSkipReason::BackglassMarker
        );
        assert_eq!(vpx.sounds[0].path, "* Backglass Output *");

        rename_backglass_marker_sounds(&mut vpx);
        let conversion = wavs_to_flac(&mut vpx);
        // a candidate now, whatever the converter makes of the test samples
        assert_eq!(vpx.sounds[0].path, "bell.wav");
        assert!(
            conversion
                .skipped()
                .iter()
                .all(|skipped| skipped.reason() != &SoundSkipReason::BackglassMarker)
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn stereo_playfield_sounds_are_downmixed() {
        use crate::vpx::sound::flac_tests::pcm_wav;
        let mut vpx = clean_vpx();
        vpx.sounds.push(pcm_wav("hit", 2, 16, 1000));
        vpx.sounds.push(pcm_wav("click", 1, 16, 1000));
        let mut music = pcm_wav("music", 2, 16, 1000);
        music.output_target = crate::vpx::sound::OutputTarget::Backglass;
        vpx.sounds.push(music);
        let mut mp3 = pcm_wav("voice", 2, 16, 10);
        mp3.path = "voice.mp3".to_string();
        mp3.data = vec![0xFF, 0xFB, 0x90, 0x64, 0, 0];
        vpx.sounds.push(mp3);

        let conversion = playfield_sounds_to_mono(&mut vpx);

        assert_eq!(
            conversion
                .converted()
                .iter()
                .map(|c| (c.name(), c.bytes_before(), c.bytes_after()))
                .collect::<Vec<_>>(),
            vec![("hit", 4000, 2000)]
        );
        assert_eq!(
            conversion
                .skipped()
                .iter()
                .map(|s| (s.name(), s.reason().clone()))
                .collect::<Vec<_>>(),
            vec![("voice", SoundSkipReason::Lossy)]
        );
        // the backglass sound stays stereo
        assert_eq!(vpx.sounds[2].wave_form.channels, 2);
        let stereo: Vec<_> = audit_kinds(&vpx)
            .into_iter()
            .filter_map(|kind| match kind {
                Kind::StereoTableSound { sound } => Some(sound),
                _ => None,
            })
            .collect();
        assert_eq!(stereo, vec!["voice"]);
    }
}
