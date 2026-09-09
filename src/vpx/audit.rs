//! Consistency checks for a table.
//!
//! [`audit`] reports problems in an in-memory [`VPX`] that vpinball would
//! tolerate at runtime but that usually mean something went wrong while
//! producing the table: references to images, materials, surfaces, part
//! groups or collection items that do not exist, duplicate or over-long
//! names, and a few storage level suggestions.
//!
//! Because the checks run on the model they apply to every way a table is
//! loaded: a vpx file, an expanded directory, or files assembled by an
//! editor. Every check is deterministic; nothing here parses the script or
//! applies heuristics.
//!
//! See <https://github.com/francisdb/vpin/issues/96> for the wider audit
//! wish list this is a part of.

use super::VPX;
use super::gameitem::GameItemEnum;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// The sentinel vpinball's editor writes for "no image selected"
const NONE_SELECTION: &str = "<None>";

/// vpinball limits object names to 32 characters
/// (<https://github.com/vpinball/vpinball/issues/1706>)
const MAX_NAME_LENGTH: usize = 32;

/// A single consistency problem found by [`audit`].
///
/// `item` names the game item carrying the problem, `field` the property
/// holding the dangling reference.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Finding {
    /// A game item or table setting references an image that does not exist
    MissingImage {
        item: String,
        field: &'static str,
        image: String,
    },
    /// A table setting references an image that does not exist, but
    /// vpinball renders with a built-in instead: the environment image
    /// falls back to its own environment map, the ball image to its
    /// built-in ball, a ball decal is left off. The reference is stale,
    /// the table plays as intended
    MissingImageWithFallback {
        field: &'static str,
        image: String,
        /// What vpinball uses instead
        fallback: &'static str,
    },
    /// The color grade image does not exist. vpinball looks it up in the
    /// table only and silently renders without color grading when it is
    /// missing, so this one changes the picture
    MissingColorGradeImage { image: String },
    /// A game item or table setting references a material that does not exist
    MissingMaterial {
        item: String,
        field: &'static str,
        material: String,
    },
    /// A game item is placed on a surface (wall or ramp) that does not exist
    MissingSurface { item: String, surface: String },
    /// A game item belongs to a part group that does not exist
    MissingPartGroup { item: String, part_group: String },
    /// A collection contains an item that does not exist
    MissingCollectionItem { collection: String, item: String },
    /// Two images, sounds, game items or collections share a name
    DuplicateName { kind: NameKind, name: String },
    /// A game item name is longer than vpinball supports
    NameTooLong { item: String, length: usize },
    /// The table info has no table name
    MissingTableName,
    /// An image is stored as an uncompressed era bitmap; vpinball suggests
    /// converting these to webp
    BmpImage { image: String },
    /// The color grade lookup table image is not the 256x16 layout the
    /// shader expects, which silently renders wrong colors
    ColorGradeLutUnusualSize {
        image: String,
        width: u32,
        height: u32,
    },
    /// The script mixes line ending styles. vpinball and the standalone
    /// VBScript engine accept any of them, but line based tooling (diffs of
    /// an extracted script, patchers, some editors) trips over a mix. The
    /// counts tell a stray line from a wholesale mix
    MixedScriptLineEndings { crlf: usize, lf: usize, cr: usize },
    /// An embedded font whose face names no textbox or decal uses and the
    /// script does not mention; it only adds to the file
    UnusedFont { font: String, faces: Vec<String> },
    /// The glass is below two inches or upside down
    GlassHeightInvalid { detail: &'static str },
    /// The legacy spherical ball mapping renders badly in VR, stereo and
    /// head tracked setups
    BallSphericalMapping,
    /// A legacy textbox is used for DMD rendering, a flasher renders better
    TextboxUsedForDmd { item: String },
    /// A timer fires faster than a 60 FPS frame, which causes stutters
    FastTimer { item: String, interval: i32 },
    /// A light has a negative intensity
    NegativeLightIntensity { item: String },
    /// A sound plays on the playfield speakers but is not mono
    StereoTableSound { sound: String },
    /// The embedded screenshot is large; it bloats the file and every save
    /// spends noticeable time hashing it into the integrity signature.
    /// Large ones are usually PNG captures, which JPEG stores much smaller.
    LargeScreenshot { bytes: usize, png: bool },
    /// The script could not be parsed, so the script-level checks could not run
    ScriptParseError { detail: String },
    /// The script has no `Option Explicit`, so a typo in a variable name
    /// silently creates a new variable instead of being caught
    MissingOptionExplicit,
    /// A sub or function is declared more than once; the later one wins and
    /// the earlier is dead
    DuplicateProcedure { name: String },
    /// The script uses `Execute`, which runs code built at runtime; vpinball
    /// warns this triggers security checks and can stutter. `ExecuteGlobal`
    /// is not flagged since tables normally use it to load scripts at startup
    ExecuteUsed,
    /// The script uses a VPinMAME controller but the table has no timer named
    /// `PinMAMETimer`, which it needs to drive the emulation
    MissingPinMameTimer,
    /// The script uses a VPinMAME controller but never calls `vpmInit`
    MissingVpmInit,
    /// The script uses `vpmTimer` but the table has no timer named `PulseTimer`
    MissingPulseTimer,
}

/// How serious a [`Finding`] is
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Severity {
    /// An improvement worth considering
    Suggestion,
    /// Something looks wrong, though vpinball tolerates it at runtime
    Warning,
    /// Something is wrong and will misbehave when played
    Error,
}

impl Finding {
    /// How serious this finding is
    pub fn severity(&self) -> Severity {
        match self {
            Finding::BmpImage { .. }
            | Finding::LargeScreenshot { .. }
            | Finding::MixedScriptLineEndings { .. }
            | Finding::MissingImageWithFallback { .. }
            | Finding::UnusedFont { .. } => Severity::Suggestion,
            Finding::NegativeLightIntensity { .. } | Finding::StereoTableSound { .. } => {
                Severity::Error
            }
            Finding::MissingOptionExplicit => Severity::Suggestion,
            _ => Severity::Warning,
        }
    }
}

/// What kind of name a [`Finding::DuplicateName`] is about
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Image,
    Sound,
    GameItem,
    Collection,
}

impl fmt::Display for NameKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameKind::Image => write!(f, "image"),
            NameKind::Sound => write!(f, "sound"),
            NameKind::GameItem => write!(f, "game item"),
            NameKind::Collection => write!(f, "collection"),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::MissingImage { item, field, image } => {
                write!(f, "{item}: {field} references missing image {image:?}")
            }
            Finding::MissingImageWithFallback {
                field,
                image,
                fallback,
            } => write!(
                f,
                "table settings: {field} references missing image {image:?}, vpinball uses {fallback}"
            ),
            Finding::MissingColorGradeImage { image } => write!(
                f,
                "table settings: color grade image {image:?} does not exist, color grading is silently off"
            ),
            Finding::MissingMaterial {
                item,
                field,
                material,
            } => write!(
                f,
                "{item}: {field} references missing material {material:?}"
            ),
            Finding::MissingSurface { item, surface } => {
                write!(f, "{item}: placed on missing surface {surface:?}")
            }
            Finding::MissingPartGroup { item, part_group } => {
                write!(f, "{item}: belongs to missing part group {part_group:?}")
            }
            Finding::MissingCollectionItem { collection, item } => {
                write!(
                    f,
                    "collection {collection:?} contains missing item {item:?}"
                )
            }
            Finding::DuplicateName { kind, name } => write!(f, "duplicate {kind} name {name:?}"),
            Finding::NameTooLong { item, length } => write!(
                f,
                "{item}: name is {length} characters, vpinball supports {MAX_NAME_LENGTH}"
            ),
            Finding::MissingTableName => write!(f, "table info has no table name"),
            Finding::BmpImage { image } => write!(
                f,
                "image {image:?} is stored as a bitmap, consider converting to webp"
            ),
            Finding::ColorGradeLutUnusualSize {
                image,
                width,
                height,
            } => write!(
                f,
                "color grade image {image:?} is {width}x{height}, the shader expects 256x16"
            ),
            Finding::MixedScriptLineEndings { crlf, lf, cr } => {
                let styles: Vec<String> = [(crlf, "CRLF"), (lf, "LF"), (cr, "CR")]
                    .into_iter()
                    .filter(|(count, _)| **count > 0)
                    .map(|(count, name)| format!("{count} {name}"))
                    .collect();
                write!(f, "script mixes line endings: {}", styles.join(", "))
            }
            Finding::UnusedFont { font, faces } => write!(
                f,
                "font {font:?} ({}) is not used by any textbox or decal and the script does not mention it",
                faces
                    .iter()
                    .map(|face| format!("{face:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Finding::GlassHeightInvalid { detail } => {
                write!(f, "glass height seems invalid: {detail}")
            }
            Finding::BallSphericalMapping => write!(
                f,
                "ball uses legacy spherical mapping, which renders badly in VR, stereo and head tracked setups"
            ),
            Finding::TextboxUsedForDmd { item } => write!(
                f,
                "{item}: legacy textbox used for DMD rendering, a flasher renders better"
            ),
            Finding::FastTimer { item, interval } => write!(
                f,
                "{item}: timer fires every {interval}ms, faster than a 60 FPS frame, which causes stutters"
            ),
            Finding::NegativeLightIntensity { item } => {
                write!(f, "{item}: negative light intensity")
            }
            Finding::StereoTableSound { sound } => write!(
                f,
                "sound {sound:?} plays on the playfield speakers but is not mono"
            ),
            Finding::LargeScreenshot { bytes, png } => {
                let advice = if *png {
                    "consider converting it to JPEG"
                } else {
                    "consider a smaller one"
                };
                write!(
                    f,
                    "embedded screenshot is {:.1} MB, {advice}",
                    *bytes as f64 / 1e6
                )
            }
            Finding::ScriptParseError { detail } => {
                write!(f, "script could not be parsed: {detail}")
            }
            Finding::MissingOptionExplicit => {
                write!(
                    f,
                    "script has no 'Option Explicit', typos create silent new variables"
                )
            }
            Finding::DuplicateProcedure { name } => {
                write!(f, "script declares {name:?} more than once")
            }
            Finding::ExecuteUsed => {
                write!(
                    f,
                    "script uses Execute, which runs runtime-built code and can stutter"
                )
            }
            Finding::MissingPinMameTimer => write!(
                f,
                "script uses a VPinMAME controller but the table has no timer named 'PinMAMETimer'"
            ),
            Finding::MissingVpmInit => {
                write!(
                    f,
                    "script uses a VPinMAME controller but never calls vpmInit"
                )
            }
            Finding::MissingPulseTimer => write!(
                f,
                "script uses 'vpmTimer' but the table has no timer named 'PulseTimer'"
            ),
        }
    }
}

/// Checks a table for consistency problems, returning an empty list when
/// nothing is wrong.
///
/// Name comparisons are case insensitive, like vpinball's own lookups.
pub fn audit(vpx: &VPX) -> Vec<Finding> {
    let mut findings = Vec::new();

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
    let item_names = name_set(vpx.gameitems.iter().map(|item| item.name()));

    check_table_settings(vpx, &images, &materials, &mut findings);
    for item in &vpx.gameitems {
        check_item_references(item, &images, &materials, &surfaces, &mut findings);
        check_part_group(item, &part_groups, &mut findings);
        let name = item.name();
        if name.chars().count() > MAX_NAME_LENGTH {
            findings.push(Finding::NameTooLong {
                item: item_label(item),
                length: name.chars().count(),
            });
        }
    }

    for collection in &vpx.collections {
        for item in &collection.items {
            if !item_names.contains(item.to_lowercase().as_str()) {
                findings.push(Finding::MissingCollectionItem {
                    collection: collection.name.clone(),
                    item: item.clone(),
                });
            }
        }
    }

    check_fonts(vpx, &mut findings);

    check_duplicates(
        vpx.images.iter().map(|image| image.name.as_str()),
        NameKind::Image,
        &mut findings,
    );
    check_duplicates(
        vpx.sounds.iter().map(|sound| sound.name.as_str()),
        NameKind::Sound,
        &mut findings,
    );
    check_duplicates(
        vpx.gameitems.iter().map(|item| item.name()),
        NameKind::GameItem,
        &mut findings,
    );
    check_duplicates(
        vpx.collections.iter().map(|c| c.name.as_str()),
        NameKind::Collection,
        &mut findings,
    );

    if vpx
        .info
        .table_name
        .as_ref()
        .is_none_or(|name| name.is_empty())
    {
        findings.push(Finding::MissingTableName);
    }

    for image in &vpx.images {
        if image.bits.is_some() {
            findings.push(Finding::BmpImage {
                image: image.name.clone(),
            });
        }
    }

    let script = &vpx.gamedata.code.string;
    let crlf = script.matches("\r\n").count();
    let lf = script.matches('\n').count() - crlf;
    let cr = script.matches('\r').count() - crlf;
    if [crlf, lf, cr].iter().filter(|count| **count > 0).count() > 1 {
        findings.push(Finding::MixedScriptLineEndings { crlf, lf, cr });
    }

    if let Some(screenshot) = &vpx.info.screenshot
        && screenshot.len() > LARGE_SCREENSHOT_BYTES
    {
        findings.push(Finding::LargeScreenshot {
            bytes: screenshot.len(),
            png: screenshot.starts_with(&[0x89, b'P', b'N', b'G']),
        });
    }

    check_render_settings(vpx, &mut findings);
    for item in &vpx.gameitems {
        check_item_behavior(item, &mut findings);
    }
    for sound in &vpx.sounds {
        if sound.output_target == crate::vpx::sound::OutputTarget::Table
            && sound.wave_form.channels > 1
        {
            findings.push(Finding::StereoTableSound {
                sound: sound.name.clone(),
            });
        }
    }

    #[cfg(feature = "script-audit")]
    script::check(vpx, &mut findings);

    findings
}

/// Screenshots above this size get a [`Finding::LargeScreenshot`]
const LARGE_SCREENSHOT_BYTES: usize = 1024 * 1024;

/// two inches in vp units (1 VPU = 0.53975 mm)
const TWO_INCHES_VPU: f32 = 2.0 * 25.4 / 0.539_75;

fn check_render_settings(vpx: &VPX, findings: &mut Vec<Finding>) {
    let gamedata = &vpx.gamedata;
    if let Some(bottom) = gamedata.glass_bottom_height
        && bottom > gamedata.glass_top_height
    {
        findings.push(Finding::GlassHeightInvalid {
            detail: "the bottom is higher than the top",
        });
    }
    if gamedata.glass_top_height < TWO_INCHES_VPU
        || gamedata
            .glass_bottom_height
            .is_some_and(|bottom| bottom < TWO_INCHES_VPU)
    {
        findings.push(Finding::GlassHeightInvalid {
            detail: "the glass is below two inches",
        });
    }
    // vpinball defaults to the legacy mapping when the field is absent,
    // which is the case for every table saved before 10.8
    if gamedata.ball_spherical_mapping.unwrap_or(true) {
        findings.push(Finding::BallSphericalMapping);
    }
}

fn check_item_behavior(item: &GameItemEnum, findings: &mut Vec<Finding>) {
    if let GameItemEnum::TextBox(textbox) = item
        && (textbox.is_dmd == Some(true) || textbox.text.to_uppercase().contains("DMD"))
    {
        findings.push(Finding::TextboxUsedForDmd {
            item: item_label(item),
        });
    }
    if let GameItemEnum::Light(light) = item
        && light.intensity < 0.0
    {
        findings.push(Finding::NegativeLightIntensity {
            item: item_label(item),
        });
    }
    if let Some(timer) = item.timer()
        && timer.is_enabled
        && timer.interval != -1
        && timer.interval != -2
        && timer.interval < 17
    {
        findings.push(Finding::FastTimer {
            item: item_label(item),
            interval: timer.interval,
        });
    }
}

fn name_set<'a>(names: impl Iterator<Item = &'a str>) -> HashSet<String> {
    names.map(|name| name.to_lowercase()).collect()
}

fn material_names(vpx: &VPX) -> HashSet<String> {
    let mut names: HashSet<String> = vpx
        .gamedata
        .materials_old
        .iter()
        .map(|material| material.name.to_lowercase())
        .collect();
    if let Some(materials) = &vpx.gamedata.materials {
        names.extend(
            materials
                .iter()
                .map(|material| material.name.to_lowercase()),
        );
    }
    names
}

fn check_duplicates<'a>(
    names: impl Iterator<Item = &'a str>,
    kind: NameKind,
    findings: &mut Vec<Finding>,
) {
    let mut seen: HashMap<String, &str> = HashMap::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_lowercase(), name).is_some() {
            findings.push(Finding::DuplicateName {
                kind,
                name: name.to_string(),
            });
        }
    }
}

/// An embedded font is registered by the names inside the font file, so
/// those are what a textbox or decal refers to. A font whose names do not
/// decode is left alone.
fn check_fonts(vpx: &VPX, findings: &mut Vec<Finding>) {
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
            findings.push(Finding::UnusedFont {
                font: font.name.clone(),
                faces,
            });
        }
    }
}

fn item_label(item: &GameItemEnum) -> String {
    format!("{} {:?}", item.type_name(), item.name())
}

struct References<'a> {
    images: Vec<(&'static str, &'a str)>,
    materials: Vec<(&'static str, &'a str)>,
    surface: Option<&'a str>,
}

fn check_table_settings(
    vpx: &VPX,
    images: &HashSet<String>,
    materials: &HashSet<String>,
    findings: &mut Vec<Finding>,
) {
    let gamedata = &vpx.gamedata;
    let missing = |image: &str| {
        !image.is_empty()
            && !image.eq_ignore_ascii_case(NONE_SELECTION)
            && !images.contains(image.to_lowercase().as_str())
    };
    let image_fields: [(&'static str, &str); 4] = [
        ("playfield image", &gamedata.image),
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
            findings.push(Finding::MissingImage {
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
            findings.push(Finding::MissingImageWithFallback {
                field,
                image: image.to_string(),
                fallback,
            });
        }
    }
    if missing(&gamedata.image_color_grade) {
        findings.push(Finding::MissingColorGradeImage {
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
        findings.push(Finding::ColorGradeLutUnusualSize {
            image: image.name.clone(),
            width: image.width,
            height: image.height,
        });
    }
    if !gamedata.playfield_material.is_empty()
        && !materials.contains(gamedata.playfield_material.to_lowercase().as_str())
    {
        findings.push(Finding::MissingMaterial {
            item: "table settings".to_string(),
            field: "playfield material",
            material: gamedata.playfield_material.clone(),
        });
    }
}

fn check_part_group(
    item: &GameItemEnum,
    part_groups: &HashSet<String>,
    findings: &mut Vec<Finding>,
) {
    if let Some(part_group) = item.part_group_name()
        && !part_group.is_empty()
        && !part_groups.contains(part_group.to_lowercase().as_str())
    {
        findings.push(Finding::MissingPartGroup {
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
    findings: &mut Vec<Finding>,
) {
    let refs = item_references(item);
    for (field, image) in &refs.images {
        if !image.is_empty()
            && !image.eq_ignore_ascii_case(NONE_SELECTION)
            && !images.contains(image.to_lowercase().as_str())
        {
            findings.push(Finding::MissingImage {
                item: item_label(item),
                field,
                image: (*image).to_string(),
            });
        }
    }
    for (field, material) in &refs.materials {
        if !material.is_empty() && !materials.contains(material.to_lowercase().as_str()) {
            findings.push(Finding::MissingMaterial {
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
        findings.push(Finding::MissingSurface {
            item: item_label(item),
            surface: surface.to_string(),
        });
    }
}

/// The image, material and surface references an item carries, straight
/// from the per type fields
fn item_references(item: &GameItemEnum) -> References<'_> {
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
    use crate::vpx;
    use pretty_assertions::assert_eq;

    fn blank_vpx() -> VPX {
        let bytes = include_bytes!("../../testdata/completely_blank_table_10_7_4.vpx");
        #[allow(clippy::unwrap_used)]
        vpx::from_bytes(bytes).unwrap()
    }

    /// The blank fixture with its dangling default references cleared
    fn clean_vpx() -> VPX {
        let mut vpx = blank_vpx();
        vpx.gamedata.image.clear();
        vpx.gamedata.image_color_grade.clear();
        vpx.gamedata.ball_image.clear();
        vpx.gamedata.ball_image_front.clear();
        vpx.gamedata.env_image = None;
        vpx.gamedata.ball_spherical_mapping = Some(false);
        vpx
    }

    #[test]
    fn a_clean_table_has_no_findings() {
        assert_eq!(audit(&clean_vpx()), Vec::new());
    }

    #[test]
    fn the_blank_template_has_dangling_default_references() {
        // vpinball's blank template keeps the default image names while the
        // images themselves are only present in the sample table
        let findings = audit(&blank_vpx());
        assert_eq!(findings.len(), 6, "{findings:#?}");
        let count = |wanted: fn(&Finding) -> bool| findings.iter().filter(|f| wanted(f)).count();
        // the playfield image is the only one without a fallback
        assert_eq!(count(|f| matches!(f, Finding::MissingImage { .. })), 1);
        assert_eq!(
            count(|f| matches!(f, Finding::MissingImageWithFallback { .. })),
            3
        );
        assert_eq!(
            count(|f| matches!(f, Finding::MissingColorGradeImage { .. })),
            1
        );
        // pre 10.8 tables implicitly use the legacy ball mapping
        assert!(findings.contains(&Finding::BallSphericalMapping));
    }

    #[test]
    fn a_missing_image_reference_is_reported() {
        let mut vpx = clean_vpx();
        vpx.gamedata.image = "no_such_image".to_string();
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::MissingImage {
                item: "table settings".to_string(),
                field: "playfield image",
                image: "no_such_image".to_string(),
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
    }

    #[test]
    fn a_missing_image_with_a_built_in_fallback_is_a_suggestion() {
        let mut vpx = clean_vpx();
        vpx.gamedata.ball_image = "no_such_image".to_string();
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::MissingImageWithFallback {
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
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::MissingColorGradeImage {
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
        assert_eq!(audit(&vpx), Vec::new());
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
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::MissingCollectionItem {
                collection: "Bumpers".to_string(),
                item: "Bumper1".to_string(),
            }]
        );
    }

    #[test]
    fn duplicate_names_are_reported_case_insensitively() {
        let mut vpx = clean_vpx();
        for name in ["ding", "DING"] {
            vpx.images.push(crate::vpx::image::ImageData {
                name: name.to_string(),
                ..Default::default()
            });
        }
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::DuplicateName {
                kind: NameKind::Image,
                name: "DING".to_string(),
            }]
        );
    }

    #[test]
    fn a_large_screenshot_is_a_suggestion() {
        let mut vpx = clean_vpx();
        let mut screenshot = vec![0u8; 2 * 1024 * 1024];
        screenshot[..4].copy_from_slice(&[0x89, b'P', b'N', b'G']);
        vpx.info.screenshot = Some(screenshot);
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::LargeScreenshot {
                bytes: 2 * 1024 * 1024,
                png: true,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
    }

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
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::ColorGradeLutUnusualSize {
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
        let findings = audit(&vpx);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(matches!(
            &findings[0],
            Finding::FastTimer { interval: 5, .. }
        ));
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
        let findings = audit(&vpx);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].severity(), Severity::Error);
    }

    #[test]
    fn an_upside_down_glass_is_reported() {
        let mut vpx = clean_vpx();
        vpx.gamedata.glass_top_height = 200.0;
        vpx.gamedata.glass_bottom_height = Some(300.0);
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::GlassHeightInvalid {
                detail: "the bottom is higher than the top",
            }]
        );
    }

    #[test]
    fn mixed_script_line_endings_are_reported() {
        let mut vpx = clean_vpx();
        // valid VBScript with Option Explicit so only the line-ending check
        // fires, with a bare LF and a bare CR mixed into the CRLF endings
        vpx.gamedata.code.string = "Option Explicit\r\nDim x\nDim y\rDim z\r\n".to_string();
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::MixedScriptLineEndings {
                crlf: 2,
                lf: 1,
                cr: 1
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "script mixes line endings: 2 CRLF, 1 LF, 1 CR"
        );
    }

    #[test]
    fn consistent_lf_line_endings_are_fine() {
        let mut vpx = clean_vpx();
        vpx.gamedata.code.string = "Option Explicit\nDim x\nDim y\n".to_string();
        assert_eq!(audit(&vpx), vec![]);
    }

    #[test]
    fn an_unused_embedded_font_is_reported() {
        use crate::vpx::font::FontData;
        use crate::vpx::gameitem::font::Font;
        use crate::vpx::gameitem::textbox::TextBox;
        use crate::vpx::ttf::font_with_names;
        let font_data = |name: &str, family: &str, full: &str| FontData {
            name: name.to_string(),
            path: format!("{name}.ttf"),
            data: font_with_names(family, full),
        };
        let mut vpx = clean_vpx();
        vpx.fonts = vec![
            font_data("led", "Advanced LED Board-7", "Advanced LED Board-7"),
            font_data("script_only", "Emerald Beacon", "Emerald Beacon Italic"),
            font_data("unused", "Nobody Uses This", "Nobody Uses This"),
            FontData {
                name: "junk".to_string(),
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

        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::UnusedFont {
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

    #[cfg(feature = "script-audit")]
    mod script {
        use super::*;
        use pretty_assertions::assert_eq;

        /// like clean_vpx but with a known good, CRLF, Option Explicit script
        fn scripted(body: &str) -> VPX {
            let mut vpx = clean_vpx();
            let script = format!("Option Explicit\r\n{}", body.replace('\n', "\r\n"));
            vpx.gamedata.code.string = script;
            vpx
        }

        fn script_findings(vpx: &VPX) -> Vec<Finding> {
            audit(vpx)
                .into_iter()
                .filter(|f| {
                    matches!(
                        f,
                        Finding::ScriptParseError { .. }
                            | Finding::MissingOptionExplicit
                            | Finding::DuplicateProcedure { .. }
                            | Finding::ExecuteUsed
                            | Finding::MissingPinMameTimer
                            | Finding::MissingVpmInit
                            | Finding::MissingPulseTimer
                    )
                })
                .collect()
        }

        #[test]
        fn a_clean_script_has_no_script_findings() {
            let vpx = scripted("Sub Foo()\nEnd Sub\n");
            assert_eq!(script_findings(&vpx), Vec::new());
        }

        #[test]
        fn a_missing_option_explicit_is_a_suggestion() {
            let mut vpx = clean_vpx();
            vpx.gamedata.code.string = "Sub Foo()\r\nEnd Sub\r\n".to_string();
            let findings = script_findings(&vpx);
            assert_eq!(findings, vec![Finding::MissingOptionExplicit]);
            assert_eq!(findings[0].severity(), Severity::Suggestion);
        }

        #[test]
        fn a_duplicate_procedure_is_reported() {
            let vpx = scripted("Sub Foo()\nEnd Sub\nSub Foo()\nEnd Sub\n");
            assert_eq!(
                script_findings(&vpx),
                vec![Finding::DuplicateProcedure {
                    name: "Foo".to_string()
                }]
            );
        }

        #[test]
        fn a_method_name_reused_across_classes_is_not_a_duplicate() {
            let vpx = scripted(
                "Class A\nPublic Sub Init()\nEnd Sub\nEnd Class\n                 Class B\nPublic Sub Init()\nEnd Sub\nEnd Class\n",
            );
            assert_eq!(script_findings(&vpx), Vec::new());
        }

        #[test]
        fn execute_is_flagged_but_execute_global_is_not() {
            let executed = scripted("Execute \"x = 1\"\n");
            assert_eq!(script_findings(&executed), vec![Finding::ExecuteUsed]);
            let global = scripted("ExecuteGlobal \"x = 1\"\n");
            assert_eq!(script_findings(&global), Vec::new());
        }

        #[test]
        fn a_broken_script_reports_a_parse_error() {
            let mut vpx = clean_vpx();
            vpx.gamedata.code.string = "Sub Foo(\r\n".to_string();
            let findings = script_findings(&vpx);
            assert_eq!(findings.len(), 1, "{findings:#?}");
            assert!(matches!(findings[0], Finding::ScriptParseError { .. }));
        }
    }
}

#[cfg(feature = "script-audit")]
mod script {
    use super::{Finding, VPX};
    use crate::vpx::gameitem::GameItemEnum;
    use std::collections::HashSet;
    use vbscript::parser::Parser;
    use vbscript::parser::ast::{Expr, FullIdent, Item, Stmt};

    /// What one pass over the script collected
    #[derive(Default)]
    struct Scan {
        option_explicit: bool,
        /// every identifier seen, lowercased, like vpinball's audit bag
        identifiers: HashSet<String>,
        /// declared sub/function names, qualified by class so that a method
        /// name reused across classes is not a duplicate
        declared: Vec<String>,
        /// the class currently being scanned, if any
        current_class: Option<String>,
    }

    pub(super) fn check(vpx: &VPX, findings: &mut Vec<Finding>) {
        let script = &vpx.gamedata.code.string;
        if script.trim().is_empty() {
            return;
        }
        let items = match Parser::new(script).file() {
            Ok(items) => items,
            Err(e) => {
                findings.push(Finding::ScriptParseError {
                    detail: format!("{e:?}"),
                });
                return;
            }
        };

        let mut scan = Scan::default();
        scan.items(&items);

        if !scan.option_explicit {
            findings.push(Finding::MissingOptionExplicit);
        }

        let mut seen: HashSet<String> = HashSet::new();
        for name in &scan.declared {
            if !seen.insert(name.to_lowercase()) {
                findings.push(Finding::DuplicateProcedure { name: name.clone() });
            }
        }

        // only bare Execute, like vpinball: it evaluates runtime-built code
        // and can stutter in game logic. ExecuteGlobal is normal at load time
        // (tables inject their controller and backglass scripts with it), so
        // flagging it would be noise
        if scan.identifiers.contains("execute") {
            findings.push(Finding::ExecuteUsed);
        }

        let timers: HashSet<String> = vpx
            .gameitems
            .iter()
            .filter_map(|item| match item {
                GameItemEnum::Timer(timer) => Some(timer.name.to_lowercase()),
                _ => None,
            })
            .collect();

        let uses_vpm =
            scan.identifiers.contains("loadvpm") || scan.identifiers.contains("loadvpmalt");
        if uses_vpm {
            if !timers.contains("pinmametimer") {
                findings.push(Finding::MissingPinMameTimer);
            }
            if !scan.identifiers.contains("vpminit") {
                findings.push(Finding::MissingVpmInit);
            }
        }
        if scan.identifiers.contains("vpmtimer") && !timers.contains("pulsetimer") {
            findings.push(Finding::MissingPulseTimer);
        }
    }

    impl Scan {
        fn items(&mut self, items: &[Item]) {
            for item in items {
                match item {
                    Item::OptionExplicit => self.option_explicit = true,
                    Item::Class { name, methods, .. } => {
                        self.current_class = Some(name.clone());
                        self.stmts(methods);
                        self.current_class = None;
                    }
                    Item::Statement(stmt) => self.stmt(stmt),
                    Item::Const { .. } | Item::Variable { .. } => {}
                }
            }
        }

        fn stmts(&mut self, stmts: &[Stmt]) {
            for stmt in stmts {
                self.stmt(stmt);
            }
        }

        fn stmt(&mut self, stmt: &Stmt) {
            match stmt {
                Stmt::Sub { name, body, .. } | Stmt::Function { name, body, .. } => {
                    let qualified = match &self.current_class {
                        Some(class) => format!("{class}.{name}"),
                        None => name.clone(),
                    };
                    self.declared.push(qualified);
                    self.stmts(body);
                }
                Stmt::Assignment { full_ident, value } => {
                    self.full_ident(full_ident);
                    self.expr(value);
                }
                Stmt::Set { var, rhs } => {
                    self.full_ident(var);
                    if let vbscript::parser::ast::SetRhs::Expr(e) = rhs {
                        self.expr(e);
                    }
                }
                Stmt::SubCall { fn_name, args } => {
                    self.full_ident(fn_name);
                    self.args(args);
                }
                Stmt::Call(fi) => self.full_ident(fi),
                Stmt::IfStmt {
                    condition,
                    body,
                    elseif_statements,
                    else_stmt,
                } => {
                    self.expr(condition);
                    self.stmts(body);
                    for (cond, block) in elseif_statements {
                        self.expr(cond);
                        self.stmts(block);
                    }
                    if let Some(block) = else_stmt {
                        self.stmts(block);
                    }
                }
                Stmt::WhileStmt { condition, body } => {
                    self.expr(condition);
                    self.stmts(body);
                }
                Stmt::ForStmt {
                    start,
                    end,
                    step,
                    body,
                    ..
                } => {
                    self.expr(start);
                    self.expr(end);
                    if let Some(step) = step {
                        self.expr(step);
                    }
                    self.stmts(body);
                }
                Stmt::ForEachStmt { group, body, .. } => {
                    self.expr(group);
                    self.stmts(body);
                }
                Stmt::DoLoop { body, .. } => self.stmts(body),
                Stmt::SelectCase {
                    test_expr,
                    cases,
                    else_stmt,
                } => {
                    self.expr(test_expr);
                    for case in cases {
                        self.stmts(&case.body);
                    }
                    if let Some(block) = else_stmt {
                        self.stmts(block);
                    }
                }
                Stmt::With { object, body } => {
                    self.full_ident(object);
                    self.stmts(body);
                }
                _ => {}
            }
        }

        fn args(&mut self, args: &[Option<Expr>]) {
            for arg in args.iter().flatten() {
                self.expr(arg);
            }
        }

        fn full_ident(&mut self, fi: &FullIdent) {
            self.expr(&fi.0);
        }

        fn expr(&mut self, expr: &Expr) {
            match expr {
                Expr::Ident(name) => {
                    self.identifiers.insert(name.to_lowercase());
                }
                Expr::New(name) => {
                    self.identifiers.insert(name.to_lowercase());
                }
                Expr::MemberExpression { base, property } => {
                    self.expr(base);
                    self.identifiers.insert(property.to_lowercase());
                }
                Expr::FnApplication { callee, args } => {
                    self.expr(callee);
                    self.args(args);
                }
                Expr::PrefixOp { expr, .. } => self.expr(expr),
                Expr::InfixOp { lhs, rhs, .. } => {
                    self.expr(lhs);
                    self.expr(rhs);
                }
                Expr::Literal(_) | Expr::WithScoped => {}
            }
        }
    }
}
