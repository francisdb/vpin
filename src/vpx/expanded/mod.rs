//! Expanded VPX directory format for easier editing and version control.
//!
//! This module provides functions to extract VPX files into a directory structure
//! with separate JSON and binary files, and reassemble them back into VPX format.
//!
//! # Primitive Mesh Formats
//!
//! Primitive mesh data can be exported in two formats:
//! - **OBJ** (default): Text-based Wavefront OBJ format, human-readable
//! - **GLB**: Binary GLTF format, significantly faster for large meshes
//! - **GLTF**: JSON + external BIN buffer for tooling-friendly workflows
//!
//! Use [`write_with_options`] to specify the format and other options.
//! Both formats are supported for reading, with OBJ checked first for backward compatibility.

mod bumpers;
mod flashers;
mod flippers;
mod fonts;
mod gameitems;
mod gltf_export;
mod images;
mod materials;
mod mesh_common;
mod mesh_validation;
mod metadata;
mod primitives;
mod ramps;
mod rubbers;
mod sounds;
mod spinners;
mod util;
mod walls;

use crate::filesystem::{FileSystem, MemoryFileSystem, RealFileSystem};
use crate::vpx::material::Material;
use crate::vpx::{VPX, Version};
pub use gltf_export::export_glb;
use log::{info, warn};
pub use primitives::BytesMutExt;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Read, Write};
use std::path::Path;

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
#[derive(Debug, Clone)]
pub struct ExpandOptions {
    mesh_format: PrimitiveMeshFormat,
    generate_derived_meshes: bool,
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
        }
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

    let version_path = expanded_dir.as_ref().join("version.txt");
    let mut version_file = fs.create_file(&version_path)?;
    let version_string = vpx.version.to_u32_string();
    version_file.write_all(version_string.as_bytes())?;
    info!("✓ Version file written");

    if let Some(screenshot) = &vpx.info.screenshot {
        let screenshot_path = expanded_dir.as_ref().join("screenshot.png");
        let mut screenshot_file = fs.create_file(&screenshot_path)?;
        screenshot_file.write_all(screenshot)?;
        info!("✓ Screenshot written");
    } else {
        info!("✓ No screenshot to write");
    }

    info!("Writing table info...");
    metadata::write_info(&vpx.info, &vpx.custominfotags, expanded_dir, fs)?;
    info!("✓ Table info written");

    info!("Writing collections...");
    metadata::write_collections(&vpx.collections, expanded_dir, fs)?;
    info!("✓ {} Collections written", vpx.collections.len());

    info!("Writing game items...");
    gameitems::write_gameitems(&vpx.gameitems, expanded_dir, options, fs)?;
    info!("✓ {} Game items written", vpx.gameitems.len());

    info!("Writing images...");
    images::write_images(&vpx.images, expanded_dir, fs)?;
    info!("✓ {} Images written", vpx.images.len());

    info!("Writing sounds...");
    sounds::write_sounds(&vpx.sounds, expanded_dir, fs)?;
    info!("✓ {} Sounds written", vpx.sounds.len());

    info!("Writing fonts...");
    fonts::write_fonts(&vpx.fonts, expanded_dir, fs)?;
    info!("✓ {} Fonts written", vpx.fonts.len());

    info!("Writing game data...");
    metadata::write_game_data(&vpx.gamedata, expanded_dir, fs)?;
    info!("✓ Game data written");

    if let Some(materials) = &vpx.gamedata.materials {
        info!("Writing materials...");
        materials::write_materials(materials, expanded_dir, fs)?;
        info!("✓ Materials written");
        validate_material_conversion(&vpx, materials);
    } else {
        info!("Writing legacy materials...");
        materials::write_legacy_materials(
            &vpx.gamedata.materials_old,
            vpx.gamedata.materials_physics_old.as_ref(),
            expanded_dir,
            fs,
        )?;
        info!("✓ Legacy materials written");
    }

    info!("Writing render probes...");
    metadata::write_renderprobes(vpx.gamedata.render_probes.as_ref(), expanded_dir, fs)?;
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
            format!("Could not parse version {}: {}", &version_string, e),
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

pub fn extract_directory_list(vpx_file_path: &Path) -> Vec<String> {
    let vpx = crate::vpx::read(vpx_file_path).unwrap();
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
    write_fs(&vpx, &expanded_dir, &options, &fs).unwrap();

    let mut files = fs.list_files();
    files.sort();
    files
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
        primitive.num_vertices = None;
        primitive.num_indices = None;
        primitive.compressed_vertices_len = None;
        primitive.compressed_vertices_data = None;
        primitive.compressed_indices_len = None;
        primitive.compressed_indices_data = None;
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
                GameItemEnum::Primitive(primitive),
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

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_extract_directory_list() {
        let vpx_path = Path::new("testdata/completely_blank_table_10_7_4.vpx");

        let files = extract_directory_list(vpx_path);

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
