//! The VPZ table pack of Visual Pinball X: a table as a tree of JSON
//! documents and assets in their native formats, stored as a directory or
//! as a `.vpz` zip archive with the same content.
//!
//! The format is defined by vpinball in `docs/VPZ File Format.md`, which
//! marks it preliminary. This module models the pack layout:
//!
//! ```text
//! manifest.json                  pack identification
//! table.json                     the table definition
//! script.vbs                     the game script
//! parts/<name>.json              one document per scene node
//! collections/<name>.json        one document per collection
//! materials/<name>.json          one document per material
//! renderprobes/<name>.json       one document per render probe
//! images/<name>.<ext>            image bytes, with images/<name>.json
//! sounds/<name>.<ext>            sound bytes, with sounds/<name>.json
//! fonts/<name>.<ext>             font bytes, with fonts/<name>.json
//! meshes/<name>.glb              primitive mesh, glTF binary
//! ```
//!
//! The manifest and the asset sidecars are typed. The table and the scene
//! documents are kept as ordered JSON properties: their keys follow the
//! vpinball field maps, which may still change.
//!
//! A [`Vpz`] owns everything that ties files together, so the files are
//! derived from it on write:
//!
//! - entity names come from the file names, or from a `name` property
//!   when sanitizing or a collision changed the file name;
//! - the `parts`, `collections`, `materials` and `renderprobes` name lists
//!   and the `vbs_script` reference of `table.json` are rewritten from
//!   the model, and their order is the order of the model lists;
//! - a primitive's `mesh` property points to its [`Part::mesh`].
//!
//! Files no entity claims are kept in [`Vpz::other_files`].
//!
//! [`from_vpx`] gives the pack vpinball saves from a `.vpx` table, and
//! [`to_vpx`] the table vpinball saves after loading a pack.
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! let pack = vpin::vpz::read("table.vpz")?;
//! println!("{} parts", pack.parts.len());
//! vpin::vpz::write(&pack, "table-folder")?;
//! # Ok(())
//! # }
//! ```

mod convert;
mod json;
mod names;
mod read;
mod write;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Cursor, Read, Seek, Write};
use std::path::Path;

pub use convert::{from_vpx, to_vpx};
pub use names::sanitize_file_name;

/// The `file_format` of a pack manifest
pub const FILE_FORMAT: &str = "vpinball-pack";

/// The newest `file_version` this module reads and the one it writes
pub const FILE_VERSION: u32 = 1;

/// A VPZ table pack.
///
/// A pack without a table (`table: None`) is a partial pack, for example
/// a set of parts exported from the editor.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Vpz {
    /// `manifest.json`
    pub manifest: Manifest,
    /// `table.json`, without its `$type`
    pub table: Option<Document>,
    /// The game script, `script.vbs`
    pub script: Option<String>,
    /// The scene nodes in editor order (part z-order)
    pub parts: Vec<Part>,
    /// The collections in editor order
    pub collections: Vec<NamedDocument>,
    /// The materials in editor order
    pub materials: Vec<NamedDocument>,
    /// The render probes in editor order
    pub render_probes: Vec<NamedDocument>,
    /// The images, in file name order when read
    pub images: Vec<Asset<ImageSidecar>>,
    /// The sounds, in file name order when read
    pub sounds: Vec<Asset<SoundSidecar>>,
    /// The fonts, in file name order when read
    pub fonts: Vec<Asset<FontSidecar>>,
    /// Files of the pack no entity claims, by their pack path
    pub other_files: BTreeMap<String, Vec<u8>>,
}

/// `manifest.json`: identifies the pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`FILE_FORMAT`]
    pub file_format: String,
    /// The format version, [`FILE_VERSION`] for the current format
    pub file_version: u32,
    /// Table name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Table author
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Table version
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Table description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the pack was written, as vpinball formats it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_date: Option<String>,
    /// Properties this model does not know, in file order
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            file_format: FILE_FORMAT.to_string(),
            file_version: FILE_VERSION,
            name: None,
            author: None,
            version: None,
            description: None,
            save_date: None,
            extra: Map::new(),
        }
    }
}

/// A JSON document of the pack, without its `$type`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    /// The properties in file order
    pub properties: Map<String, Value>,
}

/// A collection, material or render probe document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NamedDocument {
    /// The unique name
    pub name: String,
    /// The properties in file order, without `$type` and `name`
    pub properties: Map<String, Value>,
}

/// A scene node: `parts/<name>.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// The unique name
    pub name: String,
    /// The `$type` of the document
    pub part_type: PartType,
    /// The properties in file order, without `$type`, `name` and `mesh`
    pub properties: Map<String, Value>,
    /// The glTF binary of a primitive mesh, `meshes/<name>.glb`
    pub mesh: Option<Vec<u8>>,
}

/// The `$type` of a part document.
///
/// A type this module does not know is kept as [`PartType::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
#[allow(missing_docs)]
pub enum PartType {
    Ball,
    Bumper,
    Decal,
    DispReel,
    Flasher,
    Flipper,
    Gate,
    HitTarget,
    Kicker,
    Light,
    LightSeq,
    PartGroup,
    Plunger,
    Primitive,
    Ramp,
    Rubber,
    Spinner,
    Surface,
    Textbox,
    Timer,
    Trigger,
    /// A type this module does not know, as written
    Other(String),
}

const PART_TYPES: [(PartType, &str); 21] = [
    (PartType::Ball, "ball"),
    (PartType::Bumper, "bumper"),
    (PartType::Decal, "decal"),
    (PartType::DispReel, "dispreel"),
    (PartType::Flasher, "flasher"),
    (PartType::Flipper, "flipper"),
    (PartType::Gate, "gate"),
    (PartType::HitTarget, "hittarget"),
    (PartType::Kicker, "kicker"),
    (PartType::Light, "light"),
    (PartType::LightSeq, "lightseq"),
    (PartType::PartGroup, "partgroup"),
    (PartType::Plunger, "plunger"),
    (PartType::Primitive, "primitive"),
    (PartType::Ramp, "ramp"),
    (PartType::Rubber, "rubber"),
    (PartType::Spinner, "spinner"),
    (PartType::Surface, "surface"),
    (PartType::Textbox, "textbox"),
    (PartType::Timer, "timer"),
    (PartType::Trigger, "trigger"),
];

impl PartType {
    /// The `$type` value
    pub fn as_str(&self) -> &str {
        match self {
            PartType::Other(name) => name,
            known => PART_TYPES
                .iter()
                .find(|(part_type, _)| part_type == known)
                .map(|(_, name)| *name)
                .unwrap_or_default(),
        }
    }
}

impl From<String> for PartType {
    fn from(name: String) -> Self {
        PART_TYPES
            .iter()
            .find(|(_, known)| *known == name)
            .map(|(part_type, _)| part_type.clone())
            .unwrap_or(PartType::Other(name))
    }
}

impl From<PartType> for String {
    fn from(part_type: PartType) -> Self {
        part_type.as_str().to_string()
    }
}

impl fmt::Display for PartType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An image, sound or font: the file bytes and the sidecar document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Asset<S> {
    /// The unique name
    pub name: String,
    /// The extension of the data file without the dot, for example `png`,
    /// empty for a file without one
    pub extension: String,
    /// The file bytes, untouched
    pub data: Vec<u8>,
    /// The sidecar document; a data file without one reads as the default
    pub sidecar: S,
}

/// `images/<name>.json`: the image properties assigned at import.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImageSidecar {
    /// The path the image was imported from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_path: Option<String>,
    /// Width in pixels
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height in pixels
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Alpha test threshold on a 0..255 scale, negative when disabled
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_test: Option<f64>,
    /// MD5 of the image data, 32 hex digits
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Whether the image has no transparency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque: Option<bool>,
    /// Legacy binary sharing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<i64>,
    /// Properties this model does not know, in file order
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `sounds/<name>.json`: the sound properties assigned at import.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SoundSidecar {
    /// The path the sound was imported from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_path: Option<String>,
    /// Where the sound plays
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_target: Option<OutputTarget>,
    /// Volume offset
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_offset: Option<i32>,
    /// -100 is full left, +100 is full right
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_right_offset: Option<i32>,
    /// -100 is full rear, +100 is full front
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rear_front_offset: Option<i32>,
    /// Properties this model does not know, in file order
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The `output_target` of a sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum OutputTarget {
    /// `playfield`
    Playfield,
    /// `backglass`
    Backglass,
    /// A value this module does not know, as written
    Other(String),
}

impl From<String> for OutputTarget {
    fn from(value: String) -> Self {
        match value.as_str() {
            "playfield" => OutputTarget::Playfield,
            "backglass" => OutputTarget::Backglass,
            _ => OutputTarget::Other(value),
        }
    }
}

impl From<OutputTarget> for String {
    fn from(target: OutputTarget) -> Self {
        match target {
            OutputTarget::Playfield => "playfield".to_string(),
            OutputTarget::Backglass => "backglass".to_string(),
            OutputTarget::Other(value) => value,
        }
    }
}

/// `fonts/<name>.json`: the font properties assigned at import.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FontSidecar {
    /// The path the font was imported from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_path: Option<String>,
    /// Properties this model does not know, in file order
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Builds a pack from its files, keyed by their path in the pack with `/`
/// separators.
pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> io::Result<Vpz> {
    read::from_files(files)
}

/// The files of a pack, keyed by their path in the pack with `/`
/// separators.
pub fn to_files(vpz: &Vpz) -> io::Result<BTreeMap<String, Vec<u8>>> {
    write::to_files(vpz)
}

/// Reads a pack from a `.vpz` zip archive.
pub fn read_zip<R: Read + Seek>(reader: R) -> io::Result<Vpz> {
    let mut archive = zip::ZipArchive::new(reader).map_err(io::Error::other)?;
    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(io::Error::other)?;
        if entry.is_dir() {
            continue;
        }
        let path = entry
            .enclosed_name()
            .and_then(|path| path.to_str().map(|path| path.replace('\\', "/")))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsafe zip entry name {:?}", entry.name()),
                )
            })?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data)?;
        files.insert(path, data);
    }
    from_files(files)
}

/// Writes a pack as a `.vpz` zip archive.
pub fn write_zip<W: Write + Seek>(vpz: &Vpz, writer: W) -> io::Result<W> {
    let files = to_files(vpz)?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(files.values().any(|data| data.len() >= u32::MAX as usize));
    let mut zip = zip::ZipWriter::new(writer);
    // the manifest first, so a reader can identify the pack early
    let manifest = files.get_key_value(read::MANIFEST);
    for (path, data) in manifest
        .into_iter()
        .chain(files.iter().filter(|(path, _)| *path != read::MANIFEST))
    {
        zip.start_file(path.as_str(), options)
            .map_err(io::Error::other)?;
        zip.write_all(data)?;
    }
    zip.finish().map_err(io::Error::other)
}

/// Reads a pack from a directory.
pub fn read_dir<P: AsRef<Path>>(dir: P) -> io::Result<Vpz> {
    let dir = dir.as_ref();
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(dir)
                .ok()
                .and_then(|relative| {
                    relative
                        .components()
                        .map(|component| component.as_os_str().to_str())
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} is not a UTF-8 pack path", path.display()),
                    )
                })?
                .join("/");
            files.insert(relative, fs::read(&path)?);
        }
    }
    from_files(files)
}

/// Writes a pack to a directory, which must not exist yet or be empty.
pub fn write_dir<P: AsRef<Path>>(vpz: &Vpz, dir: P) -> io::Result<()> {
    let dir = dir.as_ref();
    if dir.exists() && fs::read_dir(dir)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is not empty", dir.display()),
        ));
    }
    for (path, data) in to_files(vpz)? {
        let target = dir.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, data)?;
    }
    Ok(())
}

/// Reads a pack from a directory or a `.vpz` zip archive.
pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vpz> {
    let path = path.as_ref();
    if path.is_dir() {
        read_dir(path)
    } else {
        read_zip(io::BufReader::new(fs::File::open(path)?))
    }
}

/// Writes a pack the way vpinball picks the form: a directory when the
/// path is an existing directory or has no extension, a `.vpz` zip
/// archive otherwise.
pub fn write<P: AsRef<Path>>(vpz: &Vpz, path: P) -> io::Result<()> {
    let path = path.as_ref();
    if path.is_dir() || (!path.exists() && path.extension().is_none()) {
        write_dir(vpz, path)
    } else {
        let file = write_zip(vpz, io::BufWriter::new(fs::File::create(path)?))?;
        file.into_inner().map_err(|e| e.into_error())?.sync_all()
    }
}

/// Reads a pack from the bytes of a `.vpz` zip archive.
pub fn from_zip_bytes(bytes: &[u8]) -> io::Result<Vpz> {
    read_zip(Cursor::new(bytes))
}

/// The bytes of a pack as a `.vpz` zip archive.
pub fn to_zip_bytes(vpz: &Vpz) -> io::Result<Vec<u8>> {
    Ok(write_zip(vpz, Cursor::new(Vec::new()))?.into_inner())
}

#[cfg(test)]
mod tests;
