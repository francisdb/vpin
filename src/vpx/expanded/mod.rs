//! Expanded VPX directory format for easier editing and version control.
//!
//! This module provides functions to extract VPX files into a directory structure
//! with separate JSON and binary files, and reassemble them back into VPX format.
//!
//! # Primitive Mesh Formats
//!
//! Primitive mesh data can be exported in three formats:
//! - **OBJ** (default): Text-based Wavefront OBJ format, human-readable
//! - **GLB**: Binary GLTF format, significantly faster for large meshes
//! - **GLTF**: JSON + external BIN buffer for tooling-friendly workflows
//!
//! Use [`write()`] with [`ExpandOptions`] to specify the format and other
//! options. All formats are supported for reading, with OBJ checked first
//! for backward compatibility.
//!
//! # Writing part of a table
//!
//! [`ExpandOptions::filter`] takes a predicate over the path of each file
//! relative to the expanded directory, for example `gamedata.json` or
//! `images/playfield.webp`. Files the predicate rejects are not written,
//! and the work to produce them, decoding bitmaps and decompressing
//! meshes, is skipped. Directories only come into being for files that
//! are written. Index files such as `images.json` still list every entry,
//! so a partial directory documents what was left out; it cannot be read
//! back as a table since the reader expects the files its indexes name.

mod fonts;
mod gameitems;
mod images;
mod materials;
mod metadata;
mod primitives;
mod sounds;
pub(crate) mod util;

use crate::filesystem::{FileSystem, MemoryFileSystem, RealFileSystem};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gltf::{GltfContainer, write_gltf};
use crate::vpx::material::Material;
use crate::vpx::obj::{VpxFace, write_obj};
use crate::vpx::{VPX, Version};
use log::{info, warn};
pub use primitives::BytesMutExt;
use serde::Serialize;
use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Format for exporting primitive mesh data
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveMeshFormat {
    /// Wavefront OBJ format (text-based, human-readable)
    #[default]
    Obj,
    /// Binary GLTF format (GLB) - more efficient for large meshes
    /// TODO: Consider packing animation frames into a single GLB using GLTF animations
    /// TODO: Consider adding compression support for GLB files
    Glb,
    /// GLTF JSON + external BIN buffer
    Gltf,
}

/// Options for expanding VPX files to directory format.
///
/// Use [`ExpandOptions::new`] to create a new instance with default settings,
/// then chain configuration methods to customize behavior.
///
/// # Examples
///
/// ```
/// use vpin::vpx::expanded::{ExpandOptions, PrimitiveMeshFormat};
///
/// // Default options (OBJ format, no derived meshes)
/// let options = ExpandOptions::new();
///
/// // Custom options with GLB format and derived mesh generation
/// let options = ExpandOptions::new()
///     .mesh_format(PrimitiveMeshFormat::Glb)
///     .generate_derived_meshes(true);
/// ```
#[derive(Clone)]
pub struct ExpandOptions {
    mesh_format: PrimitiveMeshFormat,
    generate_derived_meshes: bool,
    filter: Option<PathFilter>,
}

/// Decides per file, by its path relative to the expanded directory,
/// whether it is written
type PathFilter = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

impl std::fmt::Debug for ExpandOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpandOptions")
            .field("mesh_format", &self.mesh_format)
            .field("generate_derived_meshes", &self.generate_derived_meshes)
            .field("filter", &self.filter.as_ref().map(|_| "<predicate>"))
            .finish()
    }
}

impl ExpandOptions {
    /// Creates a new set of options with default settings.
    ///
    /// Defaults:
    /// - `mesh_format`: [`PrimitiveMeshFormat::Obj`]
    /// - `generate_derived_meshes`: `false`
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the format for primitive mesh data.
    ///
    /// Default: [`PrimitiveMeshFormat::Obj`]
    pub fn mesh_format(mut self, format: PrimitiveMeshFormat) -> Self {
        self.mesh_format = format;
        self
    }

    /// Sets whether to generate derived meshes for walls, ramps, and rubbers.
    ///
    /// When enabled, mesh files are generated from the drag points of these game items.
    /// This is useful for visualization tools but increases extraction time and disk usage.
    ///
    /// Default: `false`
    pub fn generate_derived_meshes(mut self, generate: bool) -> Self {
        self.generate_derived_meshes = generate;
        self
    }

    /// Only writes the files whose path, relative to the expanded
    /// directory, satisfies the predicate (`true` keeps, like
    /// [`Iterator::filter`]). See the [module documentation](self) for
    /// what a partial directory holds.
    ///
    /// ```
    /// use std::path::Path;
    /// use vpin::vpx::expanded::ExpandOptions;
    ///
    /// // everything except the image and sound data
    /// let options = ExpandOptions::new().filter(|path: &Path| {
    ///     !path.starts_with("images") && !path.starts_with("sounds")
    /// });
    /// ```
    ///
    /// Paths are compared by component, as [`Path::starts_with`] does:
    /// `gameitems.json` does not start with `gameitems`, so rejecting the
    /// `gameitems` directory keeps its index file.
    pub fn filter(mut self, predicate: impl Fn(&Path) -> bool + Send + Sync + 'static) -> Self {
        self.filter = Some(Arc::new(predicate));
        self
    }

    /// Whether the file at this path relative to the expanded directory
    /// is written
    pub(super) fn should_write(&self, relative_path: &Path) -> bool {
        self.filter
            .as_ref()
            .is_none_or(|predicate| predicate(relative_path))
    }

    /// Returns the configured mesh format.
    pub(super) fn get_mesh_format(&self) -> PrimitiveMeshFormat {
        self.mesh_format
    }

    /// Returns whether derived mesh generation is enabled.
    pub(super) fn should_generate_derived_meshes(&self) -> bool {
        self.generate_derived_meshes
    }
}

impl Default for ExpandOptions {
    fn default() -> Self {
        Self {
            mesh_format: PrimitiveMeshFormat::Obj,
            generate_derived_meshes: false,
            filter: None,
        }
    }
}

/// The directory game item files go in
pub(super) const GAMEITEMS_DIR: &str = "gameitems";
/// The directory image files go in
pub(super) const IMAGES_DIR: &str = "images";
/// The directory sound files go in
pub(super) const SOUNDS_DIR: &str = "sounds";
/// The directory font files go in
pub(super) const FONTS_DIR: &str = "fonts";

/// Where an expanded write goes: the target directory, the file system
/// and the options. Every writer asks it before producing a file, so a
/// filtered out file is neither built nor written, and a directory only
/// comes into being when a file goes into it.
///
/// Paths are relative to the expanded directory, the same ones the
/// [`ExpandOptions::filter`] predicate sees.
pub(super) struct Output<'a> {
    root: &'a Path,
    fs: &'a dyn FileSystem,
    options: &'a ExpandOptions,
}

impl<'a> Output<'a> {
    pub(super) fn new(root: &'a Path, fs: &'a dyn FileSystem, options: &'a ExpandOptions) -> Self {
        Self { root, fs, options }
    }

    pub(super) fn options(&self) -> &ExpandOptions {
        self.options
    }

    /// The absolute path of a file, for messages
    pub(super) fn path(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }

    pub(super) fn exists(&self, relative: &Path) -> bool {
        self.fs.exists(&self.path(relative))
    }

    /// Whether this file is produced at all; callers skip the work behind
    /// a file that is not
    pub(super) fn wants(&self, relative: &Path) -> bool {
        self.options.should_write(relative)
    }

    /// The absolute path of a wanted file with its directory in place,
    /// `None` when the file is filtered out
    pub(super) fn prepare(&self, relative: &Path) -> io::Result<Option<PathBuf>> {
        if !self.wants(relative) {
            return Ok(None);
        }
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            self.fs.create_dir_all(parent)?;
        }
        Ok(Some(path))
    }

    /// A buffered writer for the file, `None` when it is filtered out.
    /// Callers flush before dropping it so write errors surface.
    pub(super) fn create_buffered(&self, relative: &Path) -> io::Result<Option<Box<dyn Write>>> {
        match self.prepare(relative)? {
            Some(path) => Ok(Some(self.fs.create_buffered_file(&path)?)),
            None => Ok(None),
        }
    }

    /// Writes the bytes, nothing when the file is filtered out
    pub(super) fn write(&self, relative: &Path, data: &[u8]) -> io::Result<()> {
        match self.prepare(relative)? {
            Some(path) => self.fs.write_file(&path, data),
            None => Ok(()),
        }
    }

    /// Writes the bytes the closure produces. The closure only runs when
    /// the file is wanted, so the caller does not need to ask first.
    pub(super) fn write_with<'b>(
        &self,
        relative: &Path,
        produce: impl FnOnce() -> Result<Cow<'b, [u8]>, WriteError>,
    ) -> Result<(), WriteError> {
        let Some(path) = self.prepare(relative)? else {
            return Ok(());
        };
        let data = produce()?;
        self.fs.write_file(&path, &data)?;
        Ok(())
    }

    /// Writes the value as pretty printed JSON. The value is only built
    /// when the file is written, so a filtered out index costs nothing.
    pub(super) fn write_json<T: Serialize>(
        &self,
        relative: &Path,
        value: impl FnOnce() -> T,
    ) -> Result<(), WriteError> {
        let Some(mut file) = self.create_buffered(relative)? else {
            return Ok(());
        };
        serde_json::to_writer_pretty(&mut file, &value())?;
        file.flush()?;
        Ok(())
    }

    /// Writes a mesh in the configured format, nothing when the file is
    /// filtered out
    pub(super) fn write_mesh(
        &self,
        relative: &Path,
        name: &str,
        vertices: &[VertexWrapper],
        indices: &[VpxFace],
    ) -> Result<(), WriteError> {
        let Some(path) = self.prepare(relative)? else {
            return Ok(());
        };
        let result = match self.options.get_mesh_format() {
            PrimitiveMeshFormat::Obj => write_obj(name, vertices, indices, &path, self.fs),
            PrimitiveMeshFormat::Glb => {
                write_gltf(name, vertices, indices, &path, GltfContainer::Glb, self.fs)
            }
            PrimitiveMeshFormat::Gltf => {
                write_gltf(name, vertices, indices, &path, GltfContainer::Gltf, self.fs)
            }
        };
        result.map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))
    }
}

#[derive(Debug)]
pub enum WriteError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl Error for WriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WriteError::Io(error) => Some(error),
            WriteError::Json(error) => Some(error),
        }
    }
}

impl Display for WriteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::Io(error) => write!(f, "IO error: {error}"),
            WriteError::Json(error) => write!(f, "JSON error: {error}"),
        }
    }
}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        WriteError::Io(error)
    }
}

impl From<serde_json::Error> for WriteError {
    fn from(error: serde_json::Error) -> Self {
        WriteError::Json(error)
    }
}

/// Write VPX to expanded directory format
pub fn write<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    options: &ExpandOptions,
) -> Result<(), WriteError> {
    write_fs(vpx, expanded_dir, options, &RealFileSystem)
}

/// Write VPX to expanded directory format using the provided file system
pub fn write_fs<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    options: &ExpandOptions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    info!("=== Starting VPX extraction process ===");
    info!("Target directory: {}", expanded_dir.as_ref().display());
    let out = Output::new(expanded_dir.as_ref(), fs, options);

    out.write(
        Path::new("version.txt"),
        vpx.version.to_u32_string().as_bytes(),
    )?;
    info!("✓ Version file written");

    if let Some(screenshot) = &vpx.info.screenshot {
        out.write(Path::new("screenshot.png"), screenshot)?;
        info!("✓ Screenshot written");
    } else {
        info!("✓ No screenshot to write");
    }

    info!("Writing table info...");
    metadata::write_info(&vpx.info, &vpx.custominfotags, &out)?;
    info!("✓ Table info written");

    info!("Writing collections...");
    metadata::write_collections(&vpx.collections, &out)?;
    info!("✓ {} Collections written", vpx.collections.len());

    info!("Writing game items...");
    let table_dims = crate::vpx::TableDimensions::from_gamedata(&vpx.gamedata);
    gameitems::write_gameitems(&vpx.gameitems, &table_dims, &out)?;
    info!("✓ {} Game items written", vpx.gameitems.len());

    info!("Writing images...");
    images::write_images(&vpx.images, &out)?;
    info!("✓ {} Images written", vpx.images.len());

    info!("Writing sounds...");
    sounds::write_sounds(&vpx.sounds, &out)?;
    info!("✓ {} Sounds written", vpx.sounds.len());

    info!("Writing fonts...");
    fonts::write_fonts(&vpx.fonts, &out)?;
    info!("✓ {} Fonts written", vpx.fonts.len());

    info!("Writing game data...");
    metadata::write_game_data(&vpx.gamedata, &out)?;
    info!("✓ Game data written");

    if let Some(materials) = &vpx.gamedata.materials {
        info!("Writing materials...");
        materials::write_materials(materials, &out)?;
        info!("✓ Materials written");
        validate_material_conversion(&vpx, materials);
    } else {
        info!("Writing legacy materials...");
        materials::write_legacy_materials(
            &vpx.gamedata.materials_old,
            vpx.gamedata.materials_physics_old.as_ref(),
            &out,
        )?;
        info!("✓ Legacy materials written");
    }

    info!("Writing render probes...");
    metadata::write_renderprobes(vpx.gamedata.render_probes.as_ref(), &out)?;
    info!("✓ Render probes written");

    info!("=== VPX extraction process completed successfully ===");
    Ok(())
}

/// Validate that materials in the old and new formats match, and log warnings for any discrepancies.
///
/// We have seen files edited by 10.8 and afterward by 10.7 to be messed up.
fn validate_material_conversion(vpx: &&VPX, materials: &Vec<Material>) {
    for old_material in &vpx.gamedata.materials_old {
        if !materials.iter().any(|m| m.name == old_material.name) {
            warn!(
                "Material '{}' exists in the old format but not in the 10.8 format.",
                old_material.name
            );
        }
    }
    for material in materials {
        if !vpx
            .gamedata
            .materials_old
            .iter()
            .any(|m| m.name == material.name)
        {
            warn!(
                "Material '{}' exists in the 10.8 format but not in the old format.",
                material.name
            );
        }
    }
}

pub fn read<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<VPX> {
    read_fs(expanded_dir, &RealFileSystem)
}

pub fn read_fs<P: AsRef<Path>>(expanded_dir: &P, fs: &dyn FileSystem) -> io::Result<VPX> {
    info!("=== Starting VPX assembly process ===");
    // Storage layers like Safari/WebKit OPFS or macOS HFS+ can change the
    // Unicode normalization form of file names between write and lookup,
    // breaking the byte-exact match with names recorded in the index json
    // files. Retry lookups with NFC/NFD-normalized paths to compensate.
    // See https://github.com/francisdb/vpin/issues/355
    let normalizing_fs = util::NormalizingFileSystem::new(fs);
    let fs: &dyn FileSystem = &normalizing_fs;
    let version_path = expanded_dir.as_ref().join("version.txt");
    if !fs.exists(&version_path) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Version file not found: {}", version_path.display()),
        ));
    }
    let mut version_file = fs.open_file(&version_path)?;
    let mut version_string = String::new();
    version_file.read_to_string(&mut version_string)?;
    let version = Version::parse(&version_string).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Could not parse version {}: {}", version_string, e),
        )
    })?;

    let screenshot_path = expanded_dir.as_ref().join("screenshot.png");
    let screenshot = if fs.exists(&screenshot_path) {
        let screenshot = fs.read_file(&screenshot_path)?;
        Some(screenshot)
    } else {
        None
    };

    info!("Reading table info...");
    let (info, custominfotags) = metadata::read_info(expanded_dir, screenshot, fs)?;
    info!("✓ Table info read");

    info!("Reading collections...");
    let collections = metadata::read_collections(expanded_dir, fs)?;
    info!("✓ {} Collections read", collections.len());

    info!("Reading game items...");
    let gameitems = gameitems::read_gameitems(expanded_dir, fs)?;
    info!("✓ {} Game items read", gameitems.len());

    info!("Reading images...");
    let images = images::read_images(expanded_dir, fs)?;
    info!("✓ {} Images read", images.len());

    info!("Reading sounds...");
    let sounds = sounds::read_sounds(expanded_dir, fs)?;
    info!("✓ {} Sounds read", sounds.len());

    info!("Reading fonts...");
    let fonts = fonts::read_fonts(expanded_dir, fs)?;
    info!("✓ {} Fonts read", fonts.len());

    info!("Reading game data...");
    let mut gamedata = metadata::read_game_data(expanded_dir, fs)?;
    gamedata.collections_size = collections.len() as u32;
    gamedata.gameitems_size = gameitems.len() as u32;
    gamedata.images_size = images.len() as u32;
    gamedata.sounds_size = sounds.len() as u32;
    gamedata.fonts_size = fonts.len() as u32;
    let materials_opt = materials::read_materials(expanded_dir, fs)?;
    match materials_opt {
        Some(materials) => {
            use crate::vpx::material::{SaveMaterial, SavePhysicsMaterial};
            gamedata.materials_old = materials.iter().map(SaveMaterial::from).collect();
            gamedata.materials_physics_old =
                Some(materials.iter().map(SavePhysicsMaterial::from).collect());
            gamedata.materials_size = materials.len() as u32;
            gamedata.materials = Some(materials);
        }
        None => {
            if let Some(old_materials) = materials::read_old_materials(expanded_dir, fs)? {
                gamedata.materials_old = old_materials;
                gamedata.materials_physics_old =
                    materials::read_old_materials_physics(expanded_dir, fs)?;
                gamedata.materials_size = gamedata.materials_old.len() as u32;
            } else {
                warn!("No materials found");
            }
        }
    }
    gamedata.render_probes = metadata::read_renderprobes(expanded_dir, fs)?;
    info!("✓ Game data read");

    // apply what vpinball's editor enforces on names, so the assembled
    // table is one it could have written. References are left alone:
    // published tables have references longer than any stored name,
    // which vpinball resolves to its default material, and the audit
    // reports them as missing
    // vpinball takes the 10.8 material records over the old ones when the
    // file version is 10.8 or newer and they are present. That record
    // holds any name and the player looks materials up by it, so a longer
    // one only loses the records older versions read
    match &gamedata.materials {
        Some(materials) if version >= Version::new(1080) => {
            materials::warn_material_names(materials.iter().map(|material| material.name.as_str()))
        }
        _ => materials::check_material_names(
            gamedata
                .materials_old
                .iter()
                .map(|material| material.name.as_str()),
        )?,
    }

    let vpx = VPX {
        custominfotags,
        info,
        version,
        gamedata,
        gameitems,
        images,
        sounds,
        fonts,
        collections,
    };
    info!("=== VPX assembly process completed successfully ===");
    Ok(vpx)
}

pub fn extract_directory_list(vpx_file_path: &Path) -> io::Result<Vec<String>> {
    let vpx = crate::vpx::read(vpx_file_path)?;
    let fs = MemoryFileSystem::default();

    // take the file name without extension as the directory name
    let expanded_dir = Path::new(
        vpx_file_path
            .file_stem()
            .unwrap_or_else(|| std::ffi::OsStr::new("expanded")),
    );

    // default options with no derived meshes and OBJ format
    let options = ExpandOptions::new()
        .generate_derived_meshes(false)
        .mesh_format(PrimitiveMeshFormat::Obj);
    write_fs(&vpx, &expanded_dir, &options, &fs).map_err(io::Error::other)?;

    let mut files = fs.list_files();
    files.sort();
    Ok(files)
}

/// Generate the file name for a generated mesh file
pub(super) fn generated_mesh_file_name(
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> String {
    let extension = mesh_file_extension(mesh_format);
    format!("{json_file_name}-generated.{extension}")
}

/// The file extension mesh files get in this format
pub(super) fn mesh_file_extension(mesh_format: PrimitiveMeshFormat) -> &'static str {
    match mesh_format {
        PrimitiveMeshFormat::Obj => "obj",
        PrimitiveMeshFormat::Glb => "glb",
        PrimitiveMeshFormat::Gltf => "gltf",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::MemoryFileSystem;
    use crate::vpx::collection::Collection;
    use crate::vpx::font::FontData;
    use crate::vpx::gamedata::GameData;
    use crate::vpx::gameitem;
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::primitive::Primitive;
    use crate::vpx::image::{ImageData, ImageDataBits, ImageDataJpeg};
    use crate::vpx::sound::{OutputTarget, SoundData, WaveForm};
    use crate::vpx::tableinfo::TableInfo;
    use crate::vpx::version::Version;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // Encoded data for 2x2 argb with alpha always 0xFF because the vpinball
    // bmp export does not support alpha channel.
    // See lzw_writer tests on what colors these are.
    const LZW_COMPRESSED_DATA: [u8; 14] =
        [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4];

    #[test]
    fn derived_meshes_use_the_table_dimensions() {
        // Flasher UVs are world-aligned to the table bounds, so the
        // derived mesh must change when the table size does; the bounds
        // used to be hardcoded to 952x2162.
        use crate::vpx::gameitem::dragpoint::DragPoint;
        use crate::vpx::gameitem::flasher::Flasher;

        fn write(right: f32, bottom: f32) -> Vec<u8> {
            let drag_point = |x: f32, y: f32| DragPoint {
                x,
                y,
                ..Default::default()
            };
            let flasher = Flasher {
                name: "Flasher1".to_string(),
                height: 50.0,
                // World alignment maps UVs to table coordinates - the
                // mode that actually consumes the table bounds.
                image_alignment:
                    crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment::World,
                drag_points: vec![
                    drag_point(100.0, 100.0),
                    drag_point(100.0, 400.0),
                    drag_point(400.0, 400.0),
                    drag_point(400.0, 100.0),
                ],
                ..Default::default()
            };
            let mut vpx = VPX::default();
            vpx.gamedata.right = right;
            vpx.gamedata.bottom = bottom;
            vpx.gameitems = vec![GameItemEnum::Flasher(flasher)];

            let fs = MemoryFileSystem::new();
            let options = ExpandOptions::new().generate_derived_meshes(true);
            write_fs(&vpx, &"/vpx".to_string(), &options, &fs).unwrap();
            let path = fs
                .list_files()
                .into_iter()
                .find(|f| f.ends_with("-generated.obj"))
                .expect("derived flasher mesh should be written");
            fs.get_file(&path).unwrap()
        }

        let default_size = write(952.0, 2162.0);
        let double_size = write(1904.0, 4324.0);
        assert_ne!(
            default_size, double_size,
            "derived mesh should depend on the table dimensions"
        );
    }

    #[test]
    fn a_long_material_name_is_kept_in_a_10_8_table() -> TestResult {
        use crate::vpx::gameitem::wall::Wall;
        use crate::vpx::material::Material;
        let fs = MemoryFileSystem::default();
        let mut vpx = VPX {
            version: Version::new(1080),
            ..Default::default()
        };
        let mut material = Material::default();
        material.name = "a".repeat(33);
        vpx.gamedata.materials = Some(vec![material]);
        vpx.gameitems = vec![GameItemEnum::Wall(Wall {
            name: "Wall1".to_string(),
            top_material: "a".repeat(33),
            ..Default::default()
        })];
        write_fs(&vpx, &"/vpx".to_string(), &ExpandOptions::new(), &fs)?;

        // published tables have such names and vpinball plays them
        let read = read_fs(&"/vpx".to_string(), &fs)?;
        assert_eq!(read.gamedata.materials.unwrap()[0].name, "a".repeat(33));
        let GameItemEnum::Wall(wall) = &read.gameitems[0] else {
            panic!("expected a wall");
        };
        assert_eq!(wall.top_material, "a".repeat(33));
        Ok(())
    }

    #[test]
    fn a_material_name_the_old_record_cannot_store_is_rejected() -> TestResult {
        use crate::vpx::material::SaveMaterial;
        let fs = MemoryFileSystem::default();
        let mut vpx = VPX {
            version: Version::new(1072),
            ..Default::default()
        };
        vpx.gamedata.materials_old = vec![SaveMaterial {
            name: "a".repeat(32),
            ..Default::default()
        }];
        write_fs(&vpx, &"/vpx".to_string(), &ExpandOptions::new(), &fs)?;

        let error = read_fs(&"/vpx".to_string(), &fs).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            format!(
                "Material name {:?} is 32 bytes, vpinball stores at most 31",
                "a".repeat(32)
            )
        );
        Ok(())
    }

    #[test]
    fn a_long_collection_name_is_cut_like_vpinball_does() -> TestResult {
        let fs = MemoryFileSystem::default();
        let mut vpx = VPX::default();
        vpx.collections.push(Collection {
            name: "c".repeat(32),
            items: Vec::new(),
            fire_events: false,
            stop_single_events: false,
            group_elements: false,
        });
        write_fs(&vpx, &"/vpx".to_string(), &ExpandOptions::new(), &fs)?;

        let read = read_fs(&"/vpx".to_string(), &fs)?;
        assert_eq!(read.collections[0].name, "c".repeat(31));
        Ok(())
    }

    #[test]
    fn an_uncompressed_mesh_is_assembled_uncompressed() -> TestResult {
        // a triangle as builds from before 2015 stored it: raw records
        let mut vertices = Vec::new();
        for (x, y) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0)] {
            let mut vertex = [0u8; 32];
            vertex[0..4].copy_from_slice(&x.to_le_bytes());
            vertex[4..8].copy_from_slice(&y.to_le_bytes());
            vertex[20..24].copy_from_slice(&1.0f32.to_le_bytes());
            vertices.extend_from_slice(&vertex);
        }
        let indices: Vec<u8> = [0u16, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect();
        let primitive = Primitive {
            name: "Old".to_string(),
            use_3d_mesh: true,
            num_vertices: Some(3),
            vertices_data: Some(vertices.clone()),
            num_indices: Some(3),
            indices_data: Some(indices.clone()),
            ..Default::default()
        };
        let vpx = VPX {
            gameitems: vec![GameItemEnum::Primitive(Box::new(primitive))],
            ..Default::default()
        };
        let fs = MemoryFileSystem::default();
        write_fs(&vpx, &"/vpx".to_string(), &ExpandOptions::new(), &fs)?;

        let read = read_fs(&"/vpx".to_string(), &fs)?;
        let GameItemEnum::Primitive(back) = &read.gameitems[0] else {
            panic!("expected a primitive");
        };
        assert_eq!(back.vertices_data, Some(vertices));
        assert_eq!(back.indices_data, Some(indices));
        assert_eq!(back.compressed_vertices_data, None);
        assert_eq!(back.compressed_indices_data, None);
        Ok(())
    }

    #[test]
    fn test_read_write() -> TestResult {
        let fs = MemoryFileSystem::default();
        let version = Version::new(1074);
        let screenshot = vec![0, 1, 2, 3];

        let mut bumper: gameitem::bumper::Bumper = Faker.fake();
        bumper.name = "test bumper".to_string();
        let mut decal: gameitem::decal::Decal = Faker.fake();
        decal.name = "test decal".to_string();
        let mut flasher: gameitem::flasher::Flasher = Faker.fake();
        flasher.name = "test flasher".to_string();
        let mut flipper: gameitem::flipper::Flipper = Faker.fake();
        flipper.name = "test flipper".to_string();
        let mut gate: gameitem::gate::Gate = Faker.fake();
        gate.name = "test gate".to_string();
        let mut hittarget: gameitem::hittarget::HitTarget = Faker.fake();
        hittarget.name = "test hittarget".to_string();
        let mut kicker: gameitem::kicker::Kicker = Faker.fake();
        kicker.name = "test kicker".to_string();
        let mut light: gameitem::light::Light = Faker.fake();
        light.name = "test light".to_string();
        let mut light_sequencer: gameitem::lightsequencer::LightSequencer = Faker.fake();
        light_sequencer.name = "test light sequencer".to_string();
        let mut plunger: gameitem::plunger::Plunger = Faker.fake();
        plunger.name = "test plunger".to_string();
        let mut primitive: Primitive = Faker.fake();
        primitive.name = "test primitive".to_string();
        // keep the vertices and indices empty to work around compression errors on fake data
        primitive.use_3d_mesh = false;
        primitive.num_vertices = None;
        primitive.num_indices = None;
        primitive.compressed_vertices_len = None;
        primitive.compressed_vertices_data = None;
        primitive.compressed_indices_len = None;
        primitive.compressed_indices_data = None;
        primitive.vertices_data = None;
        primitive.indices_data = None;
        primitive.compressed_animation_vertices_len = None;
        primitive.compressed_animation_vertices_data = None;
        let mut ramp: gameitem::ramp::Ramp = Faker.fake();
        ramp.name = "test ramp".to_string();
        let mut reel: gameitem::reel::Reel = Faker.fake();
        reel.name = "test reel".to_string();
        let mut rubber: gameitem::rubber::Rubber = Faker.fake();
        rubber.name = "test rubber".to_string();
        let mut spinner: gameitem::spinner::Spinner = Faker.fake();
        spinner.name = "test spinner".to_string();
        let mut textbox: gameitem::textbox::TextBox = Faker.fake();
        textbox.name = "test textbox".to_string();
        let mut timer: gameitem::timer::Timer = Faker.fake();
        timer.name = "test timer".to_string();
        let mut trigger: gameitem::trigger::Trigger = Faker.fake();
        trigger.name = "test trigger".to_string();
        let mut wall: gameitem::wall::Wall = Faker.fake();
        wall.name = "test wall".to_string();

        let mut gamedata = GameData::default();
        gamedata.code.string = r#"debug.print "Hello world""#.to_string();

        // Since for the json format these are calculated from the file contents we need to set them
        // to a correct value here
        let gamedata: GameData = GameData {
            gameitems_size: 20,
            images_size: 3,
            sounds_size: 2,
            fonts_size: 2,
            collections_size: 2,
            ..Default::default()
        };

        let mut vpx = VPX {
            custominfotags: vec!["test prop 2".to_string(), "test prop".to_string()],
            info: TableInfo {
                table_name: Some("test table name".to_string()),
                author_name: Some("test author name".to_string()),
                screenshot: Some(screenshot),
                table_blurb: Some("test table blurb".to_string()),
                table_rules: Some("test table rules".to_string()),
                author_email: Some("test author email".to_string()),
                release_date: Some("test release date".to_string()),
                table_save_rev: Some("123a".to_string()),
                table_version: Some("test table version".to_string()),
                author_website: Some("test author website".to_string()),
                table_save_date: Some("test table save date".to_string()),
                table_description: Some("test table description".to_string()),
                properties: HashMap::from([
                    ("test prop".to_string(), "test prop value".to_string()),
                    ("test prop2".to_string(), "test prop2 value".to_string()),
                ]),
            },
            version,
            gamedata,
            gameitems: vec![
                GameItemEnum::Bumper(bumper),
                GameItemEnum::Decal(decal),
                GameItemEnum::Flasher(flasher),
                GameItemEnum::Flipper(flipper),
                GameItemEnum::Gate(gate),
                GameItemEnum::HitTarget(hittarget),
                GameItemEnum::Kicker(kicker),
                GameItemEnum::Light(light),
                GameItemEnum::LightSequencer(light_sequencer),
                GameItemEnum::Plunger(plunger),
                GameItemEnum::Primitive(Box::new(primitive)),
                GameItemEnum::Ramp(ramp),
                GameItemEnum::Reel(reel),
                GameItemEnum::Rubber(rubber),
                GameItemEnum::Spinner(spinner),
                GameItemEnum::TextBox(textbox),
                GameItemEnum::Timer(timer),
                GameItemEnum::Trigger(trigger),
                GameItemEnum::Wall(wall),
                GameItemEnum::Generic(
                    100,
                    gameitem::generic::Generic {
                        name: "test gameitem".to_string(),
                        fields: vec![],
                    },
                ),
            ],
            images: vec![
                ImageData {
                    name: "test image".to_string(),
                    internal_name: None,
                    path: "test.png".to_string(),
                    width: 0,
                    height: 0,
                    link: None,
                    alpha_test_value: 0.0,
                    is_opaque: Some(true),
                    is_signed: Some(false),
                    jpeg: Some(ImageDataJpeg {
                        path: "test.png jpeg".to_string(),
                        name: "test image jpeg".to_string(),
                        internal_name: None,
                        data: vec![0, 1, 2, 3],
                    }),
                    bits: None,
                    md5_hash: None,
                },
                // this image will be replaced by a webp by the user
                ImageData {
                    name: "test image replaced".to_string(),
                    internal_name: None,
                    path: "replace.png".to_string(),
                    width: 0,
                    height: 0,
                    link: None,
                    alpha_test_value: 0.0,
                    is_opaque: Some(true),
                    is_signed: Some(false),
                    jpeg: Some(ImageDataJpeg {
                        path: "replace.png jpeg".to_string(),
                        name: "test image replaced jpeg".to_string(),
                        internal_name: None,
                        data: vec![0, 1, 2, 3],
                    }),
                    bits: None,
                    md5_hash: None,
                },
                ImageData {
                    name: "test image 2".to_string(),
                    internal_name: None,
                    path: "test2.bmp".to_string(),
                    width: 2,
                    height: 2,
                    link: None,
                    alpha_test_value: 0.0,
                    is_opaque: Some(true),
                    is_signed: Some(false),
                    jpeg: None,
                    bits: Some(ImageDataBits {
                        lzw_compressed_data: LZW_COMPRESSED_DATA.to_vec(),
                    }),
                    md5_hash: None,
                },
            ],
            sounds: vec![
                SoundData {
                    name: "test sound".to_string(),
                    path: "test.wav".to_string(),
                    wave_form: WaveForm {
                        format_tag: 1,
                        channels: 0,
                        samples_per_sec: 0,
                        avg_bytes_per_sec: 0,
                        block_align: 0,
                        bits_per_sample: 0,
                        cb_size: 0, // always 0
                    },
                    data: vec![0, 1, 2, 3],
                    internal_name: "test internal name".to_string(),
                    fade: 0,
                    volume: 0,
                    balance: 0,
                    output_target: OutputTarget::Table,
                },
                SoundData {
                    name: "test sound2".to_string(),
                    path: "test.ogg".to_string(),
                    wave_form: WaveForm::new(),
                    data: vec![0, 1, 2, 3],
                    internal_name: "test internal name2".to_string(),
                    fade: 1,
                    volume: 2,
                    balance: 3,
                    output_target: OutputTarget::Backglass,
                },
            ],
            fonts: vec![
                FontData {
                    name: "test font".to_string(),
                    path: "test.ttf".to_string(),
                    data: vec![0, 1, 2, 3],
                },
                FontData {
                    name: "test font2".to_string(),
                    path: "test2.ttf".to_string(),
                    data: vec![5, 6, 7],
                },
            ],
            collections: vec![
                Collection {
                    name: "test collection".to_string(),
                    items: vec!["test item".to_string()],
                    fire_events: false,
                    stop_single_events: false,
                    group_elements: false,
                },
                Collection {
                    name: "test collection 2".to_string(),
                    items: vec!["test item 2".to_string(), "test item 3".to_string()],
                    fire_events: true,
                    stop_single_events: true,
                    group_elements: true,
                },
            ],
        };

        let path = Path::new("expanded");
        write_fs(&vpx, &path, &ExpandOptions::default(), &fs)?;

        // the user has updated one image from png to webp
        let image_path = path.join("images").join("test image replaced.png");
        let new_image_path = image_path.with_extension("webp");
        fs.rename(&image_path, &new_image_path)?;

        // adjust the image path in the vpx
        vpx.images[1].change_extension("webp");

        let read = read_fs(&path, &fs)?;

        assert_eq!(&vpx, &read);
        Ok(())
    }

    /// A primitive with a non-ASCII name, mesh fields cleared to work around
    /// compression errors on fake data (same as in test_read_write).
    fn unicode_named_primitive() -> Primitive {
        let mut primitive: Primitive = Faker.fake();
        // NFC "ö" (U+00F6), the form found in real VPX files
        primitive.name = "PfL\u{00F6}cher".to_string();
        primitive.editor_layer_name = Some("Layer_1".to_string());
        primitive.use_3d_mesh = false;
        primitive.num_vertices = None;
        primitive.num_indices = None;
        primitive.compressed_vertices_len = None;
        primitive.compressed_vertices_data = None;
        primitive.compressed_indices_len = None;
        primitive.compressed_indices_data = None;
        primitive.vertices_data = None;
        primitive.indices_data = None;
        primitive.compressed_animation_vertices_len = None;
        primitive.compressed_animation_vertices_data = None;
        primitive
    }

    fn unicode_named_vpx() -> VPX {
        VPX {
            version: Version::new(1074),
            gamedata: GameData {
                gameitems_size: 1,
                images_size: 1,
                sounds_size: 1,
                fonts_size: 1,
                ..Default::default()
            },
            gameitems: vec![GameItemEnum::Primitive(Box::new(unicode_named_primitive()))],
            images: vec![ImageData {
                name: "L\u{00F6}cher image".to_string(),
                internal_name: None,
                path: "test.png".to_string(),
                width: 0,
                height: 0,
                link: None,
                alpha_test_value: 0.0,
                is_opaque: Some(true),
                is_signed: Some(false),
                jpeg: Some(ImageDataJpeg {
                    path: "test.png jpeg".to_string(),
                    name: "L\u{00F6}cher image jpeg".to_string(),
                    internal_name: None,
                    data: vec![0, 1, 2, 3],
                }),
                bits: None,
                md5_hash: None,
            }],
            sounds: vec![SoundData {
                name: "L\u{00F6}cher sound".to_string(),
                path: "test.wav".to_string(),
                wave_form: WaveForm {
                    format_tag: 1,
                    channels: 0,
                    samples_per_sec: 0,
                    avg_bytes_per_sec: 0,
                    block_align: 0,
                    bits_per_sample: 0,
                    cb_size: 0,
                },
                data: vec![0, 1, 2, 3],
                internal_name: "test internal name".to_string(),
                fade: 0,
                volume: 0,
                balance: 0,
                output_target: OutputTarget::Table,
            }],
            fonts: vec![FontData {
                name: "L\u{00F6}cher font".to_string(),
                path: "test.ttf".to_string(),
                data: vec![0, 1, 2, 3],
            }],
            ..Default::default()
        }
    }

    /// Simulates a storage layer (Safari/WebKit OPFS, macOS HFS+) that returns
    /// NFD-normalized file names for an extracted tree whose index json files
    /// reference the NFC forms.
    /// See https://github.com/francisdb/vpin/issues/355
    #[test]
    fn test_read_from_nfd_normalizing_storage() -> TestResult {
        use unicode_normalization::UnicodeNormalization;
        let vpx = unicode_named_vpx();

        let fs = MemoryFileSystem::default();
        let path = Path::new("expanded");
        write_fs(&vpx, &path, &ExpandOptions::default(), &fs)?;

        let mut renamed = 0;
        for file in fs.list_files() {
            let nfd: String = file.nfd().collect();
            if nfd != file {
                fs.rename(Path::new(&file), Path::new(&nfd))?;
                renamed += 1;
            }
        }
        // gameitem json, image, sound, font
        assert_eq!(renamed, 4, "expected all unicode-named files to be renamed");

        let read = read_fs(&path, &fs)?;
        assert_eq!(&vpx, &read);
        Ok(())
    }

    /// The reverse direction: the index references NFD file names (e.g. a tree
    /// extracted by an older vpin from a VPX with NFD part names) while the
    /// storage normalized the files themselves to NFC.
    #[test]
    fn test_read_with_nfd_index_and_nfc_files() -> TestResult {
        use unicode_normalization::UnicodeNormalization;
        let vpx = unicode_named_vpx();

        let fs = MemoryFileSystem::default();
        let path = Path::new("expanded");
        write_fs(&vpx, &path, &ExpandOptions::default(), &fs)?;

        // rewrite the gameitems index to reference the NFD form while the
        // files on "disk" keep their NFC names
        let index_path = path.join("gameitems.json");
        let index = fs.read_to_string(&index_path)?;
        let index_nfd: String = index.nfd().collect();
        assert_ne!(index, index_nfd);
        fs.write_file(&index_path, index_nfd.as_bytes())?;

        let read = read_fs(&path, &fs)?;
        assert_eq!(&vpx, &read);
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_extract_directory_list() {
        let vpx_path = Path::new("testdata/completely_blank_table_10_7_4.vpx");

        let files = extract_directory_list(vpx_path).unwrap();

        let base = Path::new("completely_blank_table_10_7_4");

        let first_4 = files.iter().take(4).cloned().collect::<Vec<String>>();
        assert_eq!(
            first_4,
            vec![
                base.join("collections.json"),
                base.join("fonts.json"),
                base.join("gamedata.json"),
                base.join("gameitems.json"),
            ]
        );

        let last_4 = files.iter().rev().take(4).cloned().collect::<Vec<String>>();
        assert_eq!(
            last_4,
            vec![
                base.join("version.txt"),
                base.join("sounds.json"),
                base.join("script.vbs"),
                base.join("materials-physics-old.json"),
            ]
        );

        assert_eq!(files.len(), 95);
    }

    /// A table with one game item and one image, enough for every
    /// expanded file kind to show up
    fn small_table() -> VPX {
        let mut wall: gameitem::wall::Wall = Faker.fake();
        wall.name = "test wall".to_string();
        VPX {
            gameitems: vec![GameItemEnum::Wall(wall)],
            images: vec![ImageData {
                name: "test image".to_string(),
                path: "test.png".to_string(),
                jpeg: Some(ImageDataJpeg {
                    path: "test.png".to_string(),
                    name: "test image".to_string(),
                    internal_name: None,
                    data: vec![0, 1, 2, 3],
                }),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// The files below `root` in the memory file system, relative and sorted
    fn relative_files(fs: &MemoryFileSystem, root: &Path) -> Vec<String> {
        let mut files: Vec<String> = fs
            .list_files()
            .iter()
            .map(|file| {
                Path::new(file)
                    .strip_prefix(root)
                    .unwrap_or(Path::new(file))
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn test_write_filter_keeps_only_matching_files() -> TestResult {
        let vpx = small_table();
        let root = Path::new("table");

        let unfiltered = MemoryFileSystem::new();
        write_fs(&vpx, &root, &ExpandOptions::new(), &unfiltered)?;
        let all_files = relative_files(&unfiltered, root);
        assert!(all_files.contains(&"gameitems/Wall.test_wall.json".to_string()));
        assert!(all_files.iter().any(|file| file.starts_with("images/")));

        // everything except the game item files
        let filtered = MemoryFileSystem::new();
        let options = ExpandOptions::new().filter(|path: &Path| !path.starts_with("gameitems"));
        write_fs(&vpx, &root, &options, &filtered)?;
        let expected: Vec<String> = all_files
            .iter()
            .filter(|file| !file.starts_with("gameitems/"))
            .cloned()
            .collect();
        let files = relative_files(&filtered, root);
        assert_eq!(files, expected);
        // the index still lists what was left out
        assert!(files.contains(&"gameitems.json".to_string()));

        // a single file, with the same content as in the full write
        let single = MemoryFileSystem::new();
        let options = ExpandOptions::new().filter(|path: &Path| path == Path::new("gamedata.json"));
        write_fs(&vpx, &root, &options, &single)?;
        assert_eq!(relative_files(&single, root), vec!["gamedata.json"]);
        assert_eq!(
            single.get_file("table/gamedata.json"),
            unfiltered.get_file("table/gamedata.json")
        );
        Ok(())
    }

    /// Records the directories a writer asks for, to check that a filtered
    /// write does not create folders it puts nothing in
    struct DirRecordingFileSystem {
        inner: MemoryFileSystem,
        dirs: std::sync::Mutex<Vec<std::path::PathBuf>>,
    }

    impl FileSystem for DirRecordingFileSystem {
        fn create_file(&self, path: &Path) -> io::Result<Box<dyn Write>> {
            self.inner.create_file(path)
        }
        fn open_file(&self, path: &Path) -> io::Result<Box<dyn Read>> {
            self.inner.open_file(path)
        }
        fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.inner.read_file(path)
        }
        fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            self.inner.write_file(path, data)
        }
        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            let mut dirs = self
                .dirs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !dirs.contains(&path.to_path_buf()) {
                dirs.push(path.to_path_buf());
            }
            self.inner.create_dir_all(path)
        }
        fn exists(&self, path: &Path) -> bool {
            self.inner.exists(path)
        }
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_write_filter_keeps_derived_meshes_it_asks_for() -> TestResult {
        // the guard in front of the mesh builders must name the files the
        // way the writers do, or wanted meshes would never be built
        let vpx = crate::vpx::read(Path::new("testdata/completely_blank_table_10_7_4.vpx"))?;
        let root = Path::new("table");

        let all = MemoryFileSystem::new();
        let options = ExpandOptions::new().generate_derived_meshes(true);
        write_fs(&vpx, &root, &options, &all)?;
        let generated: Vec<String> = relative_files(&all, root)
            .into_iter()
            .filter(|file| file.contains("-generated"))
            .collect();
        assert!(!generated.is_empty());

        let only_generated = MemoryFileSystem::new();
        let options = ExpandOptions::new()
            .generate_derived_meshes(true)
            .filter(|path: &Path| path.to_string_lossy().contains("-generated"));
        write_fs(&vpx, &root, &options, &only_generated)?;
        assert_eq!(relative_files(&only_generated, root), generated);
        Ok(())
    }

    #[test]
    fn test_write_filter_creates_no_empty_directories() -> TestResult {
        let vpx = small_table();
        let root = Path::new("table");

        let fs = DirRecordingFileSystem {
            inner: MemoryFileSystem::new(),
            dirs: std::sync::Mutex::new(Vec::new()),
        };
        let options = ExpandOptions::new().filter(|path: &Path| path.starts_with("gameitems"));
        write_fs(&vpx, &root, &options, &fs)?;

        let dirs = fs
            .dirs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        // only the game item folder: gameitems.json is not below gameitems/, and nothing asks for images/
        assert_eq!(dirs, vec![root.join("gameitems")]);
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_expand_options_derived_meshes() {
        let vpx_path = Path::new("testdata/completely_blank_table_10_7_4.vpx");
        let vpx = crate::vpx::read(vpx_path).unwrap();

        // Without derived meshes (default)
        {
            let fs = MemoryFileSystem::default();
            let path = Path::new("expanded");
            let options = ExpandOptions::default();
            write_fs(&vpx, &path, &options, &fs).unwrap();

            let files = fs.list_files();
            // Should not contain any -generated files
            let generated_files: Vec<_> =
                files.iter().filter(|f| f.contains("-generated")).collect();
            assert!(
                generated_files.is_empty(),
                "Should not generate derived meshes by default: {:?}",
                generated_files
            );
        }

        // With derived meshes enabled
        {
            let fs = MemoryFileSystem::default();
            let path = Path::new("expanded");
            let options = ExpandOptions::new().generate_derived_meshes(true);
            write_fs(&vpx, &path, &options, &fs).unwrap();

            let files = fs.list_files();
            // Should contain -generated files
            let generated_files: Vec<_> =
                files.iter().filter(|f| f.contains("-generated")).collect();
            assert!(
                !generated_files.is_empty(),
                "Should generate derived meshes when enabled"
            );
        }
    }
}
