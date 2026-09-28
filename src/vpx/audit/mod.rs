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
//!
//! Each check lives in the module for its area: `references` for
//! dangling references, `names` for what things are called, `assets` for
//! images, sounds and fonts, `items` for how game items behave, `code`
//! for text checks on the script and `script` for the checks that parse
//! it. `finding` turns a check's result into the [`Finding`](crate::vpx::audit::Finding) it returns.

pub(crate) mod assets;
mod code;
mod finding;
mod items;
mod names;
mod references;
#[cfg(feature = "script-audit")]
mod script;
#[cfg(test)]
pub(crate) mod test_support;

pub use finding::{Finding, ScriptLocation};

use super::VPX;
use crate::vpx::gameitem::MAX_NAME_LENGTH;
use finding::ItemIndex;
use std::fmt;

/// The structured form of a finding, one variant per check. Crate
/// private so that new checks and new fields are not API changes;
/// [`Finding`] is the flat form [`audit`] returns.
///
/// `item` names the game item carrying the problem, `field` the property
/// holding the dangling reference.
#[derive(Debug, Clone, PartialEq, Eq)]
// the script checks that construct the script variants only exist with
// the script-audit feature, the variants stay so the codes are always known
#[cfg_attr(not(feature = "script-audit"), allow(dead_code))]
pub(crate) enum Kind {
    /// A game item or table setting references an image that does not exist
    MissingImage {
        /// Type and name of the game item that carries the reference, such
        /// as `Wall "Apron"`, or `table settings` for a reference the table
        /// itself holds
        item: String,
        /// The image property that holds the reference, named as the
        /// editor shows it: `image`, `side image`, `normal map`, `decal
        /// image`, or a table setting such as `playfield image`
        field: &'static str,
        /// Name of the referenced image, as written in the item
        image: String,
    },
    /// A table setting references an image that does not exist, but
    /// vpinball renders with a built-in instead: the environment image
    /// falls back to its own environment map, the ball image to its
    /// built-in ball, a ball decal is left off. The reference is stale,
    /// the table plays as intended
    MissingImageWithFallback {
        /// The table setting that holds the reference: `environment
        /// image`, `ball image` or `ball decal image`
        field: &'static str,
        /// Name of the referenced image, as written in the table settings
        image: String,
        /// What vpinball uses instead
        fallback: &'static str,
    },
    /// The color grade image does not exist. vpinball looks it up in the
    /// table only and silently renders without color grading when it is
    /// missing, so this one changes the picture
    MissingColorGradeImage {
        /// Name of the color grade image, as written in the table settings
        image: String,
    },
    /// A game item or table setting references a material that does not exist
    MissingMaterial {
        /// Type and name of the game item that carries the reference, such
        /// as `Wall "Apron"`, or `table settings` for a reference the table
        /// itself holds
        item: String,
        /// The material property that holds the reference, named as the
        /// editor shows it: `material`, `side material`, `physics
        /// material` and so on, or `playfield material` for the table
        field: &'static str,
        /// Name of the referenced material, as written in the item
        material: String,
    },
    /// A game item is placed on a surface (wall or ramp) that does not exist
    MissingSurface {
        /// Type and name of the game item placed on the surface
        item: String,
        /// Name of the wall or ramp the item is placed on, as written in
        /// the item
        surface: String,
    },
    /// A game item belongs to a part group that does not exist
    MissingPartGroup {
        /// Type and name of the game item that belongs to the group
        item: String,
        /// Name of the part group, as written in the item
        part_group: String,
    },
    /// A collection contains an item that does not exist
    MissingCollectionItem {
        /// Name of the collection that lists the item
        collection: String,
        /// Name of the listed game item, as written in the collection
        item: String,
    },
    /// Several images, sounds, game items, collections or materials share
    /// a name, compared case insensitively like vpinball's lookups,
    /// reported once per name with how many carry it. Only the first image
    /// or sound is ever found: vpinball drops the others at load since
    /// September 2026, whatever their case, so a resave loses them. Older
    /// builds drop an exact duplicate only and their lookups stop at the
    /// first match for one that differs in case only. A duplicate part is
    /// renamed at load, which breaks the script's reference to it.
    /// vpinball keeps only the last material of a name since September
    /// 2026 and drops the others at load, so a resave loses them. Older
    /// builds keep all of them, but the editor resolves a reference by its
    /// exact name while the player looks it up case insensitively and
    /// takes the last one loaded, so the two render it differently
    DuplicateName {
        /// Which list of the table the name is in
        kind: NameKind,
        /// The shared name, spelled as the first entry that carries it
        name: String,
        /// How many entries carry the name
        count: usize,
    },
    /// An image, material or surface reference of a game item holds a
    /// character that is lost when the table is saved. The vpx file stores
    /// these records as one byte per character, so anything above `U+00FF`
    /// is written as `?` and the reference no longer matches its target.
    /// A table read from a vpx file never has such text; it comes from an
    /// edit or from the json of an extracted table
    UnstorableText {
        /// Type and name of the game item, such as `Wall "Apron"`
        item: String,
        /// The field holding the text, such as `image` or `top material`
        field: &'static str,
        /// The text as it is now
        text: String,
    },
    /// A game item or collection name is longer than vpinball's editor
    /// allows. vpinball cuts a collection name at load and a game item
    /// name as soon as it is edited, and the script's reference to the
    /// full name then fails
    NameTooLong {
        /// Type and name of the game item, such as `Wall "Apron"`, or
        /// `Collection` and the name of the collection
        item: String,
        /// Length of the name in characters; the limit is
        /// [`MAX_NAME_LENGTH`]
        length: usize,
    },
    /// A game item or collection has a name the script already gives a
    /// meaning: a VBScript keyword, which the script cannot even refer
    /// to; a VBScript builtin function or constant, which the item then
    /// hides, since the script engine resolves named items before its
    /// builtins; or one of the table's global script methods and
    /// properties, which vpinball on Windows renames at load and
    /// standalone vpinball lets the item hide
    ReservedName {
        /// Whether a game item or a collection carries the name
        kind: NameKind,
        /// The reserved name, spelled as the item or collection writes it
        name: String,
        /// What the name clashes with
        reserved: ReservedName,
    },
    /// Game items of one type have no name, reported once per type with
    /// how many. Such an item cannot be reached from the script, and
    /// vpinball gives all but the first a number when it loads the table.
    /// Decals only gained a name in 2026, so tables saved before that have
    /// unnamed ones. vpinball and vpx-editor name them when they load the
    /// table and write the name on save, so a resave fixes it; that is a
    /// suggestion
    UnnamedItems {
        /// The item type, as the editor shows it, such as `Wall` or `Decal`
        type_name: String,
        /// How many items of that type have no name
        count: usize,
    },
    /// The table info has no table name
    MissingTableName,
    /// An image is stored as an uncompressed era bitmap; vpinball suggests
    /// converting these to webp, which
    /// [`fix::bitmaps_to_webp`](crate::vpx::fix::bitmaps_to_webp) does
    BmpImage {
        /// Name of the image
        image: String,
    },
    /// The color grade lookup table image is stored in a lossy format, a
    /// jpeg or a lossy webp. The image is data, every texel a color the
    /// shader looks up, so the compression put every lookup slightly off
    /// and nothing tells the author. Re-saving it does not undo that: the
    /// image needs replacing with the original png or lossless webp LUT
    LossyColorGradeImage {
        /// Name of the color grade image
        image: String,
        /// The lossy format it is stored in: `jpeg` or `lossy webp`
        format: &'static str,
    },
    /// The color grade lookup table image is not the 256x16 layout the
    /// shader expects, which silently renders wrong colors
    ColorGradeLutUnusualSize {
        /// Name of the color grade image
        image: String,
        /// Width of the image in pixels; the shader expects 256
        width: u32,
        /// Height of the image in pixels; the shader expects 16
        height: u32,
    },
    /// The script mixes line ending styles. vpinball and the standalone
    /// VBScript engine accept any of them, but line based tooling (diffs of
    /// an extracted script, patchers, some editors) trips over a mix. The
    /// counts tell a stray line from a wholesale mix
    MixedScriptLineEndings {
        /// How many lines end in CR LF
        crlf: usize,
        /// How many lines end in a bare LF
        lf: usize,
        /// How many lines end in a bare CR
        cr: usize,
    },
    /// An embedded font whose face names no textbox or decal uses and the
    /// script does not mention; it only adds to the file
    UnusedFont {
        /// Name of the embedded font, as the table lists it
        font: String,
        /// Face names inside the font file, which are what a textbox or
        /// decal refers to
        faces: Vec<String>,
    },
    /// A textbox or decal uses a font that is neither embedded in the
    /// table nor one of the core fonts available on every platform, so
    /// it renders with a substitute on any machine without it installed.
    /// Standalone resolves fonts differently: it looks for
    /// `Name-Style.ttf` (spaces removed) next to the table and falls back
    /// to Liberation Sans for anything else, embedded or not
    NonStandardFont {
        /// Type and name of the textbox or decal
        item: String,
        /// Face name of the font, as written in the item
        font: String,
    },
    /// The script uses a table property vpinball has deprecated; it logs
    /// an error and the call does nothing
    DeprecatedTableProperty {
        /// Name of the property, spelled as vpinball declares it
        property: String,
    },
    /// The script sets a VPinMAME controller property the standalone
    /// PinMAME plugin has deprecated; it logs and ignores it, VPinMAME
    /// on Windows still honors it
    DeprecatedControllerProperty {
        /// Name of the property, spelled as VPinMAME declares it
        property: String,
    },
    /// A primitive mesh with over a million vertices; the biggest tables
    /// bake a whole playfield into one, anything else that size is a
    /// mistake in the import
    HugeMesh {
        /// Type and name of the primitive
        item: String,
        /// How many vertices the mesh has
        vertices: u32,
        /// How many indices the mesh has; zero when the table does not
        /// record it
        indices: u32,
    },
    /// An image no game item or table setting uses and the script never
    /// names; it only adds to the file
    UnusedImage {
        /// Name of the image
        image: String,
        /// Size of the stored image data in bytes, the JPEG or the LZW
        /// compressed bitmap as the file holds it
        bytes: usize,
    },
    /// A sound the script never names; it only adds to the file
    UnusedSound {
        /// Name of the sound
        sound: String,
        /// Size of the stored sound data in bytes
        bytes: usize,
    },
    /// Images or sounds whose stored bytes are identical under different
    /// names, one finding per group in the order the table lists them.
    /// For images the parts and the script can point at one name and the
    /// copies go. For sounds that is only true when the script does not
    /// rely on the names: `PlaySound` with `usesame` updates the instance
    /// playing under that name, so a rolling ball sound needs one name
    /// per ball that can roll at once. The common case, one sample stored
    /// once per ball slot, is by design up to that number, and tables
    /// often carry many more copies than balls they can have in play.
    /// Sharing one sample between names is asked of vpinball in
    /// <https://github.com/vpinball/vpinball/issues/3939>
    SameAssetData {
        /// Whether the group holds images or sounds
        kind: NameKind,
        /// Names of the assets that hold the same data, in the order the
        /// table lists them; names that differ in case only count once
        names: Vec<String>,
    },
    /// Materials no game item or the playfield uses and the script never
    /// names. They cost nothing in the file, but they clutter the material
    /// list; reported once per table since most tables carry dozens
    UnusedMaterials {
        /// Names of the unused materials, in the order the table lists them
        names: Vec<String>,
        /// How many materials the table has in all
        total: usize,
    },
    /// The script plays or stops a sound that does not exist; vpinball
    /// logs a warning and plays nothing
    MissingSound {
        /// Name of the sound as the script writes it, lower cased; only
        /// the start of the name when the script builds the rest at runtime
        sound: String,
    },
    /// The image's stored width and height differ from the encoded
    /// picture; older vpinball versions wrote the dimensions after their
    /// load time resize. vpinball logs it as a corrupted file and uses the
    /// picture's own size, so this only matters to tools reading the header
    ImageDimensionMismatch {
        /// Name of the image
        image: String,
        /// Width and height stored in the image record
        stored: (u32, u32),
        /// Width and height of the encoded picture
        actual: (u32, u32),
    },
    /// An image's file name says one format and its content is another: a
    /// gif under a `.png` name, a png under `.jpg`. vpinball reads the
    /// content and never looks at the name, so the table plays as
    /// intended; a tool that trusts the name, an extraction to disk for
    /// one, gets the format wrong
    ImageExtensionMismatch {
        /// Name of the image
        image: String,
        /// The extension of the stored file name, as written
        extension: String,
        /// The format the content is, as [`content_format`](crate::vpx::images::content_format) names it
        format: &'static str,
    },
    /// An image's content starts with no signature of any format vpinball
    /// or vpin reads, and its file name is no help either. vpinball
    /// identifies an image by its content alone and does not load one it
    /// cannot identify, so the image never shows. A tga without the 2.0
    /// footer has no signature either; it is recognised by its `.tga` name
    /// here and by header heuristics in vpinball, so it is not this
    ImageFormatUnknown {
        /// Name of the image
        image: String,
        /// The extension of the stored file name, as written
        extension: String,
    },
    /// An image vpin cannot read. Either its format is known but the
    /// header does not parse, which vpinball may well reject too, or it is
    /// a Photoshop, tiff or dds file, which vpinball reads through
    /// FreeImage but vpin has no decoder for
    UnreadableImage {
        /// Name of the image
        image: String,
        /// The format, as [`content_format`](crate::vpx::images::content_format) names it
        format: &'static str,
        /// What the decoder said about the header, `None` when vpin has no
        /// decoder for the format
        error: Option<String>,
    },
    /// The glass is below two inches or upside down
    GlassHeightInvalid {
        /// What is wrong, as a phrase: the bottom is higher than the top,
        /// or the glass is below two inches
        detail: &'static str,
    },
    /// The legacy spherical ball mapping renders badly in VR, stereo and
    /// head tracked setups
    BallSphericalMapping,
    /// A legacy textbox is used for DMD rendering, a flasher renders better
    TextboxUsedForDmd {
        /// Type and name of the textbox
        item: String,
    },
    /// A timer fires faster than a 60 FPS frame, which causes stutters
    FastTimer {
        /// Type and name of the game item that owns the timer
        item: String,
        /// The timer interval in milliseconds
        interval: i32,
    },
    /// A light has a negative intensity
    NegativeLightIntensity {
        /// Type and name of the light
        item: String,
    },
    /// A primitive lets light from below through (its `disable lighting
    /// from below` is under 1) while nothing about it is see-through: its
    /// material has no active opacity under 1 and its image has no
    /// transparency. vpinball discards the translucency and warns about
    /// it in its own table audit. A static primitive is left alone,
    /// vpinball renders those without translucency by design
    OpaquePrimitiveTranslucency {
        /// Type and name of the primitive
        item: String,
    },
    /// A primitive is marked static, which bakes it at load, while the
    /// script refers to it. Writes to most of its properties are lost
    /// once it is baked, which happens on the first frame, after `Init`
    /// and the startup option event. Reading its properties is fine, and
    /// a script that sets `DisableStaticPrerendering` before writing
    /// renders the primitive dynamically from then on, so its writes land
    StaticPrimitiveInScript {
        /// Name of the primitive, what the script mentions
        name: String,
        /// Type and name of the primitive
        item: String,
        /// The script names `DisableStaticPrerendering`, so its author
        /// knows about baking and most likely switches it off before
        /// writing; reported as info then instead of a warning
        script_toggles_prerendering: bool,
    },
    /// A light with a linear or incandescent fader has a fade speed of
    /// zero, below zero or not a number, so a state change never moves its
    /// intensity: switched through `State` the light stays as it started.
    /// The editor writes a zero speed when the fade time is edited while
    /// the intensity is zero (it stores intensity per millisecond) and
    /// nothing repairs it later
    /// (<https://github.com/vpinball/vpinball/issues/3937>). A script that
    /// sets `Intensity` bypasses the fade and is not affected. A light
    /// saved with a zero intensity shows nothing yet, so that is only a
    /// suggestion until something lights it
    LightCannotFade {
        /// Type and name of the light
        item: String,
        /// The unusable fade up speed, with which the light cannot turn
        /// on, as text: `0`, a negative number or `NaN`
        up: Option<String>,
        /// The unusable fade down speed, with which the light cannot turn
        /// off, as text
        down: Option<String>,
        /// The light has an intensity, so the stuck state shows
        lit: bool,
    },
    /// A sound plays on the playfield speakers but is not mono
    StereoTableSound {
        /// Name of the sound
        sound: String,
    },
    /// The embedded screenshot is large; it bloats the file and every save
    /// spends noticeable time hashing it into the integrity signature.
    /// Large ones are usually PNG captures, which JPEG stores much smaller.
    LargeScreenshot {
        /// Size of the embedded screenshot in bytes
        bytes: usize,
        /// The screenshot is a PNG, which JPEG would store much smaller
        png: bool,
    },
    /// The script could not be parsed, so the script-level checks could not run
    ScriptParseError {
        /// The parser's description of what went wrong, without the position
        detail: String,
        /// Where the parser gave up, when it knows
        location: Option<ScriptLocation>,
    },
    /// The script has no `Option Explicit`, so a typo in a variable name
    /// silently creates a new variable instead of being caught
    MissingOptionExplicit,
    /// A sub or function is declared more than once; the later one wins and
    /// the earlier is dead
    DuplicateProcedure {
        /// Name of the sub or function, spelled as the repeated declaration
        /// writes it
        name: String,
        /// Where the repeated declaration names it
        location: ScriptLocation,
    },
    /// Script level variables the script declares with `Dim`, `Public` or
    /// `Private` and never names again, not even inside a string handed
    /// to `Eval` or `Execute`: dead declarations. Constants are left
    /// alone, and so are the globals the standard scripts read, such as
    /// `BallSize` or `UseVPMDMD`. Reported per variable
    UnusedVariable {
        /// Name of the variable, as the script declares it
        name: String,
        /// Where the script declares it
        location: ScriptLocation,
    },
    /// A variable a sub, function or property declares with `Dim` and never
    /// names in its body: a dead declaration. Reported per variable
    UnusedLocalVariable {
        /// `procedure.variable`, both as the script spells them
        name: String,
        /// Where the procedure declares it
        location: ScriptLocation,
    },
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
    ScriptNameShadowsItem {
        /// The name the script declares, spelled as the script writes it
        name: String,
        /// Whether a game item or a collection is hidden
        kind: NameKind,
        /// Where the script declares it
        location: ScriptLocation,
    },
    /// The script uses `Rnd` without calling `Randomize`, so every run
    /// draws the same sequence
    RndWithoutRandomize,
    /// A game item's timer is enabled but the script has no `<item>_Timer`
    /// handler, so every tick fires an event nothing receives. Handlers
    /// core.vbs builds (`PinMAMETimer`, `PulseTimer`, `vpmBuildEvent`,
    /// `InitTimer`) and timers handled through an event firing collection
    /// are accounted for; handlers built with `Execute`, `ExecuteGlobal` or
    /// `Eval` are not seen
    TimerWithoutHandler {
        /// Name of the game item whose timer is enabled, without its type;
        /// the missing handler is `<item>_Timer`
        item: String,
        /// The timer interval in milliseconds; -1 means every frame
        interval: i32,
    },
    /// Event handlers such as `<name>_Hit` or `<name>_Timer` whose item,
    /// collection or table does not exist and that nothing calls by name:
    /// dead code, typically a sound package pasted in without its
    /// collections, or an item that was renamed. Reported per handler
    HandlerWithoutItem {
        /// Name of the handler, such as `Bumper1_Hit`, as the script spells
        /// it
        name: String,
        /// Where the script declares it
        location: ScriptLocation,
    },
    /// The script assigns a ball's `ID`, which vpinball made read only in
    /// 10.8.1 to keep every ball's id unique: a different value fails and
    /// stops the script with a runtime error. Keep a script's own tag on
    /// a ball in its `UserValue` instead. A write to `Me` or to a variable
    /// the script sets to a new instance of a class declaring an `ID` of
    /// its own is left alone
    BallIdAssigned {
        /// Where the script assigns it
        location: ScriptLocation,
    },
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

impl Kind {
    /// How serious this finding is
    pub(super) fn severity(&self) -> Severity {
        match self {
            Kind::BmpImage { .. }
            | Kind::LargeScreenshot { .. }
            | Kind::MixedScriptLineEndings { .. }
            | Kind::MissingImageWithFallback { .. }
            | Kind::UnusedFont { .. }
            | Kind::NonStandardFont { .. }
            | Kind::DeprecatedTableProperty { .. } => Severity::Suggestion,
            Kind::DeprecatedControllerProperty { .. } | Kind::HugeMesh { .. } => Severity::Info,
            Kind::UnusedImage { .. } | Kind::UnusedSound { .. } | Kind::SameAssetData { .. } => {
                Severity::Suggestion
            }
            Kind::UnusedMaterials { .. } => Severity::Info,
            Kind::ImageDimensionMismatch { .. } | Kind::ImageExtensionMismatch { .. } => {
                Severity::Info
            }
            Kind::ImageFormatUnknown { .. } => Severity::Error,
            Kind::UnreadableImage { error: None, .. } => Severity::Suggestion,
            Kind::NegativeLightIntensity { .. }
            | Kind::StereoTableSound { .. }
            | Kind::BallIdAssigned { .. } => Severity::Error,
            Kind::MissingOptionExplicit | Kind::RndWithoutRandomize => Severity::Suggestion,
            Kind::TimerWithoutHandler { .. } | Kind::HandlerWithoutItem { .. } => Severity::Info,
            Kind::StaticPrimitiveInScript {
                script_toggles_prerendering: true,
                ..
            } => Severity::Info,
            Kind::UnusedVariable { .. } | Kind::UnusedLocalVariable { .. } => Severity::Info,
            Kind::UnnamedItems { type_name, .. } if type_name == "Decal" => Severity::Suggestion,
            Kind::LightCannotFade { lit: false, .. } => Severity::Suggestion,
            Kind::ReservedName { reserved, .. } => match reserved {
                ReservedName::VbsKeyword | ReservedName::TableGlobal => Severity::Error,
                ReservedName::VbsBuiltin => Severity::Warning,
            },
            _ => Severity::Warning,
        }
    }
}

/// What a [`Kind::ReservedName`] clashes with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservedName {
    /// A VBScript keyword such as `To` or `Empty`
    VbsKeyword,
    /// A VBScript builtin function or constant such as `Timer` or `Left`
    VbsBuiltin,
    /// A method or property of the table's global script object, such as
    /// `PlaySound` or `ActiveBall`, or the `Debug` object
    TableGlobal,
}

/// What kind of name a [`Kind::DuplicateName`] is about
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameKind {
    /// A name in the table's image list
    Image,
    /// A name in the table's sound list
    Sound,
    /// A name in the table's game item list
    GameItem,
    /// A name in the table's collection list
    Collection,
    /// A name in the table's material list
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

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::MissingImage { item, field, image } => {
                write!(f, "{item}: {field} references missing image {image:?}")
            }
            Kind::MissingImageWithFallback {
                field,
                image,
                fallback,
            } => write!(
                f,
                "table settings: {field} references missing image {image:?}, vpinball uses {fallback}"
            ),
            Kind::MissingColorGradeImage { image } => write!(
                f,
                "table settings: color grade image {image:?} does not exist, color grading is silently off"
            ),
            Kind::MissingMaterial {
                item,
                field,
                material,
            } => write!(
                f,
                "{item}: {field} references missing material {material:?}"
            ),
            Kind::MissingSurface { item, surface } => {
                write!(f, "{item}: placed on missing surface {surface:?}")
            }
            Kind::MissingPartGroup { item, part_group } => {
                write!(f, "{item}: belongs to missing part group {part_group:?}")
            }
            Kind::MissingCollectionItem { collection, item } => {
                write!(
                    f,
                    "collection {collection:?} contains missing item {item:?}"
                )
            }
            Kind::DuplicateName { kind, name, count } => {
                let consequence = match kind {
                    NameKind::Image | NameKind::Sound => "vpinball keeps only the first one",
                    NameKind::GameItem => {
                        "vpinball renames all but the first, which breaks the script's reference"
                    }
                    NameKind::Collection => "the script reaches only one of them",
                    NameKind::Material => {
                        "vpinball keeps only the last one, older builds render it differently in the editor and the player"
                    }
                };
                write!(f, "{count} {kind}s share the name {name:?}, {consequence}")
            }
            Kind::UnnamedItems { type_name, count } => {
                let items = if *count == 1 {
                    "item has"
                } else {
                    "items have"
                };
                let consequence = if type_name == "Decal" {
                    "saving the table in vpinball or vpx-editor names them"
                } else {
                    "they cannot be reached from the script"
                };
                write!(f, "{count} {type_name} {items} no name, {consequence}")
            }
            Kind::ReservedName {
                kind,
                name,
                reserved,
            } => {
                let consequence = match reserved {
                    ReservedName::VbsKeyword => {
                        "is a VBScript keyword, the script cannot refer to the item"
                    }
                    ReservedName::VbsBuiltin => {
                        "is a VBScript builtin, the item hides it from the script"
                    }
                    ReservedName::TableGlobal => {
                        "is a table script global, vpinball renames the item at load"
                    }
                };
                write!(f, "{kind} {name:?} {consequence}")
            }
            Kind::UnstorableText { item, field, text } => {
                let saved: String = text
                    .chars()
                    .map(|c| if u32::from(c) > 0xFF { '?' } else { c })
                    .collect();
                write!(
                    f,
                    "{item}: {field} {text:?} will be saved to the vpx file as {saved:?}, which stores one byte per character"
                )
            }
            Kind::NameTooLong { item, length } => write!(
                f,
                "{item}: name is {length} characters, vpinball cuts names at {MAX_NAME_LENGTH}"
            ),
            Kind::MissingTableName => write!(f, "table info has no table name"),
            Kind::BmpImage { image } => write!(
                f,
                "image {image:?} is stored as a bitmap, consider converting to webp"
            ),
            Kind::LossyColorGradeImage { image, format } => write!(
                f,
                "color grade image {image:?} is a {format}, a lossy format; the compression already put every color lookup off, replace it with the original png or lossless webp LUT"
            ),
            Kind::ColorGradeLutUnusualSize {
                image,
                width,
                height,
            } => write!(
                f,
                "color grade image {image:?} is {width}x{height}, the shader expects 256x16"
            ),
            Kind::MixedScriptLineEndings { crlf, lf, cr } => {
                let styles: Vec<String> = [(crlf, "CRLF"), (lf, "LF"), (cr, "CR")]
                    .into_iter()
                    .filter(|(count, _)| **count > 0)
                    .map(|(count, name)| format!("{count} {name}"))
                    .collect();
                write!(f, "script mixes line endings: {}", styles.join(", "))
            }
            Kind::HugeMesh {
                item,
                vertices,
                indices,
            } => write!(
                f,
                "{item}: mesh has {vertices} vertices and {indices} indices, in the top thousandth of all primitives"
            ),
            Kind::ImageDimensionMismatch {
                image,
                stored,
                actual,
            } => write!(
                f,
                "image {image:?} is stored as {}x{} but the picture is {}x{}; vpinball logs a corrupted file and uses the picture's size",
                stored.0, stored.1, actual.0, actual.1
            ),
            Kind::ImageExtensionMismatch {
                image,
                extension,
                format,
            } => write!(
                f,
                "image {image:?} is a {format} file stored under a .{extension} name; vpinball reads the content, a tool trusting the name gets the format wrong"
            ),
            Kind::ImageFormatUnknown { image, extension } => write!(
                f,
                "image {image:?} has no known image signature and its .{extension} name is no help; vpinball cannot identify it and does not load it"
            ),
            Kind::UnreadableImage {
                image,
                format,
                error: None,
            } => write!(
                f,
                "image {image:?} is a {format} file, which vpinball reads but most other tools do not; consider re-saving it as png or webp"
            ),
            Kind::UnreadableImage {
                image,
                format,
                error: Some(error),
            } => write!(
                f,
                "image {image:?} is a {format} file whose header does not parse ({error}); vpinball may fail to load it too"
            ),

            Kind::UnusedFont { font, faces } => write!(
                f,
                "font {font:?} ({}) is not used by any textbox or decal and the script does not mention it",
                faces
                    .iter()
                    .map(|face| format!("{face:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Kind::NonStandardFont { item, font } => write!(
                f,
                "{item}: font {font:?} is not embedded in the table and not a core font, so it renders with a substitute where it is not installed (standalone needs it as a .ttf next to the table)"
            ),
            Kind::DeprecatedControllerProperty { property } => write!(
                f,
                "script sets the VPinMAME controller property {property}, which the standalone PinMAME plugin ignores"
            ),
            Kind::DeprecatedTableProperty { property } => write!(
                f,
                "script uses the deprecated table property {property}, which does nothing"
            ),
            Kind::UnusedImage { image, bytes } => write!(
                f,
                "image {image:?} ({} KB) is not used by any item or table setting and the script does not name it",
                bytes / 1024
            ),
            Kind::UnusedMaterials { names, total } => write!(
                f,
                "{} of {total} materials are not used by any item or the playfield and the script does not name them: {}",
                names.len(),
                names
                    .iter()
                    .map(|name| format!("{name:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Kind::MissingSound { sound } => {
                write!(f, "script names missing sound {sound:?}")
            }
            Kind::SameAssetData { kind, names } => write!(
                f,
                "{} {kind}s hold the same data: {}",
                names.len(),
                names
                    .iter()
                    .map(|name| format!("{name:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Kind::UnusedSound { sound, bytes } => write!(
                f,
                "sound {sound:?} ({} KB) is not named in the script",
                bytes / 1024
            ),
            Kind::GlassHeightInvalid { detail } => {
                write!(f, "glass height seems invalid: {detail}")
            }
            Kind::BallSphericalMapping => write!(
                f,
                "ball uses legacy spherical mapping, which renders badly in VR, stereo and head tracked setups"
            ),
            Kind::TextboxUsedForDmd { item } => write!(
                f,
                "{item}: legacy textbox used for DMD rendering, a flasher renders better"
            ),
            Kind::FastTimer { item, interval } => write!(
                f,
                "{item}: timer fires every {interval}ms, faster than a 60 FPS frame, which causes stutters"
            ),
            Kind::NegativeLightIntensity { item } => {
                write!(f, "{item}: negative light intensity")
            }
            Kind::OpaquePrimitiveTranslucency { item } => write!(
                f,
                "{item}: uses translucency (lighting from below) while it is fully opaque, vpinball discards the translucency"
            ),
            Kind::StaticPrimitiveInScript {
                item,
                script_toggles_prerendering: true,
                ..
            } => write!(
                f,
                "{item}: is static (baked at load) and the script refers to it; the script toggles DisableStaticPrerendering, so writes to its properties land while that is set or before the first frame"
            ),
            Kind::StaticPrimitiveInScript {
                item,
                script_toggles_prerendering: false,
                ..
            } => write!(
                f,
                "{item}: is static (baked at load) but the script refers to it; writes to most of its properties are lost after the first frame (Init and the startup option event still land) unless the script sets DisableStaticPrerendering first; reading is fine"
            ),
            Kind::LightCannotFade { item, up, down, .. } => {
                let which = match (up, down) {
                    (Some(up), Some(down)) if up == down => format!("fade speeds are {up}"),
                    (Some(up), Some(down)) => format!("fade speeds are {up} and {down}"),
                    (Some(up), None) => format!("fade up speed is {up}"),
                    (None, Some(down)) => format!("fade down speed is {down}"),
                    (None, None) => "fade speed is unusable".to_string(),
                };
                write!(f, "{item}: {which}, state changes never show")
            }
            Kind::StereoTableSound { sound } => write!(
                f,
                "sound {sound:?} plays on the playfield speakers but is not mono"
            ),
            Kind::LargeScreenshot { bytes, png } => {
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
            Kind::ScriptParseError { detail, .. } => {
                write!(f, "script could not be parsed: {detail}")
            }
            Kind::MissingOptionExplicit => {
                write!(
                    f,
                    "script has no 'Option Explicit', typos create silent new variables"
                )
            }
            Kind::DuplicateProcedure { name, .. } => {
                write!(f, "script declares {name:?} more than once")
            }
            Kind::UnusedVariable { name, .. } => {
                write!(f, "script declares the variable {name:?} and never uses it")
            }
            Kind::UnusedLocalVariable { name, .. } => write!(
                f,
                "script declares the local variable {name:?} and its procedure never uses it"
            ),
            Kind::ExecuteUsed => {
                write!(
                    f,
                    "script uses Execute, which runs runtime-built code and can stutter"
                )
            }
            Kind::MissingPinMameTimer => write!(
                f,
                "script uses a VPinMAME controller but the table has no timer named 'PinMAMETimer'"
            ),
            Kind::MissingVpmInit => {
                write!(
                    f,
                    "script uses a VPinMAME controller but never calls vpmInit"
                )
            }
            Kind::ScriptNameShadowsItem { name, kind, .. } => write!(
                f,
                "script declares {name:?}, which hides the {kind} of that name"
            ),
            Kind::RndWithoutRandomize => write!(
                f,
                "script uses Rnd without Randomize, so every run draws the same numbers"
            ),
            Kind::MissingPulseTimer => write!(
                f,
                "script uses 'vpmTimer' but the table has no timer named 'PulseTimer'"
            ),
            Kind::TimerWithoutHandler { item, interval } => {
                write!(f, "timer of {item:?} fires ")?;
                match interval {
                    -1 => write!(f, "every frame")?,
                    ms => write!(f, "every {ms} ms")?,
                }
                write!(f, " but the script has no {item}_Timer handler")
            }
            Kind::HandlerWithoutItem { name, .. } => write!(
                f,
                "script has the event handler {name:?} for an item that does not exist"
            ),
            Kind::BallIdAssigned { .. } => write!(
                f,
                "script assigns a ball's ID, which is read only; use UserValue instead"
            ),
        }
    }
}

impl Kind {
    /// The code of the check, the variant name in kebab case
    pub(super) fn code(&self) -> &'static str {
        match self {
            Kind::MissingImage { .. } => "missing-image",
            Kind::MissingImageWithFallback { .. } => "missing-image-with-fallback",
            Kind::MissingColorGradeImage { .. } => "missing-color-grade-image",
            Kind::MissingMaterial { .. } => "missing-material",
            Kind::MissingSurface { .. } => "missing-surface",
            Kind::MissingPartGroup { .. } => "missing-part-group",
            Kind::MissingCollectionItem { .. } => "missing-collection-item",
            Kind::DuplicateName { .. } => "duplicate-name",
            Kind::UnstorableText { .. } => "unstorable-text",
            Kind::NameTooLong { .. } => "name-too-long",
            Kind::ReservedName { .. } => "reserved-name",
            Kind::UnnamedItems { .. } => "unnamed-items",
            Kind::MissingTableName => "missing-table-name",
            Kind::BmpImage { .. } => "bmp-image",
            Kind::LossyColorGradeImage { .. } => "lossy-color-grade-image",
            Kind::ColorGradeLutUnusualSize { .. } => "color-grade-lut-unusual-size",
            Kind::MixedScriptLineEndings { .. } => "mixed-script-line-endings",
            Kind::UnusedFont { .. } => "unused-font",
            Kind::NonStandardFont { .. } => "non-standard-font",
            Kind::DeprecatedTableProperty { .. } => "deprecated-table-property",
            Kind::DeprecatedControllerProperty { .. } => "deprecated-controller-property",
            Kind::HugeMesh { .. } => "huge-mesh",
            Kind::UnusedImage { .. } => "unused-image",
            Kind::UnusedSound { .. } => "unused-sound",
            Kind::SameAssetData { .. } => "same-asset-data",
            Kind::UnusedMaterials { .. } => "unused-materials",
            Kind::MissingSound { .. } => "missing-sound",
            Kind::ImageDimensionMismatch { .. } => "image-dimension-mismatch",
            Kind::ImageExtensionMismatch { .. } => "image-extension-mismatch",
            Kind::ImageFormatUnknown { .. } => "image-format-unknown",
            Kind::UnreadableImage { .. } => "unreadable-image",
            Kind::GlassHeightInvalid { .. } => "glass-height-invalid",
            Kind::BallSphericalMapping => "ball-spherical-mapping",
            Kind::TextboxUsedForDmd { .. } => "textbox-used-for-dmd",
            Kind::FastTimer { .. } => "fast-timer",
            Kind::NegativeLightIntensity { .. } => "negative-light-intensity",
            Kind::OpaquePrimitiveTranslucency { .. } => "opaque-primitive-translucency",
            Kind::StaticPrimitiveInScript { .. } => "static-primitive-in-script",
            Kind::LightCannotFade { .. } => "light-cannot-fade",
            Kind::StereoTableSound { .. } => "stereo-table-sound",
            Kind::LargeScreenshot { .. } => "large-screenshot",
            Kind::ScriptParseError { .. } => "script-parse-error",
            Kind::MissingOptionExplicit => "missing-option-explicit",
            Kind::DuplicateProcedure { .. } => "duplicate-procedure",
            // the codes are from when these were reported once per table,
            // kept for whoever filters on them
            Kind::UnusedVariable { .. } => "unused-variables",
            Kind::UnusedLocalVariable { .. } => "unused-local-variables",
            Kind::ExecuteUsed => "execute-used",
            Kind::MissingPinMameTimer => "missing-pinmame-timer",
            Kind::MissingVpmInit => "missing-vpm-init",
            Kind::MissingPulseTimer => "missing-pulse-timer",
            Kind::ScriptNameShadowsItem { .. } => "script-name-shadows-item",
            Kind::RndWithoutRandomize => "rnd-without-randomize",
            Kind::TimerWithoutHandler { .. } => "timer-without-handler",
            Kind::HandlerWithoutItem { .. } => "handlers-without-item",
            Kind::BallIdAssigned { .. } => "ball-id-assigned",
        }
    }
}

/// Checks a table for consistency problems, returning an empty list when
/// nothing is wrong.
///
/// Name comparisons are case insensitive, like vpinball's own lookups.
pub fn audit(vpx: &VPX) -> Vec<Finding> {
    let items = ItemIndex::new(vpx);
    audit_kinds(vpx)
        .into_iter()
        .map(|kind| kind.finding(vpx, &items))
        .collect()
}

/// The checks, in their structured form
pub(crate) fn audit_kinds(vpx: &VPX) -> Vec<Kind> {
    let mut findings = Vec::new();

    references::check_references(vpx, &mut findings);
    names::check_names(vpx, &mut findings);
    references::check_collections(vpx, &mut findings);

    assets::check_fonts(vpx, &mut findings);
    assets::check_font_availability(vpx, &mut findings);

    code::check_deprecated_properties(vpx, &mut findings);
    code::check_deprecated_controller_properties(vpx, &mut findings);
    assets::check_unused_assets(vpx, &mut findings);
    assets::check_same_asset_data(vpx, &mut findings);
    assets::check_sound_calls(vpx, &mut findings);

    names::check_duplicate_names(vpx, &mut findings);
    names::check_table_name(vpx, &mut findings);

    assets::check_image_storage(vpx, &mut findings);
    code::check_line_endings(vpx, &mut findings);
    assets::check_screenshot(vpx, &mut findings);

    items::check_render_settings(vpx, &mut findings);
    for item in &vpx.gameitems {
        items::check_item_behavior(item, &mut findings);
        items::check_mesh_size(item, &mut findings);
    }
    items::check_primitive_translucency(vpx, &mut findings);
    assets::check_stereo_sounds(vpx, &mut findings);

    #[cfg(feature = "script-audit")]
    script::check(vpx, &mut findings);

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_clean_table_has_no_findings() {
        assert_eq!(audit_kinds(&clean_vpx()), Vec::new());
    }

    #[test]
    fn the_blank_template_has_dangling_default_references() {
        // vpinball's blank template keeps the default image names while the
        // images themselves are only present in the sample table
        let findings: Vec<Kind> = audit_kinds(&blank_vpx())
            .into_iter()
            // the template's score textbox uses a Windows only font, it carries
            // materials nothing uses and a decal without a name, and its
            // script draws random numbers without Randomize, plays the sample
            // table's sounds and has timer handlers for timers it does not have
            .filter(|finding| {
                !matches!(
                    finding,
                    Kind::NonStandardFont { .. }
                        | Kind::RndWithoutRandomize
                        | Kind::HandlerWithoutItem { .. }
                        | Kind::UnusedMaterials { .. }
                        | Kind::MissingSound { .. }
                        | Kind::UnnamedItems { .. }
                )
            })
            .collect();
        // 6 dangling references and the image nothing refers to
        assert_eq!(findings.len(), 7, "{findings:#?}");
        let count = |wanted: fn(&Kind) -> bool| findings.iter().filter(|f| wanted(f)).count();
        // the playfield image is the only one without a fallback
        assert_eq!(count(|f| matches!(f, Kind::MissingImage { .. })), 1);
        assert_eq!(
            count(|f| matches!(f, Kind::MissingImageWithFallback { .. })),
            3
        );
        assert_eq!(
            count(|f| matches!(f, Kind::MissingColorGradeImage { .. })),
            1
        );
        // pre 10.8 tables implicitly use the legacy ball mapping
        assert!(findings.contains(&Kind::BallSphericalMapping));
    }
}
