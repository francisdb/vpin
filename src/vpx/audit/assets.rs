//! Images, sounds and fonts: what nothing uses, what is stored twice,
//! what is stored in a form that costs more than it needs to, and what
//! the script plays that the table does not have.

use super::items::renders_dmd;
use super::names::name_set;
use super::references::{item_label, item_references};
use super::{Kind, NameKind, VPX};
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::image::ImageData;
use crate::vpx::images;
use crate::vpx::sound as sounds;
use std::collections::{HashMap, HashSet};

/// Screenshots above this size get a [`Kind::LargeScreenshot`]
const LARGE_SCREENSHOT_BYTES: usize = 1024 * 1024;

/// Images stored as bitmaps, images whose content is not what their
/// name says or cannot be read, and images whose stored size disagrees
/// with the picture
pub(crate) fn check_image_storage(vpx: &VPX, findings: &mut Vec<Kind>) {
    for image in &vpx.images {
        if image.bits.is_some() {
            findings.push(Kind::BmpImage {
                image: image.name.clone(),
            });
        }
        if let Some(jpeg) = &image.jpeg {
            check_image_content(image, &jpeg.data, findings);
        }
        if let Some(actual) = picture_dimensions(image)
            && actual != (image.width, image.height)
        {
            findings.push(Kind::ImageDimensionMismatch {
                image: image.name.clone(),
                stored: (image.width, image.height),
                actual,
            });
        }
    }
}

/// What the content of an encoded image is against what its name says,
/// and whether its header reads. Only the header is read: decoding every
/// picture of a table costs seconds, and a signature check finds what
/// fails in practice.
fn check_image_content(image: &ImageData, data: &[u8], findings: &mut Vec<Kind>) {
    let extension = image.ext();
    let named = images::extension_format(&extension);
    match images::content_format(data) {
        Some(format) => {
            if named != Some(format) {
                findings.push(Kind::ImageExtensionMismatch {
                    image: image.name.clone(),
                    extension: extension.clone(),
                    format,
                });
            }
            if !images::decodable(format) {
                findings.push(Kind::UnreadableImage {
                    image: image.name.clone(),
                    format,
                    error: None,
                });
            } else if let Err(error) = image.dimensions() {
                findings.push(Kind::UnreadableImage {
                    image: image.name.clone(),
                    format,
                    error: Some(error.to_string()),
                });
            }
        }
        // no signature: a `.tga` name is the fallback that still reads
        // it, anything else nothing can tell
        None if named == Some("tga") => {
            if let Err(error) = image.dimensions() {
                findings.push(Kind::UnreadableImage {
                    image: image.name.clone(),
                    format: "tga",
                    error: Some(error.to_string()),
                });
            }
        }
        None => findings.push(Kind::ImageFormatUnknown {
            image: image.name.clone(),
            extension,
        }),
    }
}

/// A table screenshot larger than it needs to be
pub(super) fn check_screenshot(vpx: &VPX, findings: &mut Vec<Kind>) {
    if let Some(screenshot) = &vpx.info.screenshot
        && screenshot.len() > LARGE_SCREENSHOT_BYTES
    {
        findings.push(Kind::LargeScreenshot {
            bytes: screenshot.len(),
            png: screenshot.starts_with(&[0x89, b'P', b'N', b'G']),
        });
    }
}

/// Stereo sounds that play from the table, where vpinball positions
/// them
pub(super) fn check_stereo_sounds(vpx: &VPX, findings: &mut Vec<Kind>) {
    for sound in &vpx.sounds {
        if sound.output_target == crate::vpx::sound::OutputTarget::Table
            && sound.wave_form.channels > 1
        {
            findings.push(Kind::StereoTableSound {
                sound: sound.name.clone(),
            });
        }
    }
}

/// Sounds stored as a file whose content is not what the name says or
/// that vpinball's decoder cannot identify. A `.wav` name is left alone:
/// vpinball stores those as a header plus samples and rebuilds the file
pub(super) fn check_sound_storage(vpx: &VPX, findings: &mut Vec<Kind>) {
    for sound in &vpx.sounds {
        let Some(extension) = sound.extension() else {
            findings.push(Kind::SoundFormatUnknown {
                sound: sound.name.clone(),
                extension: String::new(),
            });
            continue;
        };
        if extension.eq_ignore_ascii_case("wav") {
            continue;
        }
        match sounds::content_format(&sound.data) {
            Some(format) => {
                if sounds::extension_format(extension) != Some(format) {
                    findings.push(Kind::SoundExtensionMismatch {
                        sound: sound.name.clone(),
                        extension: extension.to_string(),
                        format,
                    });
                }
            }
            None => findings.push(Kind::SoundFormatUnknown {
                sound: sound.name.clone(),
                extension: extension.to_string(),
            }),
        }
    }
}

/// Whether the picture has no transparency, as vpinball's
/// `Texture::IsOpaque` tells it: the flag vpinball stores with the image
/// since 10.8, or else a look at the alpha channel. A bitmap keeps its
/// alpha channel only when some value is neither 0 nor 255. `None` when
/// the picture does not decode.
pub(super) fn picture_is_opaque(image: &crate::vpx::image::ImageData) -> Option<bool> {
    if let Some(is_opaque) = image.is_opaque {
        return Some(is_opaque);
    }
    if let Some(jpeg) = &image.jpeg {
        use ::image::ImageDecoder;
        let mut reader = ::image::ImageReader::new(std::io::Cursor::new(&jpeg.data));
        if let Some(format) = ::image::ImageFormat::from_extension(image.ext()) {
            reader.set_format(format);
        }
        let decoder = reader.with_guessed_format().ok()?.into_decoder().ok()?;
        // most pictures have no alpha channel, which the header tells
        if !decoder.color_type().has_alpha() {
            return Some(true);
        }
        let picture = ::image::DynamicImage::from_decoder(decoder).ok()?;
        return Some(picture.to_rgba8().pixels().all(|pixel| pixel[3] == 255));
    }
    if let Some(bits) = &image.bits {
        let bytes = crate::vpx::lzw::from_lzw_blocks(&bits.lzw_compressed_data).ok()?;
        return Some(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 0 || pixel[3] == 255),
        );
    }
    None
}

/// The size of the encoded picture, read from its header only; a bitmap
/// has no header, its decoded byte count tells whether the stored size
/// fits. `None` when there is no data or it does not parse.
pub(super) fn picture_dimensions(image: &ImageData) -> Option<(u32, u32)> {
    if image.jpeg.is_some() {
        return image.dimensions().ok();
    }
    if let Some(bits) = &image.bits {
        let bytes = crate::vpx::lzw::from_lzw_blocks(&bits.lzw_compressed_data).ok()?;
        let expected = image.width as usize * image.height as usize * 4;
        // the stored size fits the data, or it does not and the data alone
        // cannot say what the size is
        return (bytes.len() == expected).then_some((image.width, image.height));
    }
    None
}

/// Fonts a table can count on everywhere, lower cased: Microsoft's
/// freely redistributable Core fonts for the Web, which Windows ships
/// and Linux and macOS install as the `msttcorefonts` package, and
/// Liberation Sans, the one font standalone vpinball ships and uses for
/// anything not provided as `Name-Style.ttf` next to the table. The
/// other fonts Windows ships (Segoe UI, Tahoma, Calibri, Lucida Sans
/// Unicode and the like) need a Windows license and are absent
/// elsewhere. Vertical variants are prefixed with `@` and stripped.
const CORE_FONTS: &[&str] = &[
    "liberation sans",
    "andale mono",
    "arial",
    "arial black",
    "comic sans ms",
    "courier new",
    "georgia",
    "impact",
    "times new roman",
    "trebuchet ms",
    "verdana",
    "webdings",
];

/// Textboxes and decals using a font the table does not embed and that
/// is not one of the core fonts. A font name is looked up as a family, so
/// a style suffix such as "Arial Narrow" counts as its family.
pub(super) fn check_font_availability(vpx: &VPX, findings: &mut Vec<Kind>) {
    let embedded: HashSet<String> = vpx
        .fonts
        .iter()
        .flat_map(|font| font.face_names())
        .map(|face| face.to_lowercase())
        .collect();
    let available = |font: &str| {
        let name = font.trim_start_matches('@').to_lowercase();
        embedded.contains(&name)
            || CORE_FONTS
                .iter()
                .any(|known| name == *known || name.starts_with(&format!("{known} ")))
    };
    for item in &vpx.gameitems {
        let font = match item {
            GameItemEnum::TextBox(textbox) if renders_dmd(textbox) => continue,
            GameItemEnum::TextBox(textbox) => textbox.font.name(),
            GameItemEnum::Decal(decal) => decal.font.name(),
            _ => continue,
        };
        if !font.is_empty() && !available(font) {
            findings.push(Kind::NonStandardFont {
                item: item_label(item),
                font: font.to_string(),
            });
        }
    }
}

/// An embedded font is registered by the names inside the font file, so
/// those are what a textbox or decal refers to. A font whose names do not
/// decode is left alone.
pub(crate) fn check_fonts(vpx: &VPX, findings: &mut Vec<Kind>) {
    if vpx.fonts.is_empty() {
        return;
    }
    let used: HashSet<String> = vpx
        .gameitems
        .iter()
        .filter_map(|item| match item {
            GameItemEnum::TextBox(textbox) => Some(textbox.font.name()),
            GameItemEnum::Decal(decal) => Some(decal.font.name()),
            _ => None,
        })
        .map(str::to_lowercase)
        .collect();
    let script = vpx.gamedata.code.string.to_lowercase();
    for font in &vpx.fonts {
        let faces = font.face_names();
        if faces.is_empty() {
            continue;
        }
        let referenced = faces.iter().any(|face| {
            let face = face.to_lowercase();
            used.contains(&face) || script.contains(&face)
        });
        if !referenced {
            findings.push(Kind::UnusedFont {
                font: font.name.clone(),
                faces,
            });
        }
    }
}

/// A string literal of a script, lower cased, with whether it is joined
/// to another expression with `&` or `+` on either side, which makes it
/// the start or the end of a name built at runtime
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Literal {
    pub(crate) text: String,
    /// `... & "text"`: the literal ends a built name
    pub(crate) joined_before: bool,
    /// `"text" & ...`: the literal starts a built name
    pub(crate) joined_after: bool,
}

/// The string literals of a VBScript, comments excluded; a doubled quote
/// inside a string is one quote. A line ending in `_` continues on the
/// next one.
pub(crate) fn script_literals(script: &str) -> Vec<Literal> {
    let mut literals: Vec<Literal> = Vec::new();
    // the statement text so far, literals replaced by a quote, to see
    // what precedes a literal
    let mut statement = String::new();
    // a literal that ended a line with a continuation, waiting to see
    // whether the next line joins it
    let mut continued: Option<usize> = None;
    for line in script.lines() {
        let mut chars = line.chars().peekable();
        let mut current: Option<String> = None;
        let first = line.trim_start().chars().next();
        if let Some(index) = continued.take()
            && matches!(first, Some('&' | '+'))
        {
            literals[index].joined_after = true;
        }
        while let Some(c) = chars.next() {
            match (&mut current, c) {
                (None, '"') => current = Some(String::new()),
                (None, '\'') => break,
                (None, c) => statement.push(c),
                (Some(literal), '"') => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        literal.push('"');
                    } else if let Some(literal) = current.take() {
                        let joined_before =
                            matches!(statement.trim_end().chars().last(), Some('&' | '+'));
                        let rest: String = chars.clone().collect();
                        let rest = rest.trim();
                        let joined_after = matches!(rest.chars().next(), Some('&' | '+'));
                        literals.push(Literal {
                            text: literal.to_lowercase(),
                            joined_before,
                            joined_after,
                        });
                        if rest == "_" || rest.is_empty() {
                            continued = Some(literals.len() - 1);
                        }
                        statement.push('"');
                    }
                }
                (Some(literal), c) => literal.push(c),
            }
        }
        if statement.trim_end().ends_with('_') {
            statement.pop();
        } else {
            statement.clear();
            continued = None;
        }
    }
    literals
}

/// The table images the markdown of the table info refers to as
/// `![alt](name)`; vpinball's in game pages render the blurb, the
/// description and the rules that way and resolve the link as an image
/// name
fn markdown_images(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("![") {
        let after = &rest[start + 2..];
        let Some(open) = after.find("](") else {
            break;
        };
        let link = &after[open + 2..];
        let Some(close) = link.find(')') else {
            break;
        };
        let name = link[..close].trim();
        if !name.is_empty() {
            names.push(name.to_lowercase());
        }
        rest = &link[close + 1..];
    }
    names
}

/// The images the script hands to FlexDMD by name, the `VPX.name` form
/// of a `NewImage` or `NewVideo` path, lower cased. FlexDMD decodes these
/// itself and cannot read webp
pub(crate) fn flexdmd_image_names(script: &str) -> HashSet<String> {
    script_literals(script)
        .iter()
        .filter_map(|literal| {
            literal
                .text
                .strip_prefix("vpx.")
                .and_then(|rest| rest.split(['&', '|']).next())
                .map(|name| name.trim().to_string())
        })
        .collect()
}

/// Whether the script names an asset: as a whole literal, which is how
/// `.Image = "name"` and `PlaySound "name"` refer to it; as the start or
/// end of a name built with `&`, the way `"fx_ballrolling" & i` plays one
/// of a numbered set; or as the `VPX.name` FlexDMD reads a table image by
fn script_names(literals: &[Literal], name: &str) -> bool {
    let lower = name.to_lowercase();
    literals.iter().any(|literal| {
        let text = literal.text.as_str();
        lower == text
            || (literal.joined_after && !text.is_empty() && lower.starts_with(text))
            || (literal.joined_before && !text.is_empty() && lower.ends_with(text))
            || text.strip_prefix("vpx.").is_some_and(|rest| {
                rest.split(['&', '|'])
                    .next()
                    .is_some_and(|n| n.trim() == lower)
            })
    })
}

/// The literal sound names the script plays or stops: the first argument
/// of the `PlaySound` family (`PlaySound`, `PlaySoundAt`, `PlaySoundAtVol`,
/// the helper subs tables define with the same prefix), of `StopSound`,
/// and of the `SoundFX` and `SoundFXDOF` wrappers those calls take the name
/// from. Lower cased, comments excluded. The flag tells that the literal is
/// joined to more with `&`, the `"fx_ballrolling" & i` way of picking one
/// of a numbered set.
pub(crate) fn sound_call_literals(script: &str) -> Vec<(String, bool)> {
    const CALLS: [&str; 3] = ["playsound", "stopsound", "soundfx"];
    let mut names = Vec::new();
    for line in script.lines() {
        let mut rest = line;
        loop {
            let lower = rest.to_lowercase();
            let Some((position, call)) = CALLS
                .iter()
                .filter_map(|call| lower.find(call).map(|position| (position, call.len())))
                .min()
            else {
                break;
            };
            let before = &rest[..position];
            // only outside strings and comments, and at a word start
            if before.matches('"').count() % 2 == 1 || before.contains('\'') {
                break;
            }
            if before
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                rest = &rest[position + call..];
                continue;
            }
            let after = &rest[position + call..];
            // the rest of the identifier, then optional spaces and a paren
            let after = after.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
            let after = after
                .trim_start()
                .strip_prefix('(')
                .unwrap_or(after)
                .trim_start();
            if let Some(literal) = after.strip_prefix('"')
                && let Some(end) = literal.find('"')
            {
                let joined = matches!(
                    literal[end + 1..].trim_start().chars().next(),
                    Some('&' | '+')
                );
                names.push((literal[..end].to_lowercase(), joined));
            }
            rest = after;
        }
    }
    names
}

/// Sounds the script plays or stops by name that the table does not have
pub(super) fn check_sound_calls(vpx: &VPX, findings: &mut Vec<Kind>) {
    let sounds = name_set(vpx.sounds.iter().map(|sound| sound.name.as_str()));
    let mut reported: HashSet<String> = HashSet::new();
    for (name, joined) in sound_call_literals(&vpx.gamedata.code.string) {
        let exists = if joined {
            sounds.iter().any(|sound| sound.starts_with(&name))
        } else {
            sounds.contains(&name)
        };
        if !name.is_empty() && !exists && reported.insert(name.clone()) {
            findings.push(Kind::MissingSound { sound: name });
        }
    }
}

/// Images and sounds nothing refers to: no item, table setting, info
/// markdown or script literal. A name built at runtime from parts the
/// scan cannot follow escapes this, so these are suggestions. Materials
/// are not checked: they cost nothing and every template ships unused
/// ones.
/// Images, sounds and materials nothing refers to: no item, table
/// setting, info markdown or script literal. A name built at runtime
/// from parts the scan cannot follow escapes this, so images and sounds
/// are suggestions; materials, which only clutter, are one informational
/// finding per table.
/// Images and sounds stored more than once under different names, by
/// their encoded bytes; a copy under the same name is a duplicate name
pub(super) fn check_same_asset_data(vpx: &VPX, findings: &mut Vec<Kind>) {
    fn report<'a>(
        kind: NameKind,
        assets: impl Iterator<Item = (&'a str, &'a [u8])>,
        findings: &mut Vec<Kind>,
    ) {
        // The names per content, in the order the table lists them. Equal
        // data has equal length and almost every asset in a table has a
        // unique length, so assets are grouped by length first and the bytes
        // are only compared within a group. Hashing every payload in full
        // was the largest item in the audit profile on big tables.
        let mut by_data: Vec<(&[u8], Vec<&str>)> = Vec::new();
        let mut by_len: HashMap<usize, Vec<usize>> = HashMap::new();
        for (name, data) in assets {
            if data.is_empty() {
                continue;
            }
            let candidates = by_len.entry(data.len()).or_default();
            match candidates.iter().find(|&&at| by_data[at].0 == data) {
                Some(&at) => by_data[at].1.push(name),
                None => {
                    candidates.push(by_data.len());
                    by_data.push((data, vec![name]));
                }
            }
        }
        for (_, names) in by_data {
            let mut distinct: Vec<String> = Vec::new();
            for name in names {
                if !distinct.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
                    distinct.push(name.to_string());
                }
            }
            if distinct.len() > 1 {
                findings.push(Kind::SameAssetData {
                    kind,
                    names: distinct,
                });
            }
        }
    }
    report(
        NameKind::Image,
        vpx.images.iter().map(|image| {
            let data: &[u8] = match (&image.jpeg, &image.bits) {
                (Some(jpeg), _) => &jpeg.data,
                (None, Some(bits)) => &bits.lzw_compressed_data,
                (None, None) => &[],
            };
            (image.name.as_str(), data)
        }),
        findings,
    );
    report(
        NameKind::Sound,
        vpx.sounds
            .iter()
            .map(|sound| (sound.name.as_str(), sound.data.as_slice())),
        findings,
    );
}

pub(super) fn check_unused_assets(vpx: &VPX, findings: &mut Vec<Kind>) {
    let literals = script_literals(&vpx.gamedata.code.string);
    let gamedata = &vpx.gamedata;

    let mut used_images: HashSet<String> = HashSet::new();
    let mut used_materials: HashSet<String> = HashSet::new();
    for item in &vpx.gameitems {
        let refs = item_references(item);
        used_images.extend(refs.images.iter().map(|(_, image)| image.to_lowercase()));
        used_materials.extend(refs.materials.iter().map(|(_, m)| m.to_lowercase()));
    }
    used_materials.insert(gamedata.playfield_material.to_lowercase());
    for text in [
        &vpx.info.table_blurb,
        &vpx.info.table_description,
        &vpx.info.table_rules,
    ]
    .into_iter()
    .flatten()
    {
        used_images.extend(markdown_images(text));
    }
    used_images.extend(
        [
            gamedata.image.as_str(),
            gamedata.backglass_image_full_desktop.as_str(),
            gamedata.backglass_image_full_fullscreen.as_str(),
            gamedata
                .backglass_image_full_single_screen
                .as_deref()
                .unwrap_or(""),
            gamedata.image_color_grade.as_str(),
            gamedata.ball_image.as_str(),
            gamedata.ball_image_front.as_str(),
            gamedata.env_image.as_deref().unwrap_or(""),
            // the image the table screenshot is taken from on save
            gamedata.screen_shot.as_str(),
        ]
        .into_iter()
        .map(str::to_lowercase),
    );
    for image in &vpx.images {
        if !used_images.contains(&image.name.to_lowercase())
            && !script_names(&literals, &image.name)
        {
            let bytes = image
                .jpeg
                .as_ref()
                .map(|jpeg| jpeg.data.len())
                .or_else(|| {
                    image
                        .bits
                        .as_ref()
                        .map(|bits| bits.lzw_compressed_data.len())
                })
                .unwrap_or(0);
            findings.push(Kind::UnusedImage {
                image: image.name.clone(),
                bytes,
            });
        }
    }
    for sound in &vpx.sounds {
        if !script_names(&literals, &sound.name) {
            findings.push(Kind::UnusedSound {
                sound: sound.name.clone(),
                bytes: sound.data.len(),
            });
        }
    }
    let material_names: Vec<&str> = match &gamedata.materials {
        Some(materials) => materials.iter().map(|m| m.name.as_str()).collect(),
        None => gamedata
            .materials_old
            .iter()
            .map(|m| m.name.as_str())
            .collect(),
    };
    let unused: Vec<String> = material_names
        .iter()
        .filter(|name| {
            !used_materials.contains(&name.to_lowercase()) && !script_names(&literals, name)
        })
        .map(|name| name.to_string())
        .collect();
    if !unused.is_empty() {
        findings.push(Kind::UnusedMaterials {
            names: unused,
            total: material_names.len(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use crate::vpx::audit::{Severity, audit_kinds};
    use crate::vpx::gameitem::GameItemEnum;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    #[test]
    fn identical_assets_under_different_names_are_a_suggestion() {
        let mut vpx = clean_vpx();
        let png = |name: &str, data: &[u8]| crate::vpx::image::ImageData {
            name: name.to_string(),
            jpeg: Some(crate::vpx::pinbinary::PinBinary {
                path: format!("C:\\{name}.png"),
                name: name.to_string(),
                internal_name: None,
                data: data.to_vec(),
            }),
            ..Default::default()
        };
        // two copies of one image, a case only duplicate name, one of the
        // same length and one that only shares the prefix
        vpx.images.push(png("flash_a", b"AAAA"));
        vpx.images.push(png("flash_b", b"AAAA"));
        vpx.images.push(png("FLASH_A", b"AAAA"));
        vpx.images.push(png("other", b"BBBB"));
        vpx.images.push(png("longer", b"AAAAA"));
        // the script names them so nothing is unused
        vpx.gamedata.code.string =
            "PlaySound \"x\": a = \"flash_a\" & \"flash_b\" & \"other\" & \"longer\"".to_string();
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Kind::SameAssetData { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![Kind::SameAssetData {
                kind: NameKind::Image,
                names: vec!["flash_a".to_string(), "flash_b".to_string()],
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "2 images hold the same data: \"flash_a\", \"flash_b\""
        );
    }

    #[test]
    fn a_large_screenshot_is_a_suggestion() {
        let mut vpx = clean_vpx();
        let mut screenshot = vec![0u8; 2 * 1024 * 1024];
        screenshot[..4].copy_from_slice(&[0x89, b'P', b'N', b'G']);
        vpx.info.screenshot = Some(screenshot);
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::LargeScreenshot {
                bytes: 2 * 1024 * 1024,
                png: true,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
    }

    /// An encoded image with the given bytes under the given file name
    fn stored_image(name: &str, extension: &str, data: Vec<u8>) -> crate::vpx::image::ImageData {
        use crate::vpx::pinbinary::PinBinary;
        crate::vpx::image::ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.{extension}"),
            width: 40,
            height: 20,
            jpeg: Some(PinBinary {
                path: format!("C:\\images\\{name}.{extension}"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            ..Default::default()
        }
    }

    fn image_content_findings(vpx: &VPX) -> Vec<Kind> {
        audit_kinds(vpx)
            .into_iter()
            .filter(|finding| {
                matches!(
                    finding,
                    Kind::ImageExtensionMismatch { .. }
                        | Kind::ImageFormatUnknown { .. }
                        | Kind::UnreadableImage { .. }
                )
            })
            .collect()
    }

    #[test]
    fn an_image_named_for_another_format_is_informational() -> TestResult {
        use crate::vpx::images::tests::{encoded_image, tga_image};
        let mut vpx = clean_vpx();
        // a jpeg under a png name, a footered tga under a png name, and
        // a png under a name that is no image format at all
        let mut jpeg = encoded_image("photo", "jpg", 40, 20)?;
        jpeg.path = "C:\\images\\photo.png".to_string();
        vpx.images.push(jpeg);
        vpx.images.push(tga_image("art", "png", 40, 20, true)?);
        let mut png = encoded_image("blob", "png", 40, 20)?;
        png.path = "C:\\images\\blob.dat".to_string();
        vpx.images.push(png);
        // a jpeg under its other extensions is fine
        let mut jfif = encoded_image("rust", "jpg", 40, 20)?;
        jfif.path = "C:\\images\\rust.jfif".to_string();
        vpx.images.push(jfif);
        vpx.images.push(encoded_image("plain", "jpeg", 40, 20)?);

        let findings = image_content_findings(&vpx);

        assert_eq!(
            findings,
            vec![
                Kind::ImageExtensionMismatch {
                    image: "photo".to_string(),
                    extension: "png".to_string(),
                    format: "jpeg",
                },
                Kind::ImageExtensionMismatch {
                    image: "art".to_string(),
                    extension: "png".to_string(),
                    format: "tga",
                },
                Kind::ImageExtensionMismatch {
                    image: "blob".to_string(),
                    extension: "dat".to_string(),
                    format: "png",
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "image \"photo\" is a jpeg file stored under a .png name; vpinball reads the content, a tool trusting the name gets the format wrong"
        );
        Ok(())
    }

    #[test]
    fn an_image_nothing_can_identify_is_an_error() -> TestResult {
        use crate::vpx::images::tests::tga_image;
        let mut vpx = clean_vpx();
        // bytes with no signature under a png name, and the same under a
        // tga name, which is the one name that still reads them
        vpx.images
            .push(stored_image("noise", "png", b"AAAAAAAA".to_vec()));
        vpx.images
            .push(stored_image("noisy", "tga", b"AAAAAAAA".to_vec()));
        // a footerless tga under its own name reads fine
        vpx.images.push(tga_image("old", "tga", 40, 20, false)?);

        let findings = image_content_findings(&vpx);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert_eq!(
            findings[0],
            Kind::ImageFormatUnknown {
                image: "noise".to_string(),
                extension: "png".to_string(),
            }
        );
        assert_eq!(findings[0].severity(), Severity::Error);
        assert_eq!(
            findings[0].to_string(),
            "image \"noise\" has no known image signature and its .png name is no help; vpinball cannot identify it and does not load it"
        );
        assert!(
            matches!(
                &findings[1],
                Kind::UnreadableImage { image, format: "tga", error: Some(_) } if image == "noisy"
            ),
            "{findings:#?}"
        );
        assert_eq!(findings[1].severity(), Severity::Warning);
        Ok(())
    }

    #[test]
    fn an_image_vpin_cannot_read_is_reported() {
        let mut vpx = clean_vpx();
        // a Photoshop file under a png name: vpinball reads it, vpin does not
        let mut psd = b"8BPS\0\x01".to_vec();
        psd.resize(64, 0);
        vpx.images.push(stored_image("layered", "png", psd));
        // a png whose header is cut short
        vpx.images.push(stored_image(
            "cut",
            "png",
            b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec(),
        ));

        let findings = image_content_findings(&vpx);

        assert_eq!(findings.len(), 3, "{findings:#?}");
        assert_eq!(
            findings[0],
            Kind::ImageExtensionMismatch {
                image: "layered".to_string(),
                extension: "png".to_string(),
                format: "psd",
            }
        );
        assert_eq!(
            findings[1],
            Kind::UnreadableImage {
                image: "layered".to_string(),
                format: "psd",
                error: None,
            }
        );
        assert_eq!(findings[1].severity(), Severity::Suggestion);
        assert_eq!(
            findings[1].to_string(),
            "image \"layered\" is a psd file, which vpinball reads but most other tools do not; consider re-saving it as png or webp"
        );
        assert!(
            matches!(
                &findings[2],
                Kind::UnreadableImage { image, format: "png", error: Some(_) } if image == "cut"
            ),
            "{findings:#?}"
        );
        assert_eq!(findings[2].severity(), Severity::Warning);
        assert!(
            findings[2]
                .to_string()
                .starts_with("image \"cut\" is a png file whose header does not parse ("),
            "{}",
            findings[2]
        );
    }

    #[test]
    fn stale_image_dimensions_are_informational() {
        use crate::vpx::image::ImageData;
        use crate::vpx::pinbinary::PinBinary;
        let mut png = Vec::new();
        ::image::RgbaImage::from_pixel(2, 3, ::image::Rgba([1, 2, 3, 255]))
            .write_to(
                &mut std::io::Cursor::new(&mut png),
                ::image::ImageFormat::Png,
            )
            .expect("encodes");
        let image = |name: &str, width: u32, height: u32| ImageData {
            name: name.to_string(),
            path: format!("{name}.png"),
            width,
            height,
            jpeg: Some(PinBinary {
                path: format!("{name}.png"),
                name: name.to_string(),
                internal_name: None,
                data: png.clone(),
            }),
            ..Default::default()
        };
        let mut vpx = clean_vpx();
        vpx.images = vec![image("right", 2, 3), image("resized", 8, 8)];
        vpx.gamedata.images_size = 2;
        vpx.gamedata.image = "right".to_string();
        vpx.gamedata.ball_image = "resized".to_string();
        let findings = audit_kinds(&vpx)
            .into_iter()
            // the fixtures reuse one payload for several names
            .filter(|finding| !matches!(finding, Kind::SameAssetData { .. }))
            .collect::<Vec<_>>();
        assert_eq!(
            findings,
            vec![Kind::ImageDimensionMismatch {
                image: "resized".to_string(),
                stored: (8, 8),
                actual: (2, 3),
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
    }

    #[test]
    fn an_unused_embedded_font_is_reported() {
        use crate::vpx::gameitem::font::Font;
        use crate::vpx::gameitem::textbox::TextBox;
        use crate::vpx::pinbinary::PinBinary;
        use crate::vpx::ttf::font_with_names;
        let font_data = |name: &str, family: &str, full: &str| PinBinary {
            name: name.to_string(),
            internal_name: None,
            path: format!("{name}.ttf"),
            data: font_with_names(family, full),
        };
        let mut vpx = clean_vpx();
        vpx.fonts = vec![
            font_data("led", "Advanced LED Board-7", "Advanced LED Board-7"),
            font_data("script_only", "Emerald Beacon", "Emerald Beacon Italic"),
            font_data("unused", "Nobody Uses This", "Nobody Uses This"),
            PinBinary {
                name: "junk".to_string(),
                internal_name: None,
                path: "junk.ttf".to_string(),
                data: vec![1, 2, 3],
            },
        ];
        vpx.gamedata.fonts_size = 4;
        let textbox = TextBox {
            name: "Score".to_string(),
            font: Font::new(
                0,
                Default::default(),
                400,
                120000,
                "advanced led board-7".to_string(),
            ),
            ..TextBox::default()
        };
        vpx.gameitems.push(GameItemEnum::TextBox(textbox));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        vpx.gamedata.set_code(
            "Option Explicit\r\n' FlexDMD.NewFont(\"Emerald Beacon Italic\", 1)\r\n".to_string(),
        );

        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::UnusedFont {
                font: "unused".to_string(),
                faces: vec!["Nobody Uses This".to_string()],
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "font \"unused\" (\"Nobody Uses This\") is not used by any textbox or decal and the script does not mention it"
        );
    }

    #[test]
    fn a_dmd_textbox_never_draws_its_font() {
        use crate::vpx::gameitem::font::Font;
        use crate::vpx::gameitem::textbox::TextBox;
        let textbox = |name: &str, text: &str, is_dmd: Option<bool>| {
            GameItemEnum::TextBox(TextBox {
                name: name.to_string(),
                text: text.to_string(),
                is_dmd,
                font: Font::new(
                    0,
                    Default::default(),
                    700,
                    180000,
                    "Lucida Sans Unicode".to_string(),
                ),
                ..TextBox::default()
            })
        };
        let mut vpx = clean_vpx();
        vpx.collections.clear();
        vpx.gamedata.collections_size = 0;
        vpx.gameitems = vec![
            textbox("ScoreText", "VISUAL PINBALL", Some(true)),
            textbox("Legacy", "DMD", None),
            textbox("Credits", "CREDITS 0", None),
        ];
        vpx.gamedata.gameitems_size = 3;
        let fonts: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|f| matches!(f, Kind::NonStandardFont { .. }))
            .collect();
        assert_eq!(
            fonts,
            vec![Kind::NonStandardFont {
                item: "TextBox \"Credits\"".to_string(),
                font: "Lucida Sans Unicode".to_string(),
            }]
        );
    }

    #[test]
    fn a_font_that_is_neither_embedded_nor_standard_is_reported() {
        use crate::vpx::gameitem::font::Font;
        use crate::vpx::gameitem::textbox::TextBox;
        use crate::vpx::pinbinary::PinBinary;
        use crate::vpx::ttf::font_with_names;
        let textbox = |name: &str, font: &str| {
            GameItemEnum::TextBox(TextBox {
                name: name.to_string(),
                font: Font::new(0, Default::default(), 400, 120000, font.to_string()),
                ..TextBox::default()
            })
        };
        let mut vpx = clean_vpx();
        vpx.fonts = vec![PinBinary {
            name: "led".to_string(),
            internal_name: None,
            path: "led.ttf".to_string(),
            data: font_with_names("Digital Readout", "Digital Readout Upright"),
        }];
        vpx.gamedata.fonts_size = 1;
        vpx.collections.clear();
        vpx.gamedata.collections_size = 0;
        vpx.gameitems = vec![
            textbox("Score", "Arial Black"),
            textbox("Vertical", "@Arial Narrow"),
            textbox("Embedded", "digital readout upright"),
            textbox("Windows", "Segoe UI"),
            textbox("Custom", "Bebas Neue"),
        ];
        vpx.gamedata.gameitems_size = 5;
        // the template's materials were only used by the items replaced here
        vpx.gamedata.materials_old.clear();
        vpx.gamedata.materials_physics_old = None;
        vpx.gamedata.materials_size = 0;
        vpx.gamedata.playfield_material.clear();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::NonStandardFont {
                    item: "TextBox \"Windows\"".to_string(),
                    font: "Segoe UI".to_string(),
                },
                Kind::NonStandardFont {
                    item: "TextBox \"Custom\"".to_string(),
                    font: "Bebas Neue".to_string(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
    }

    #[test]
    fn script_literals_skip_comments_and_see_concatenations() {
        let script = "PlaySound \"Fx_Hit\", 1 ' \"commented\"\r\n\
            x = \"say \"\"hi\"\"\" & \"VPX.Logo&dmd=2\"\r\n\
            PlaySound \"fx_ballrolling\" & i\r\n\
            Light1.Image = \"LM\" & _\r\n    color & \"On\"\r\n";
        let literals = script_literals(script);
        let literal = |text: &str, before: bool, after: bool| Literal {
            text: text.to_string(),
            joined_before: before,
            joined_after: after,
        };
        assert_eq!(
            literals,
            vec![
                literal("fx_hit", false, false),
                literal("say \"hi\"", false, true),
                literal("vpx.logo&dmd=2", true, false),
                literal("fx_ballrolling", false, true),
                literal("lm", false, true),
                literal("on", true, false),
            ]
        );
        assert!(script_names(&literals, "FX_HIT"));
        assert!(script_names(&literals, "fx_ballrolling12"));
        assert!(!script_names(&literals, "fx_hit_loud"));
        assert!(script_names(&literals, "LMRedOn"));
        assert!(script_names(&literals, "logo"));
        assert!(!script_names(&literals, "commented"));

        // a continuation line joining a literal that ended the previous one
        let literals = script_literals("x = \"fx_\" _\r\n    & name\r\ny = \"lone\"\r\n");
        assert!(literals[0].joined_after);
        assert!(!literals[1].joined_after);
    }

    #[test]
    fn sound_calls_are_found() {
        let script = "PlaySound \"fx_a\", 1\r\n\
            PlaySoundAt(\"fx_b\", Bumper1)\r\n\
            PlaySoundAtLevelStatic \"fx_c\", 0.5, Wall1 ' PlaySound \"commented\"\r\n\
            x = \"PlaySound \"\"in_string\"\"\"\r\n\
            MyPlaySound \"prefixed\"\r\n\
            PlaySound SoundFX(\"fx_d\", DOFContactors)\r\n\
            PlaySoundAtLevelStatic SoundFXDOF(\"fx_e\", 105, DOFPulse, DOFContactors), 1, Kicker1\r\n\
            StopSound \"fx_f\"\r\n\
            PlaySound \"fx_ballrolling\" & i\r\n";
        assert_eq!(
            sound_call_literals(script),
            vec![
                ("fx_a".to_string(), false),
                ("fx_b".to_string(), false),
                ("fx_c".to_string(), false),
                ("fx_d".to_string(), false),
                ("fx_e".to_string(), false),
                ("fx_f".to_string(), false),
                ("fx_ballrolling".to_string(), true)
            ]
        );
    }

    #[test]
    fn a_missing_sound_is_reported_once() {
        use crate::vpx::sound::{OutputTarget, SoundData};
        let mut vpx = clean_vpx();
        vpx.sounds = vec![SoundData {
            name: "fx_hit".to_string(),
            path: "fx_hit.wav".to_string(),
            wave_form: Default::default(),
            data: Vec::new(),
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        }];
        vpx.gamedata.sounds_size = 1;
        vpx.gamedata.set_code(
            "Option Explicit\r\nPlaySound \"FX_HIT\"\r\nPlaySound \"fx_gone\"\r\nPlaySoundAt \"fx_gone\", Bumper1\r\nPlaySound \"fx_h\" & i\r\nPlaySound \"fx_roll\" & i\r\n"
                .to_string(),
        );
        assert_eq!(
            audit_kinds(&vpx),
            vec![
                Kind::MissingSound {
                    sound: "fx_gone".to_string(),
                },
                Kind::MissingSound {
                    sound: "fx_roll".to_string(),
                },
            ]
        );
    }

    #[test]
    fn markdown_images_are_found() {
        assert_eq!(
            markdown_images(
                "# Rules\n![the playfield](Playfield_Rules) and ![](Logo)\n[a link](http://x)"
            ),
            vec!["playfield_rules".to_string(), "logo".to_string()]
        );
    }

    #[test]
    fn unused_assets_are_reported() {
        use crate::vpx::image::ImageData;
        use crate::vpx::sound::{OutputTarget, SoundData};
        let image = |name: &str| ImageData {
            name: name.to_string(),
            path: format!("{name}.png"),
            ..Default::default()
        };
        let sound = |name: &str| SoundData {
            name: name.to_string(),
            path: format!("{name}.wav"),
            wave_form: Default::default(),
            data: vec![0; 2048],
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        };
        let mut vpx = clean_vpx();
        vpx.images = vec![
            image("playfield"),
            image("scripted"),
            image("dmd_logo"),
            image("rules_sheet"),
            image("orphan"),
        ];
        vpx.gamedata.images_size = 5;
        vpx.gamedata.image = "Playfield".to_string();
        vpx.info.table_rules = Some("![rules](Rules_Sheet)".to_string());
        vpx.sounds = vec![sound("fx_hit"), sound("fx_unused")];
        vpx.gamedata.sounds_size = 2;
        let material = |name: &str| {
            let mut material = crate::vpx::material::Material::default();
            material.name = name.to_string();
            material
        };
        vpx.gamedata.materials = Some(vec![
            material("Playfield"),
            material("ByScript"),
            material("Forgotten"),
            material("Spare"),
        ]);
        vpx.gamedata.playfield_material = "playfield".to_string();
        vpx.gamedata.set_code(
            "Option Explicit\r\nDim img\r\nWall1.Image = \"Scripted\"\r\nPlaySound \"FX_Hit\"\r\nSet img = FlexDMD.NewImage(\"l\", \"VPX.dmd_logo&dmd=2\")\r\nWall1.Material = \"byscript\"\r\n"
                .to_string(),
        );

        let findings = audit_kinds(&vpx)
            .into_iter()
            // the fixtures reuse one payload for several names
            .filter(|finding| !matches!(finding, Kind::SameAssetData { .. }))
            .collect::<Vec<_>>();
        assert_eq!(
            findings,
            vec![
                Kind::UnusedImage {
                    image: "orphan".to_string(),
                    bytes: 0,
                },
                Kind::UnusedSound {
                    sound: "fx_unused".to_string(),
                    bytes: 2048,
                },
                Kind::UnusedMaterials {
                    names: vec!["Forgotten".to_string(), "Spare".to_string()],
                    total: 4,
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(findings[2].severity(), Severity::Info);
        assert_eq!(
            findings[2].to_string(),
            "2 of 4 materials are not used by any item or the playfield and the script does not name them: \"Forgotten\", \"Spare\""
        );
        assert_eq!(
            findings[1].to_string(),
            "sound \"fx_unused\" (2 KB) is not named in the script"
        );
    }

    fn stored_sound(name: &str, path: &str, data: Vec<u8>) -> crate::vpx::sound::SoundData {
        crate::vpx::sound::SoundData {
            name: name.to_string(),
            path: path.to_string(),
            data,
            wave_form: Default::default(),
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: crate::vpx::sound::OutputTarget::Table,
        }
    }

    fn sound_storage_findings(vpx: &VPX) -> Vec<Kind> {
        audit_kinds(vpx)
            .into_iter()
            .filter(|finding| {
                matches!(
                    finding,
                    Kind::SoundExtensionMismatch { .. } | Kind::SoundFormatUnknown { .. }
                )
            })
            .collect()
    }

    #[test]
    fn a_sound_named_for_another_format_is_informational() {
        let mut vpx = clean_vpx();
        let wav = b"RIFF\x24\0\0\0WAVEfmt ".to_vec();
        // a wav file and an ogg under mp3 names
        vpx.sounds
            .push(stored_sound("toy", "C:\\sounds\\toy.mp3", wav));
        vpx.sounds.push(stored_sound(
            "music",
            "C:\\sounds\\music.MP3",
            b"OggS\0\x02".to_vec(),
        ));
        // an mp3 by its ID3 tag, one by its first frame, an ogg and a
        // flac under their own names are fine, whatever the case
        vpx.sounds
            .push(stored_sound("tagged", "tagged.mp3", b"ID3\x04\0".to_vec()));
        vpx.sounds.push(stored_sound(
            "bare",
            "bare.Mp3",
            b"\xFF\xFB\x90\x64".to_vec(),
        ));
        vpx.sounds
            .push(stored_sound("vorbis", "vorbis.ogg", b"OggS\0\x02".to_vec()));
        vpx.sounds.push(stored_sound(
            "lossless",
            "lossless.flac",
            b"fLaC\0\0".to_vec(),
        ));
        // a wav name is stored as header plus samples, never a file
        vpx.sounds
            .push(stored_sound("hit", "hit.wav", vec![0xFF, 0xFF, 0, 0]));

        let findings = sound_storage_findings(&vpx);

        assert_eq!(
            findings,
            vec![
                Kind::SoundExtensionMismatch {
                    sound: "toy".to_string(),
                    extension: "mp3".to_string(),
                    format: "wav",
                },
                Kind::SoundExtensionMismatch {
                    sound: "music".to_string(),
                    extension: "MP3".to_string(),
                    format: "ogg",
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "sound \"toy\" is a wav file stored under a .mp3 name; vpinball's decoder reads the content, a tool trusting the name gets the format wrong"
        );
    }

    #[test]
    fn a_sound_the_decoder_cannot_identify_is_an_error() {
        let mut vpx = clean_vpx();
        // bytes without signature under an mp3 name, and a path with no
        // extension at all, which vpinball also hands to the decoder as is
        vpx.sounds
            .push(stored_sound("noise", "noise.mp3", b"AAAAAAAA".to_vec()));
        vpx.sounds.push(stored_sound(
            "bell",
            "* Backglass Output *",
            b"RIFF\x24\0\0\0WAVEfmt ".to_vec(),
        ));

        let findings = sound_storage_findings(&vpx);

        assert_eq!(
            findings,
            vec![
                Kind::SoundFormatUnknown {
                    sound: "noise".to_string(),
                    extension: "mp3".to_string(),
                },
                Kind::SoundFormatUnknown {
                    sound: "bell".to_string(),
                    extension: String::new(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Error);
        assert_eq!(
            findings[0].to_string(),
            "sound \"noise\" has no known audio signature and its .mp3 name is no help; vpinball's decoder cannot identify it, so the sound never plays"
        );
        assert_eq!(
            findings[1].to_string(),
            "sound \"bell\" has a path without extension; vpinball hands the stored bytes to its decoder as a file, which cannot identify them, so the sound never plays"
        );
    }
}
