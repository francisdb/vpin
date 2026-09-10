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
    /// Several images, sounds, game items, collections or materials share
    /// a name, compared case insensitively like vpinball's lookups,
    /// reported once per name with how many carry it. Only the first image
    /// or sound is ever found: vpinball drops an exact duplicate when it
    /// loads the table and its lookups stop at the first match for one
    /// that differs in case only. A duplicate part is renamed at load,
    /// which breaks the script's reference to it. Duplicate materials are
    /// kept, but the editor resolves a reference by its exact name while
    /// the player looks it up case insensitively and takes the last one
    /// loaded, so the two render it differently
    DuplicateName {
        kind: NameKind,
        name: String,
        count: usize,
    },
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
    /// A textbox or decal uses a font that is neither embedded in the
    /// table nor one of the core fonts available on every platform, so
    /// it renders with a substitute on any machine without it installed.
    /// Standalone resolves fonts differently: it looks for
    /// `Name-Style.ttf` (spaces removed) next to the table and falls back
    /// to Liberation Sans for anything else, embedded or not
    NonStandardFont { item: String, font: String },
    /// The script uses a table property vpinball has deprecated; it logs
    /// an error and the call does nothing
    DeprecatedTableProperty { property: String },
    /// The script sets a VPinMAME controller property the standalone
    /// PinMAME plugin has deprecated; it logs and ignores it, VPinMAME
    /// on Windows still honors it
    DeprecatedControllerProperty { property: String },
    /// A primitive mesh with over a million vertices; the biggest tables
    /// bake a whole playfield into one, anything else that size is a
    /// mistake in the import
    HugeMesh {
        item: String,
        vertices: u32,
        indices: u32,
    },
    /// An image no game item or table setting uses and the script never
    /// names; it only adds to the file
    UnusedImage { image: String, bytes: usize },
    /// A sound the script never names; it only adds to the file
    UnusedSound { sound: String, bytes: usize },
    /// Materials no game item or the playfield uses and the script never
    /// names. They cost nothing in the file, but they clutter the material
    /// list; reported once per table since most tables carry dozens
    UnusedMaterials { names: Vec<String>, total: usize },
    /// The script plays or stops a sound that does not exist; vpinball
    /// logs a warning and plays nothing
    MissingSound { sound: String },
    /// The image's stored width and height differ from the encoded
    /// picture; older vpinball versions wrote the dimensions after their
    /// load time resize. vpinball logs it as a corrupted file and uses the
    /// picture's own size, so this only matters to tools reading the header
    ImageDimensionMismatch {
        image: String,
        /// Width and height stored in the image record
        stored: (u32, u32),
        /// Width and height of the encoded picture
        actual: (u32, u32),
    },
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
    /// A primitive is marked static, which bakes it at load, while the
    /// script refers to it; most of its properties cannot change at
    /// runtime then (reading them is fine)
    StaticPrimitiveInScript { item: String },
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
    /// The script declares a variable, constant, procedure or class with
    /// the name of a game item or collection, which hides the item from
    /// the script
    ScriptNameShadowsItem { name: String, kind: NameKind },
    /// The script uses `Rnd` without calling `Randomize`, so every run
    /// draws the same sequence
    RndWithoutRandomize,
    /// A game item's timer is enabled but the script has no `<item>_Timer`
    /// handler, so every tick fires an event nothing receives. Handlers
    /// core.vbs builds (`PinMAMETimer`, `PulseTimer`, `vpmBuildEvent`,
    /// `InitTimer`) and timers handled through an event firing collection
    /// are accounted for; handlers built with `Execute`, `ExecuteGlobal` or
    /// `Eval` are not seen
    TimerWithoutHandler { item: String, interval: i32 },
    /// Event handlers such as `<name>_Hit` or `<name>_Timer` whose item,
    /// collection or table does not exist and that nothing calls by name:
    /// dead code, typically a sound package pasted in without its
    /// collections, or an item that was renamed. Reported once per table
    HandlersWithoutItem { names: Vec<String> },
}

/// How serious a [`Finding`] is
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Severity {
    /// Worth knowing, nothing to fix
    Info,
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
            | Finding::UnusedFont { .. }
            | Finding::NonStandardFont { .. }
            | Finding::DeprecatedTableProperty { .. } => Severity::Suggestion,
            Finding::DeprecatedControllerProperty { .. } | Finding::HugeMesh { .. } => {
                Severity::Info
            }
            Finding::UnusedImage { .. } | Finding::UnusedSound { .. } => Severity::Suggestion,
            Finding::UnusedMaterials { .. } => Severity::Info,
            Finding::ImageDimensionMismatch { .. } => Severity::Info,
            Finding::NegativeLightIntensity { .. } | Finding::StereoTableSound { .. } => {
                Severity::Error
            }
            Finding::MissingOptionExplicit | Finding::RndWithoutRandomize => Severity::Suggestion,
            Finding::TimerWithoutHandler { .. } | Finding::HandlersWithoutItem { .. } => {
                Severity::Info
            }
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
    Material,
}

impl fmt::Display for NameKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameKind::Image => write!(f, "image"),
            NameKind::Sound => write!(f, "sound"),
            NameKind::GameItem => write!(f, "game item"),
            NameKind::Collection => write!(f, "collection"),
            NameKind::Material => write!(f, "material"),
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
            Finding::DuplicateName { kind, name, count } => {
                let consequence = match kind {
                    NameKind::Image | NameKind::Sound => "vpinball only ever finds the first",
                    NameKind::GameItem => {
                        "vpinball renames all but the first, which breaks the script's reference"
                    }
                    NameKind::Collection => "the script reaches only one of them",
                    NameKind::Material => {
                        "the editor uses the exact match and the player the last one"
                    }
                };
                write!(f, "{count} {kind}s share the name {name:?}, {consequence}")
            }
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
            Finding::HugeMesh {
                item,
                vertices,
                indices,
            } => write!(
                f,
                "{item}: mesh has {vertices} vertices and {indices} indices, in the top thousandth of all primitives"
            ),
            Finding::ImageDimensionMismatch {
                image,
                stored,
                actual,
            } => write!(
                f,
                "image {image:?} is stored as {}x{} but the picture is {}x{}; vpinball logs a corrupted file and uses the picture's size",
                stored.0, stored.1, actual.0, actual.1
            ),

            Finding::UnusedFont { font, faces } => write!(
                f,
                "font {font:?} ({}) is not used by any textbox or decal and the script does not mention it",
                faces
                    .iter()
                    .map(|face| format!("{face:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Finding::NonStandardFont { item, font } => write!(
                f,
                "{item}: font {font:?} is not embedded in the table and not a core font, so it renders with a substitute where it is not installed (standalone needs it as a .ttf next to the table)"
            ),
            Finding::DeprecatedControllerProperty { property } => write!(
                f,
                "script sets the VPinMAME controller property {property}, which the standalone PinMAME plugin ignores"
            ),
            Finding::DeprecatedTableProperty { property } => write!(
                f,
                "script uses the deprecated table property {property}, which does nothing"
            ),
            Finding::UnusedImage { image, bytes } => write!(
                f,
                "image {image:?} ({} KB) is not used by any item or table setting and the script does not name it",
                bytes / 1024
            ),
            Finding::UnusedMaterials { names, total } => write!(
                f,
                "{} of {total} materials are not used by any item or the playfield and the script does not name them: {}",
                names.len(),
                names
                    .iter()
                    .map(|name| format!("{name:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Finding::MissingSound { sound } => {
                write!(f, "script names missing sound {sound:?}")
            }
            Finding::UnusedSound { sound, bytes } => write!(
                f,
                "sound {sound:?} ({} KB) is not named in the script",
                bytes / 1024
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
            Finding::StaticPrimitiveInScript { item } => write!(
                f,
                "{item}: is static (baked at load) but the script refers to it; most of its properties cannot change at runtime"
            ),
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
            Finding::ScriptNameShadowsItem { name, kind } => write!(
                f,
                "script declares {name:?}, which hides the {kind} of that name"
            ),
            Finding::RndWithoutRandomize => write!(
                f,
                "script uses Rnd without Randomize, so every run draws the same numbers"
            ),
            Finding::MissingPulseTimer => write!(
                f,
                "script uses 'vpmTimer' but the table has no timer named 'PulseTimer'"
            ),
            Finding::TimerWithoutHandler { item, interval } => {
                write!(f, "timer of {item:?} fires ")?;
                match interval {
                    -1 => write!(f, "every frame")?,
                    ms => write!(f, "every {ms} ms")?,
                }
                write!(f, " but the script has no {item}_Timer handler")
            }
            Finding::HandlersWithoutItem { names } => write!(
                f,
                "script has {} event handlers for items that do not exist: {}",
                names.len(),
                names
                    .iter()
                    .map(|name| format!("{name:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
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
    check_font_availability(vpx, &mut findings);

    check_deprecated_properties(vpx, &mut findings);
    check_deprecated_controller_properties(vpx, &mut findings);
    check_unused_assets(vpx, &mut findings);
    check_sound_calls(vpx, &mut findings);

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
    // a 10.8 table carries both lists but vpinball replaces the old one
    // with the new
    match &vpx.gamedata.materials {
        Some(materials) => check_duplicates(
            materials.iter().map(|material| material.name.as_str()),
            NameKind::Material,
            &mut findings,
        ),
        None => check_duplicates(
            vpx.gamedata
                .materials_old
                .iter()
                .map(|material| material.name.as_str()),
            NameKind::Material,
            &mut findings,
        ),
    }

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
        if let Some(actual) = picture_dimensions(image)
            && actual != (image.width, image.height)
        {
            findings.push(Finding::ImageDimensionMismatch {
                image: image.name.clone(),
                stored: (image.width, image.height),
                actual,
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
        check_mesh_size(item, &mut findings);
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

/// One in a thousand primitives in a corpus of 306 thousand has more
/// vertices than this; the ten above it are whole-playfield bakes of
/// VPW tables
const HUGE_MESH_VERTICES: u32 = 1_000_000;

fn check_mesh_size(item: &GameItemEnum, findings: &mut Vec<Finding>) {
    if let GameItemEnum::Primitive(primitive) = item
        && let Some(vertices) = primitive.num_vertices
        && vertices > HUGE_MESH_VERTICES
    {
        findings.push(Finding::HugeMesh {
            item: item_label(item),
            vertices,
            indices: primitive.num_indices.unwrap_or(0),
        });
    }
}

/// The size of the encoded picture, read from its header only; a bitmap
/// has no header, its decoded byte count tells whether the stored size
/// fits. `None` when there is no data or it does not parse.
fn picture_dimensions(image: &crate::vpx::image::ImageData) -> Option<(u32, u32)> {
    if let Some(jpeg) = &image.jpeg {
        let mut reader = ::image::ImageReader::new(std::io::Cursor::new(&jpeg.data));
        if let Some(format) = ::image::ImageFormat::from_extension(image.ext()) {
            reader.set_format(format);
        }
        return reader.with_guessed_format().ok()?.into_dimensions().ok();
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
    // the first spelling and how often the name occurs, in first seen order
    let mut seen: Vec<(&str, usize)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        match index.get(&name.to_lowercase()) {
            Some(&at) => seen[at].1 += 1,
            None => {
                index.insert(name.to_lowercase(), seen.len());
                seen.push((name, 1));
            }
        }
    }
    for (name, count) in seen {
        if count > 1 {
            findings.push(Finding::DuplicateName {
                kind,
                name: name.to_string(),
                count,
            });
        }
    }
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
fn check_font_availability(vpx: &VPX, findings: &mut Vec<Finding>) {
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
            GameItemEnum::TextBox(textbox) => textbox.font.name(),
            GameItemEnum::Decal(decal) => decal.font.name(),
            _ => continue,
        };
        if !font.is_empty() && !available(font) {
            findings.push(Finding::NonStandardFont {
                item: item_label(item),
                font: font.to_string(),
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

/// Table properties vpinball logs "is deprecated" for and ignores
/// (pintable.cpp); the camera ones moved to the view setups
const DEPRECATED_TABLE_PROPERTIES: &[&str] = &[
    "3DOffset",
    "BackglassMode",
    "EnableAntialiasing",
    "EnableFXAA",
    "FieldOfView",
    "GlobalAlphaAcc",
    "GlobalDayNight",
    "GlobalStereo3D",
    "Inclination",
    "Layback",
    "MaxSeparation",
    "PlungerFilter",
    "PlungerNormalize",
    "ReflectElementsOnPlayfield",
    "Rotation",
    "Scalex",
    "Scaley",
    "Scalez",
    "TableAdaptiveVSync",
    "TableHeight",
    "Xlatex",
    "Xlatey",
    "Xlatez",
    "YieldTime",
    "ZPD",
];

/// VPinMAME controller properties the standalone PinMAME plugin logs
/// "is deprecated" for (plugins/pinmame/Controller.h); they concern the
/// VPinMAME window, which the plugin does not have
const DEPRECATED_CONTROLLER_PROPERTIES: &[&str] = &[
    "CabinetMode",
    "DoubleSize",
    "FastFrames",
    "HandleKeyboard",
    "IgnoreRomCrc",
    "LockDisplay",
    "ShowDMDOnly",
    "ShowFrame",
    "ShowOptsDialog",
    "ShowTitle",
    "SoundMode",
];

/// The code of a script with comments and string literals removed, lower
/// cased, one line per line
fn script_code(script: &str) -> String {
    let mut code = String::with_capacity(script.len());
    for line in script.lines() {
        let mut in_string = false;
        for c in line.chars() {
            match (in_string, c) {
                (false, '"') => in_string = true,
                (false, '\'') => break,
                (false, c) => code.push(c.to_ascii_lowercase()),
                (true, '"') => in_string = false,
                (true, _) => {}
            }
        }
        code.push('\n');
    }
    code
}

/// Deprecated properties accessed on the table object, by its name
/// (`Table1.Inclination`); other objects have properties with some of
/// these names, so only the table qualified form counts
fn check_deprecated_properties(vpx: &VPX, findings: &mut Vec<Finding>) {
    let table = vpx.gamedata.name.to_lowercase();
    if table.is_empty() {
        return;
    }
    let code = script_code(&vpx.gamedata.code.string);
    let prefix = format!("{table}.");
    let mut reported: HashSet<&str> = HashSet::new();
    let mut rest = code.as_str();
    while let Some(position) = rest.find(&prefix) {
        let word_start = position == 0
            || !rest[..position]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
        let after = &rest[position + prefix.len()..];
        let property: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if word_start
            && let Some(known) = DEPRECATED_TABLE_PROPERTIES
                .iter()
                .find(|known| known.eq_ignore_ascii_case(&property))
            && reported.insert(known)
        {
            findings.push(Finding::DeprecatedTableProperty {
                property: (*known).to_string(),
            });
        }
        rest = after;
    }
}

/// Controller properties are set on whatever variable holds the
/// controller, usually inside `With Controller`, so any `.name` member
/// access counts; the names are specific to VPinMAME
fn check_deprecated_controller_properties(vpx: &VPX, findings: &mut Vec<Finding>) {
    let code = script_code(&vpx.gamedata.code.string);
    let mut reported: HashSet<&str> = HashSet::new();
    for word in code.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
        let Some(member) = word.rsplit('.').next().filter(|_| word.contains('.')) else {
            continue;
        };
        if let Some(known) = DEPRECATED_CONTROLLER_PROPERTIES
            .iter()
            .find(|known| known.eq_ignore_ascii_case(member))
            && reported.insert(known)
        {
            findings.push(Finding::DeprecatedControllerProperty {
                property: (*known).to_string(),
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
fn check_sound_calls(vpx: &VPX, findings: &mut Vec<Finding>) {
    let sounds = name_set(vpx.sounds.iter().map(|sound| sound.name.as_str()));
    let mut reported: HashSet<String> = HashSet::new();
    for (name, joined) in sound_call_literals(&vpx.gamedata.code.string) {
        let exists = if joined {
            sounds.iter().any(|sound| sound.starts_with(&name))
        } else {
            sounds.contains(&name)
        };
        if !name.is_empty() && !exists && reported.insert(name.clone()) {
            findings.push(Finding::MissingSound { sound: name });
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
fn check_unused_assets(vpx: &VPX, findings: &mut Vec<Finding>) {
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
            findings.push(Finding::UnusedImage {
                image: image.name.clone(),
                bytes,
            });
        }
    }
    for sound in &vpx.sounds {
        if !script_names(&literals, &sound.name) {
            findings.push(Finding::UnusedSound {
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
        findings.push(Finding::UnusedMaterials {
            names: unused,
            total: material_names.len(),
        });
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
        let unused: Vec<String> = audit(&vpx)
            .into_iter()
            .find_map(|finding| match finding {
                Finding::UnusedMaterials { names, .. } => Some(names),
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

    #[test]
    fn a_clean_table_has_no_findings() {
        assert_eq!(audit(&clean_vpx()), Vec::new());
    }

    #[test]
    fn the_blank_template_has_dangling_default_references() {
        // vpinball's blank template keeps the default image names while the
        // images themselves are only present in the sample table
        let findings: Vec<Finding> = audit(&blank_vpx())
            .into_iter()
            // the template's score textbox uses a Windows only font, it carries
            // materials nothing uses, and its script draws random numbers
            // without Randomize, plays the sample table's sounds and has timer
            // handlers for timers it does not have
            .filter(|finding| {
                !matches!(
                    finding,
                    Finding::NonStandardFont { .. }
                        | Finding::RndWithoutRandomize
                        | Finding::HandlersWithoutItem { .. }
                        | Finding::UnusedMaterials { .. }
                        | Finding::MissingSound { .. }
                )
            })
            .collect();
        // 6 dangling references and the image nothing refers to
        assert_eq!(findings.len(), 7, "{findings:#?}");
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
        for name in ["ding", "DING", "Ding"] {
            vpx.images.push(crate::vpx::image::ImageData {
                name: name.to_string(),
                ..Default::default()
            });
        }
        // the duplicates are referenced so only the duplicate is reported
        vpx.gamedata.image = "ding".to_string();
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::DuplicateName {
                kind: NameKind::Image,
                name: "ding".to_string(),
                count: 3,
            }]
        );
        assert_eq!(
            findings[0].to_string(),
            "3 images share the name \"ding\", vpinball only ever finds the first"
        );
    }

    #[test]
    fn duplicate_materials_are_reported_from_the_list_vpinball_uses() {
        use crate::vpx::material::{Material, SaveMaterial};
        let mut vpx = clean_vpx();
        // the old list of a 10.8 table repeats the new one, so counting
        // both would report every material
        let names = ["Apron", "apron", "Plastic"];
        vpx.gamedata.materials_old = names
            .iter()
            .map(|name| SaveMaterial {
                name: name.to_string(),
                ..Default::default()
            })
            .collect();
        vpx.gamedata.materials = Some(
            names
                .iter()
                .map(|name| {
                    let mut material = Material::default();
                    material.name = name.to_string();
                    material
                })
                .collect(),
        );
        let findings: Vec<Finding> = audit(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Finding::DuplicateName { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![Finding::DuplicateName {
                kind: NameKind::Material,
                name: "Apron".to_string(),
                count: 2,
            }]
        );
        assert_eq!(
            findings[0].to_string(),
            "2 materials share the name \"Apron\", the editor uses the exact match and the player the last one"
        );

        // a table from before 10.8 only has the old list
        vpx.gamedata.materials = None;
        let findings: Vec<Finding> = audit(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Finding::DuplicateName { .. }))
            .collect();
        assert_eq!(findings.len(), 1);
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
        vpx.gamedata
            .set_code("Option Explicit\r\nSub F1_Timer\r\nEnd Sub\r\n".to_string());
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
        let findings: Vec<Finding> = audit(&vpx)
            .into_iter()
            .filter(|finding| !matches!(finding, Finding::UnusedMaterials { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![Finding::HugeMesh {
                item: "Primitive \"Bake\"".to_string(),
                vertices: 2_000_000,
                indices: 2_100_000,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
    }

    #[test]
    fn stale_image_dimensions_are_informational() {
        use crate::vpx::image::{ImageData, ImageDataJpeg};
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
            jpeg: Some(ImageDataJpeg {
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
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![Finding::ImageDimensionMismatch {
                image: "resized".to_string(),
                stored: (8, 8),
                actual: (2, 3),
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
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

    #[test]
    fn a_font_that_is_neither_embedded_nor_standard_is_reported() {
        use crate::vpx::font::FontData;
        use crate::vpx::gameitem::font::Font;
        use crate::vpx::gameitem::textbox::TextBox;
        use crate::vpx::ttf::font_with_names;
        let textbox = |name: &str, font: &str| {
            GameItemEnum::TextBox(TextBox {
                name: name.to_string(),
                font: Font::new(0, Default::default(), 400, 120000, font.to_string()),
                ..TextBox::default()
            })
        };
        let mut vpx = clean_vpx();
        vpx.fonts = vec![FontData {
            name: "led".to_string(),
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
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![
                Finding::NonStandardFont {
                    item: "TextBox \"Windows\"".to_string(),
                    font: "Segoe UI".to_string(),
                },
                Finding::NonStandardFont {
                    item: "TextBox \"Custom\"".to_string(),
                    font: "Bebas Neue".to_string(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
    }

    #[test]
    fn deprecated_table_properties_are_reported_once() {
        let mut vpx = clean_vpx();
        vpx.gamedata.name = "Table1".to_string();
        vpx.gamedata.set_code(
            "Option Explicit\r\nTABLE1.Inclination = 42\r\nTable1.Inclination = 43\r\nTable1.Layback = 1 ' Table1.ZPD = 2\r\nx = \"Table1.YieldTime\"\r\nMyTable1.Rotation = 1\r\nPrimitive1.Rotation = 90\r\nTable1.Name = \"x\"\r\n"
                .to_string(),
        );
        assert_eq!(
            audit(&vpx),
            vec![
                Finding::DeprecatedTableProperty {
                    property: "Inclination".to_string(),
                },
                Finding::DeprecatedTableProperty {
                    property: "Layback".to_string(),
                },
            ]
        );
    }

    #[test]
    fn deprecated_controller_properties_are_informational() {
        let mut vpx = clean_vpx();
        vpx.gamedata.set_code(
            "Option Explicit\r\nWith Controller\r\n    .ShowTitle = False\r\n    .ShowDMDOnly = 1 : .ShowFrame = 0\r\n    .HandleKeyboard = 0\r\n    .Run\r\nEnd With\r\n' .ShowTitle in a comment\r\nx = \"ShowTitle\"\r\n"
                .to_string(),
        );
        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![
                Finding::DeprecatedControllerProperty {
                    property: "ShowTitle".to_string(),
                },
                Finding::DeprecatedControllerProperty {
                    property: "ShowDMDOnly".to_string(),
                },
                Finding::DeprecatedControllerProperty {
                    property: "ShowFrame".to_string(),
                },
                Finding::DeprecatedControllerProperty {
                    property: "HandleKeyboard".to_string(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
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
            audit(&vpx),
            vec![
                Finding::MissingSound {
                    sound: "fx_gone".to_string(),
                },
                Finding::MissingSound {
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

        let findings = audit(&vpx);
        assert_eq!(
            findings,
            vec![
                Finding::UnusedImage {
                    image: "orphan".to_string(),
                    bytes: 0,
                },
                Finding::UnusedSound {
                    sound: "fx_unused".to_string(),
                    bytes: 2048,
                },
                Finding::UnusedMaterials {
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
                            | Finding::ScriptNameShadowsItem { .. }
                            | Finding::RndWithoutRandomize
                            | Finding::TimerWithoutHandler { .. }
                            | Finding::HandlersWithoutItem { .. }
                            | Finding::StaticPrimitiveInScript { .. }
                    )
                })
                .collect()
        }

        #[test]
        fn an_enabled_timer_without_a_handler_is_reported() {
            use crate::vpx::collection::Collection;
            use crate::vpx::gameitem::timer::Timer;
            let mut vpx = scripted(
                "Sub Handled_Timer\nEnd Sub\n\
                 Sub Group_Timer(idx)\nEnd Sub\n\
                 Sub Table1_Init\n    vpmTimer.InitTimer Pulsed, True\n    Call vpmBuildEvent(Built, \"Timer\", \"x\")\nEnd Sub\n",
            );
            for (name, enabled, interval) in [
                ("Handled", true, 100),
                ("Grouped", true, 100),
                ("Pulsed", true, 100),
                ("Built", true, 100),
                ("PulseTimer", true, 1),
                ("Off", false, 100),
                ("Orphan", true, -1),
                ("Lonely", true, 250),
            ] {
                let mut timer = Timer {
                    name: name.to_string(),
                    ..Timer::default()
                };
                timer.timer.is_enabled = enabled;
                timer.timer.interval = interval;
                vpx.gameitems.push(GameItemEnum::Timer(timer));
            }
            vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
            vpx.collections.push(Collection {
                name: "Group".to_string(),
                items: vec!["Grouped".to_string()],
                fire_events: true,
                stop_single_events: false,
                group_elements: false,
            });
            vpx.gamedata.collections_size = 1;
            let findings = script_findings(&vpx);
            assert_eq!(
                findings,
                vec![
                    Finding::TimerWithoutHandler {
                        item: "Orphan".to_string(),
                        interval: -1,
                    },
                    Finding::TimerWithoutHandler {
                        item: "Lonely".to_string(),
                        interval: 250,
                    },
                ]
            );
            assert_eq!(findings[0].severity(), Severity::Info);
            assert_eq!(
                findings[0].to_string(),
                "timer of \"Orphan\" fires every frame but the script has no Orphan_Timer handler"
            );
        }

        #[test]
        fn handlers_for_missing_items_are_reported_once() {
            use crate::vpx::gameitem::wall::Wall;
            let mut vpx = scripted(
                "Sub Bumper1_Hit\nEnd Sub\n\
                 Sub Arch1_Hit\nEnd Sub\n\
                 Sub TBWR_Timer\nEnd Sub\n\
                 Sub Game_Init\nEnd Sub\n\
                 Sub Table1_KeyDown(ByVal key)\nEnd Sub\n\
                 Sub Helper_Timer\nEnd Sub\n\
                 Sub Table1_Init\n    Game_Init\n    Helper_Timer\nEnd Sub\n\
                 Class Foo\n    Public Sub Bar_Hit\n    End Sub\nEnd Class\n",
            );
            vpx.gameitems.push(GameItemEnum::Wall(Wall {
                name: "Bumper1".to_string(),
                ..Wall::default()
            }));
            vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
            vpx.gamedata.name = "Table1".to_string();
            let findings = script_findings(&vpx);
            assert_eq!(
                findings,
                vec![Finding::HandlersWithoutItem {
                    names: vec!["Arch1_Hit".to_string(), "TBWR_Timer".to_string()],
                }]
            );
            assert_eq!(findings[0].severity(), Severity::Info);
        }

        #[test]
        fn script_names_that_hide_items_are_reported() {
            use crate::vpx::collection::Collection;
            use crate::vpx::gameitem::wall::Wall;
            let mut vpx = scripted(
                "Dim Bumper1, Free\n\
                 Const Wall1 = 3\n\
                 Public Wall2\n\
                 Sub AllLights\nEnd Sub\n\
                 Class Wall3\nEnd Class\n\
                 Sub Table1_Init\n    Dim Wall4\nEnd Sub\n",
            );
            for name in ["Bumper1", "Wall1", "Wall2", "Wall3", "Wall4"] {
                vpx.gameitems.push(GameItemEnum::Wall(Wall {
                    name: name.to_string(),
                    ..Wall::default()
                }));
            }
            vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
            vpx.collections.push(Collection {
                name: "AllLights".to_string(),
                items: Vec::new(),
                fire_events: false,
                stop_single_events: false,
                group_elements: false,
            });
            vpx.gamedata.collections_size = vpx.collections.len() as u32;
            let shadows = |name: &str, kind: NameKind| Finding::ScriptNameShadowsItem {
                name: name.to_string(),
                kind,
            };
            assert_eq!(
                script_findings(&vpx),
                vec![
                    shadows("Bumper1", NameKind::GameItem),
                    shadows("Wall1", NameKind::GameItem),
                    shadows("Wall2", NameKind::GameItem),
                    shadows("AllLights", NameKind::Collection),
                    shadows("Wall3", NameKind::GameItem),
                ]
            );
        }

        #[test]
        fn rnd_without_randomize_is_a_suggestion() {
            let vpx = scripted("Sub Table1_Init\n    x = Rnd * 10\nEnd Sub\n");
            assert_eq!(script_findings(&vpx), vec![Finding::RndWithoutRandomize]);
            assert_eq!(
                Finding::RndWithoutRandomize.severity(),
                Severity::Suggestion
            );
            let vpx = scripted("Randomize\nSub Table1_Init\n    x = Rnd * 10\nEnd Sub\n");
            assert_eq!(script_findings(&vpx), vec![]);
        }

        #[test]
        fn a_static_primitive_named_in_the_script_is_reported() {
            use crate::vpx::gameitem::primitive::Primitive;
            let mut vpx = scripted(
                "Sub Table1_Init\r\n    Baked.Visible = False\r\n    Dynamic.Visible = False\r\nEnd Sub\r\n",
            );
            let primitive = |name: &str, static_rendering: bool| {
                GameItemEnum::Primitive(Box::new(Primitive {
                    name: name.to_string(),
                    static_rendering,
                    ..Primitive::default()
                }))
            };
            vpx.gameitems.push(primitive("Baked", true));
            vpx.gameitems.push(primitive("Dynamic", false));
            vpx.gameitems.push(primitive("Unmentioned", true));
            vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
            assert_eq!(
                script_findings(&vpx),
                vec![Finding::StaticPrimitiveInScript {
                    item: "Primitive \"Baked\"".to_string(),
                }]
            );
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
    use super::{Finding, NameKind, VPX};
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
        /// names declared at script level: variables, constants, subs,
        /// functions and classes, which all live in the namespace the
        /// table items are in
        script_level: Vec<String>,
        /// how many procedures deep the scan is
        depth: usize,
        /// subs and functions declared at script level, the only ones
        /// vpinball can dispatch events to
        procedures: Vec<String>,
        /// items handed to core.vbs `vpmBuildEvent` or `InitTimer`, which
        /// build the timer handler at runtime
        built_events: HashSet<String>,
    }

    /// The events vpinball fires on script objects, from vpinball.idl
    const EVENTS: [&str; 21] = [
        "init",
        "timer",
        "hit",
        "unhit",
        "animate",
        "limiteos",
        "limitbos",
        "spin",
        "slingshot",
        "raised",
        "dropped",
        "paused",
        "unpaused",
        "sounddone",
        "playdone",
        "optionevent",
        "musicdone",
        "keyup",
        "keydown",
        "exit",
        "collide",
    ];

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

        // a script level name equal to an item or collection name hides it
        let items: HashSet<String> = vpx
            .gameitems
            .iter()
            .map(|item| item.name().to_lowercase())
            .collect();
        let collections: HashSet<String> = vpx
            .collections
            .iter()
            .map(|collection| collection.name.to_lowercase())
            .collect();
        let mut reported: HashSet<String> = HashSet::new();
        for name in &scan.script_level {
            let lower = name.to_lowercase();
            let kind = if items.contains(&lower) {
                NameKind::GameItem
            } else if collections.contains(&lower) {
                NameKind::Collection
            } else {
                continue;
            };
            if reported.insert(lower) {
                findings.push(Finding::ScriptNameShadowsItem {
                    name: name.clone(),
                    kind,
                });
            }
        }

        if scan.identifiers.contains("rnd") && !scan.identifiers.contains("randomize") {
            findings.push(Finding::RndWithoutRandomize);
        }

        // enabled timers nothing handles, and handlers nothing fires
        let procedures: HashSet<String> = scan
            .procedures
            .iter()
            .map(|name| name.to_lowercase())
            .collect();
        let handled_by_collection: HashSet<String> = vpx
            .collections
            .iter()
            .filter(|collection| {
                collection.fire_events
                    && procedures.contains(&format!("{}_timer", collection.name.to_lowercase()))
            })
            .flat_map(|collection| collection.items.iter().map(|item| item.to_lowercase()))
            .collect();
        for item in &vpx.gameitems {
            let Some(timer) = item.timer() else {
                continue;
            };
            let name = item.name().to_lowercase();
            let handler = format!("{name}_timer");
            if !timer.is_enabled
                || name.is_empty()
                || procedures.contains(&handler)
                || scan.identifiers.contains(&handler)
                || handled_by_collection.contains(&name)
                || scan.built_events.contains(&name)
                || matches!(name.as_str(), "pinmametimer" | "pulsetimer")
            {
                continue;
            }
            findings.push(Finding::TimerWithoutHandler {
                item: item.name().to_string(),
                interval: timer.interval,
            });
        }
        let table = vpx.gamedata.name.to_lowercase();
        let orphans: Vec<String> = scan
            .procedures
            .iter()
            .filter(|name| {
                let lower = name.to_lowercase();
                lower.rsplit_once('_').is_some_and(|(object, event)| {
                    !object.is_empty()
                        && EVENTS.contains(&event)
                        && object != table
                        && !items.contains(object)
                        && !collections.contains(object)
                        && !scan.identifiers.contains(&lower)
                })
            })
            .cloned()
            .collect();
        if !orphans.is_empty() {
            findings.push(Finding::HandlersWithoutItem { names: orphans });
        }

        // like vpinball: any mention of a static primitive's name, reading a
        // property is fine but writing one has no effect once it is baked
        for item in &vpx.gameitems {
            if let GameItemEnum::Primitive(primitive) = item
                && primitive.static_rendering
                && scan.identifiers.contains(&primitive.name.to_lowercase())
            {
                findings.push(Finding::StaticPrimitiveInScript {
                    item: super::item_label(item),
                });
            }
        }
    }

    impl Scan {
        fn items(&mut self, items: &[Item]) {
            for item in items {
                match item {
                    Item::OptionExplicit => self.option_explicit = true,
                    Item::Class { name, methods, .. } => {
                        self.script_level.push(name.clone());
                        self.current_class = Some(name.clone());
                        self.stmts(methods);
                        self.current_class = None;
                    }
                    Item::Statement(stmt) => self.stmt(stmt),
                    Item::Const { values, .. } => {
                        self.script_level
                            .extend(values.iter().map(|(name, _)| name.clone()));
                    }
                    Item::Variable { vars, .. } => {
                        self.script_level
                            .extend(vars.iter().map(|(name, _)| name.clone()));
                    }
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
                    if self.current_class.is_none() {
                        self.script_level.push(name.clone());
                        if self.depth == 0 {
                            self.procedures.push(name.clone());
                        }
                    }
                    self.declared.push(qualified);
                    self.depth += 1;
                    self.stmts(body);
                    self.depth -= 1;
                }
                // a Dim inside a procedure is local, but VBScript hoists
                // nothing: only script level declarations shadow items
                Stmt::Dim { vars } if self.current_class.is_none() && self.depth == 0 => {
                    self.script_level
                        .extend(vars.iter().map(|(name, _)| name.clone()));
                }
                Stmt::Const(values) if self.current_class.is_none() && self.depth == 0 => {
                    self.script_level
                        .extend(values.iter().map(|(name, _)| name.clone()));
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
                    self.built_event(&fn_name.0, args);
                    self.full_ident(fn_name);
                    self.args(args);
                }
                Stmt::Call(fi) => {
                    if let Expr::FnApplication { callee, args } = &*fi.0 {
                        self.built_event(callee, args);
                    }
                    self.full_ident(fi);
                }
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

        /// `vpmBuildEvent item, ...` and `vpmTimer.InitTimer item, ...` give
        /// the item a handler at runtime
        fn built_event(&mut self, callee: &Expr, args: &[Option<Expr>]) {
            let name = match callee {
                Expr::Ident(name) | Expr::MemberExpression { property: name, .. } => name,
                _ => return,
            };
            if !name.eq_ignore_ascii_case("vpmbuildevent")
                && !name.eq_ignore_ascii_case("inittimer")
            {
                return;
            }
            if let Some(Some(Expr::Ident(item))) = args.first() {
                self.built_events.insert(item.to_lowercase());
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
