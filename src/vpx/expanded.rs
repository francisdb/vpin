use bytes::{BufMut, BytesMut};
use log;
use std::collections::HashSet;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io::{self, BufRead, Read, Seek, Write};
use std::iter::Zip;
use std::slice::Iter;
use std::{fs::File, path::Path};

use crate::filesystem::{FileSystem, RealFileSystem};

use super::{VPX, Version, gameitem, read_gamedata};
use cfb::CompoundFile;
use image::DynamicImage;
use log::{debug, info, warn};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use serde::de;
use serde_json::Value;
use tracing::{info_span, instrument};

use super::collection::Collection;
use super::font;
use super::gamedata::{GameData, GameDataJson};
use super::sound;
use super::sound::{SoundData, SoundDataJson, read_sound, write_sound};
use super::version;
use crate::vpx::biff::{BiffRead, BiffReader};
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::font::{FontData, FontDataJson};
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive::{
    MAX_VERTICES_FOR_2_BYTE_INDEX, ReadMesh, VertData, read_vpx_animation_frame,
    write_animation_vertex_data,
};
use crate::vpx::image::{ImageData, ImageDataBits, ImageDataJson};
use crate::vpx::jsonmodel::{collections_json, info_to_json, json_to_collections, json_to_info};
use crate::vpx::lzw::{from_lzw_blocks, to_lzw_blocks};

use crate::vpx::material::{
    Material, MaterialJson, SaveMaterial, SaveMaterialJson, SavePhysicsMaterial,
    SavePhysicsMaterialJson,
};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::{ObjData, read_obj as obj_read_obj, write_obj};
use crate::vpx::renderprobe::{RenderProbeJson, RenderProbeWithGarbage};
use crate::vpx::tableinfo::TableInfo;

/// Sanitize a filename using the sanitize-filename crate
// TODO the whole sanitize_filename effort is not cross-platform compatible
//   Eg a vpx extracted on linux could fail to be opened on Windows if the sound name
//   contains such characters.
//   This should probably be improved in the future
fn sanitize_filename<S: AsRef<str>>(name: S) -> String {
    sanitize_filename::sanitize(name)
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

// make the error compatible with io::Error keeping the source

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

pub fn write<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    write_fs(vpx, expanded_dir, &RealFileSystem)
}

pub fn write_fs<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
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
    write_info(&vpx, expanded_dir, fs)?;
    info!("✓ Table info written");

    info!("Writing collections...");
    let collections_json_path = expanded_dir.as_ref().join("collections.json");
    let mut collections_json_file = fs.create_file(&collections_json_path)?;
    let json_collections = collections_json(&vpx.collections);
    serde_json::to_writer_pretty(&mut collections_json_file, &json_collections)?;
    info!("✓ {} Collections written", vpx.collections.len());

    info!("Writing game items...");
    write_gameitems(vpx, expanded_dir, fs)?;
    info!("✓ {} Game items written", vpx.gameitems.len());

    info!("Writing images...");
    write_images(vpx, expanded_dir, fs)?;
    info!("✓ {} Images written", vpx.images.len());

    info!("Writing sounds...");
    write_sounds(vpx, expanded_dir, fs)?;
    info!("✓ {} Sounds written", vpx.sounds.len());

    info!("Writing fonts...");
    write_fonts(vpx, expanded_dir, fs)?;
    info!("✓ {} Fonts written", vpx.fonts.len());

    info!("Writing game data...");
    write_game_data(vpx, expanded_dir, fs)?;
    info!("✓ Game data written");

    if vpx.gamedata.materials.is_some() {
        info!("Writing materials...");
        write_materials(vpx, expanded_dir, fs)?;
        info!("✓ Materials written");
    } else {
        info!("Writing legacy materials...");
        write_old_materials(vpx, expanded_dir, fs)?;
        write_old_materials_physics(vpx, expanded_dir, fs)?;
        info!("✓ Legacy materials written");
    }

    info!("Writing render probes...");
    write_renderprobes(vpx, expanded_dir, fs)?;
    info!("✓ Render probes written");

    info!("=== VPX extraction process completed successfully ===");
    Ok(())
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
    let (info, custominfotags) = read_info(expanded_dir, screenshot, fs)?;
    info!("✓ Table info read");

    info!("Reading collections...");
    let collections = read_collections(expanded_dir, fs)?;
    info!("✓ {} Collections read", collections.len());

    info!("Reading game items...");
    let gameitems = read_gameitems(expanded_dir, fs)?;
    info!("✓ {} Game items read", gameitems.len());

    info!("Reading images...");
    let images = read_images(expanded_dir, fs)?;
    info!("✓ {} Images read", images.len());

    info!("Reading sounds...");
    let sounds = read_sounds(expanded_dir, fs)?;
    info!("✓ {} Sounds read", sounds.len());

    info!("Reading fonts...");
    let fonts = read_fonts(expanded_dir, fs)?;
    info!("✓ {} Fonts read", fonts.len());

    info!("Reading game data...");
    let mut gamedata = read_game_data(expanded_dir, fs)?;
    gamedata.collections_size = collections.len() as u32;
    gamedata.gameitems_size = gameitems.len() as u32;
    gamedata.images_size = images.len() as u32;
    gamedata.sounds_size = sounds.len() as u32;
    gamedata.fonts_size = fonts.len() as u32;
    let materials_opt = read_materials(expanded_dir, fs)?;
    match materials_opt {
        Some(materials) => {
            gamedata.materials_old = materials.iter().map(SaveMaterial::from).collect();
            gamedata.materials_physics_old =
                Some(materials.iter().map(SavePhysicsMaterial::from).collect());
            gamedata.materials_size = materials.len() as u32;
            gamedata.materials = Some(materials);
        }
        None => {
            gamedata.materials_old = read_old_materials(expanded_dir, fs)?;
            gamedata.materials_physics_old = read_old_materials_physics(expanded_dir, fs)?;
            gamedata.materials_size = gamedata.materials_old.len() as u32;
        }
    }
    gamedata.render_probes = read_renderprobes(expanded_dir, fs)?;
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

fn write_game_data<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let mut game_data_file = fs.create_file(&game_data_path)?;
    let json = GameDataJson::from_game_data(&vpx.gamedata);
    serde_json::to_writer_pretty(&mut game_data_file, &json)?;
    let script_path = expanded_dir.as_ref().join("script.vbs");
    let mut script_file = fs.create_file(&script_path)?;
    let script_bytes: Vec<u8> = vpx.gamedata.code.clone().into();
    script_file.write_all(script_bytes.as_ref())?;
    Ok(())
}

fn read_game_data<P: AsRef<Path>>(expanded_dir: &P, fs: &dyn FileSystem) -> io::Result<GameData> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let game_data_json: GameDataJson = read_json(game_data_path, fs)?;
    let mut game_data = game_data_json.to_game_data();
    let script_path = expanded_dir.as_ref().join("script.vbs");
    if !fs.exists(&script_path) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Script file not found: {}", script_path.display()),
        ));
    }
    let code = fs.read_file(&script_path)?;
    game_data.code = code.into();
    Ok(game_data)
}

fn read_json<P: AsRef<Path>, T>(json_path: P, fs: &dyn FileSystem) -> io::Result<T>
where
    T: de::DeserializeOwned,
{
    let path = json_path.as_ref();
    if !fs.exists(path) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("JSON file not found: {}", path.display()),
        ));
    }
    let mut json_file = fs.open_file(path)?;
    serde_json::from_reader(&mut json_file).map_err(|e| {
        io::Error::other(format!(
            "Failed to parse/read json {}: {}",
            path.display(),
            e
        ))
    })
}

fn write_images<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    info!(
        "Starting image processing - total images: {}",
        vpx.images.len()
    );

    let images_index_path = expanded_dir.as_ref().join("images.json");
    let mut images_index_file = fs.create_file(&images_index_path)?;
    let mut image_names_lower: HashSet<String> = HashSet::new();
    let mut image_names_dupe_counter = 0;
    let mut json_images = Vec::with_capacity(vpx.sounds.len());
    let images: io::Result<Vec<(String, &ImageData)>> = vpx
        .images
        .iter()
        .enumerate()
        .map(|(image_index, image)| {
            debug!(
                "Processing image {}/{}: name='{}', size={}x{}",
                image_index + 1,
                vpx.images.len(),
                image.name,
                image.width,
                image.height
            );
            let mut json = ImageDataJson::from_image_data(image);
            let name_sanitized = sanitize_filename(&image.name);
            if name_sanitized != image.name {
                info!(
                    "Image name {} contained invalid characters, sanitized to {}",
                    image.name, &name_sanitized
                );
                json.name_dedup = Some(name_sanitized.clone());
            }
            let lower_name = name_sanitized.to_lowercase();
            if image_names_lower.contains(&lower_name) {
                image_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", image.name, image_names_dupe_counter);
                info!(
                    "Image name {} is not unique, renaming file to {}",
                    name_sanitized, &name_dedup
                );
                json.name_dedup = Some(name_dedup);
            }
            image_names_lower.insert(lower_name);

            let actual_name = json.name_dedup.as_ref().unwrap_or(&image.name);
            let file_name = format!("{}.{}", actual_name, image.ext());

            if let Some(jpeg) = &image.jpeg {
                // Only if the actual image dimensions are different from
                // the ones in the vpx file we add them to the json.
                let cursor = io::Cursor::new(&jpeg.data);
                let dimensions_file = read_image_dimensions_from_file_steam(&file_name, cursor)?;
                match dimensions_file {
                    Some((width_file, height_file)) => {
                        if image.width != width_file || image.height != height_file {
                            info!(
                                "Stale image dimensions for {} in vpx {}x{} vs in image {}x{}",
                                file_name, image.width, image.height, width_file, height_file
                            );
                        }
                        if image.width != width_file {
                            json.width = Some(image.width);
                        }
                        if image.height != height_file {
                            json.height = Some(image.height);
                        }
                    }
                    None => {
                        json.width = Some(image.width);
                        json.height = Some(image.height);
                    }
                }
            };
            if image.link.is_some() {
                // Links always store the dimensions in the json
                json.width = Some(image.width);
                json.height = Some(image.height);
            }
            // for bits images we don't store the dimensions in the json as they always match

            json_images.push(json);
            debug!(
                "Successfully processed image {}/{}: '{}'",
                image_index + 1,
                vpx.images.len(),
                image.name
            );
            Ok((file_name, image))
        })
        .collect();
    let images = images?;
    serde_json::to_writer_pretty(&mut images_index_file, &json_images)?;

    let images_dir = expanded_dir.as_ref().join("images");
    fs.create_dir_all(&images_dir)?;
    debug!("Created images directory: {}", images_dir.display());
    info!("Starting to write {} image files to disk", images.len());

    images
        .iter()
        .enumerate()
        .try_for_each(|(file_index, (image_file_name, image))| {
            debug!(
                "Writing image file {}/{}: '{}'",
                file_index + 1,
                images.len(),
                image_file_name
            );
            let file_path = images_dir.join(image_file_name);
            debug!("Full file path: {}", file_path.display());

            if !fs.exists(&file_path) {
                if image.is_link() {
                    info!("Image is a link, no data to write");
                    Ok(())
                } else if let Some(jpeg) = &image.jpeg {
                    debug!("Writing JPEG data ({} bytes)", jpeg.data.len());
                    fs.write_file(&file_path, &jpeg.data).map_err(|e| {
                        warn!(
                            "ERROR: Failed to write JPEG data for '{}': {}",
                            file_path.display(),
                            e
                        );
                        e
                    })
                } else if let Some(bits) = &image.bits {
                    debug!(
                        "Writing BMP data (compressed size: {} bytes)",
                        bits.lzw_compressed_data.len()
                    );
                    assert_eq!(
                        image.ext().to_ascii_lowercase(),
                        "bmp",
                        "Images stored as bits should have the extension .bmp"
                    );

                    write_image_bmp(
                        &file_path,
                        &bits.lzw_compressed_data,
                        image.width,
                        image.height,
                        fs,
                    )
                    .map_err(|e| {
                        warn!(
                            "ERROR: Failed to write BMP image '{}': {}",
                            file_path.display(),
                            e
                        );
                        e
                    })
                } else {
                    let err = io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Image has no data: {}", file_path.display()),
                    );
                    warn!("ERROR: {}", err);
                    Err(err)
                }
            } else {
                let err = io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Two images with the same name detected, should not happen: {}",
                        file_path.display()
                    ),
                );
                warn!("ERROR: {}", err);
                Err(err)
            }
        })?;
    info!("Successfully completed writing all {} images", images.len());
    Ok(())
}

fn write_image_bmp(
    file_path: &Path,
    lzw_compressed_data: &[u8],
    width: u32,
    height: u32,
    fs: &dyn FileSystem,
) -> io::Result<()> {
    let image_to_save = vpx_image_to_dynamic_image(lzw_compressed_data, width, height);
    if image_to_save.color().has_alpha() {
        // One example is the table "Guns N Roses (Data East 1994).vpx"
        // that contains vp9 images with non-255 alpha values.
        // They are actually labeled as sRGBA in the Visual Pinball image manager.
        // However, when Visual Pinball itself exports the image it drops the alpha values.
        let file_name = file_path
            .file_name()
            .map(OsStr::to_string_lossy)
            .unwrap_or_default();
        warn!(
            "Image {file_name} has non-opaque pixels, writing as RGBA BMP that might not be supported by all applications"
        );
    }
    let mut buffer = io::Cursor::new(Vec::new());
    image_to_save
        .write_to(&mut buffer, image::ImageFormat::Bmp)
        .map_err(|image_error| {
            io::Error::other(format!(
                "Failed to encode bitmap {}: {}",
                file_path.display(),
                image_error
            ))
        })?;
    fs.write_file(file_path, buffer.get_ref())
}

pub(crate) fn vpx_image_to_dynamic_image(
    lzw_compressed_data: &[u8],
    width: u32,
    height: u32,
) -> DynamicImage {
    let decompressed_bgra = from_lzw_blocks(lzw_compressed_data);
    let decompressed_rgba: Vec<u8> = swap_red_and_blue(&decompressed_bgra);

    let rgba_image = image::RgbaImage::from_raw(width, height, decompressed_rgba)
        .expect("Decompressed image data does not match dimensions");
    let dynamic_image = DynamicImage::ImageRgba8(rgba_image);

    let uses_alpha = decompressed_bgra.chunks_exact(4).any(|bgra| bgra[3] != 255);
    if uses_alpha {
        dynamic_image
    } else {
        let rgb_image = dynamic_image.to_rgb8();
        DynamicImage::ImageRgb8(rgb_image)
    }
}

/// Can convert between RGBA and BGRA by swapping the red and blue channels
fn swap_red_and_blue(data: &[u8]) -> Vec<u8> {
    let mut swapped = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        swapped.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]])
    }
    swapped
}

fn read_images<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<ImageData>> {
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let images_index_json: Vec<ImageDataJson> = read_json(images_index_path, fs)?;
    let images_dir = expanded_dir.as_ref().join("images");
    let images: io::Result<Vec<ImageData>> = images_index_json
        .into_iter()
        .map(|image_data_json| {
            if image_data_json.is_link() {
                let image = image_data_json.to_image_data(
                    image_data_json.width.unwrap_or(0),
                    image_data_json.height.unwrap_or(0),
                    None,
                );
                Ok(image)
            } else {
                let file_name = image_data_json
                    .name_dedup
                    .as_ref()
                    .unwrap_or(&image_data_json.name);
                let full_file_name = format!("{}.{}", file_name, image_data_json.ext());
                let mut file_path = images_dir.join(&full_file_name);

                let mut new_extension = None;
                if image_data_json.ext() == "png" && !fs.exists(&file_path) {
                    let file_path_webp = images_dir.join(format!("{file_name}.webp"));
                    if fs.exists(&file_path_webp) {
                        new_extension = Some("webp");
                        file_path = file_path_webp;
                    }
                }

                if fs.exists(&file_path) {
                    let image_data = fs.read_file(&file_path)?;
                    let image = if image_data_json.is_bmp() {
                        let read_bmp = read_image_bmp(&image_data).map_err(|e| {
                            io::Error::new(
                                e.kind(),
                                format!("Failed to read BMP '{}' ({} bytes): {}", file_path.display(), image_data.len(), e)
                            )
                        })?;
                        let image_data = ImageDataBits {
                            lzw_compressed_data: read_bmp.lzw_compressed_data,
                        };
                        image_data_json.to_image_data(
                            read_bmp.width,
                            read_bmp.height,
                            Some(image_data),
                        )
                    } else {
                        let dimensions_from_file = read_image_dimensions_from_bytes(&full_file_name, &image_data)?;

                        let width = match image_data_json.width {
                            Some(w) => w,
                            None =>
                                match dimensions_from_file {
                                    Some((width_file, _)) => width_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image width not provided and could not be read from file")),
                                }
                        };

                        let height = match image_data_json.height {
                            Some(h) => h,
                            None =>
                                match dimensions_from_file {
                                    Some((_, height_file)) => height_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image height not provided and could not be read from file")),
                                }
                        };

                        if let Some((image_w, image_h)) = dimensions_from_file && (width != image_w || height != image_h) {
                            warn!(
                                "Stale image dimensions for {full_file_name} in json {}x{} vs in image {}x{}",
                                width, height, image_w, image_h
                            );
                        }

                        let mut image = image_data_json.to_image_data(width, height, None);
                        if let Some(jpg) = &mut image.jpeg {
                            jpg.data = image_data;
                        }
                        if let Some(new_extension) = new_extension {
                            // we need to change the file extension for the path
                            image.change_extension(new_extension);
                        }
                        image
                    };
                    Ok(image)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Image file not found: {}", file_path.display()),
                    ))
                }
            }
        })
        .collect();
    images
}

fn read_image_dimensions_from_file_steam<R: BufRead + Seek>(
    file_name: &str,
    reader: R,
) -> io::Result<Option<(u32, u32)>> {
    let dimensions_from_file = match image::ImageFormat::from_path(file_name) {
        Ok(format) => {
            let decoder = image::ImageReader::with_format(reader, format).with_guessed_format()?;
            if Some(format) != decoder.format() {
                warn!(
                    "Detected image format {} for [{}] where the extension suggests {:?}",
                    decoder
                        .format()
                        .map_or("unknown".to_string(), |f| format!("{f:?}")),
                    file_name,
                    format,
                );
            }
            match decoder.into_dimensions() {
                Ok(dimensions) => Some(dimensions),
                Err(image_error) => {
                    warn!("Failed to read image dimensions for {file_name}: {image_error}");
                    None
                }
            }
        }
        Err(e) => {
            warn!("Failed to determine image format for {file_name}: {e}");
            None
        }
    };
    Ok(dimensions_from_file)
}

fn read_image_dimensions_from_bytes(
    file_name: &str,
    data: &[u8],
) -> io::Result<Option<(u32, u32)>> {
    let cursor = io::Cursor::new(data);
    read_image_dimensions_from_file_steam(file_name, cursor)
}

struct ImageBmp {
    width: u32,
    height: u32,
    lzw_compressed_data: Vec<u8>,
}

fn read_image_bmp(data: &[u8]) -> io::Result<ImageBmp> {
    // Use auto-detection instead of forcing BMP format for better compatibility
    let image = image::load_from_memory(data).map_err(|image_error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to read BMP image: {image_error}"),
        )
    })?;

    let raw_rgba = match image.color() {
        image::ColorType::Rgb8 => image.to_rgba8().into_raw(),
        image::ColorType::Rgba8 => image.to_rgba8().into_raw(),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("BMP image uses {other:?}, expecting Rgb8 or Rgba8 format"),
            ));
        }
    };

    // convert to BGRA
    let raw_bgra: Vec<u8> = swap_red_and_blue(&raw_rgba);

    let image_bmp = ImageBmp {
        width: image.width(),
        height: image.height(),
        lzw_compressed_data: to_lzw_blocks(&raw_bgra),
    };

    Ok(image_bmp)
}

fn write_sounds<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let sounds_index_path = expanded_dir.as_ref().join("sounds.json");
    let mut sounds_index_file = fs.create_file(&sounds_index_path)?;
    let mut sound_names_lower: HashSet<String> = HashSet::new();
    let mut sound_names_dupe_counter = 0;
    let mut json_sounds = Vec::with_capacity(vpx.sounds.len());
    let sounds: Vec<(String, &SoundData)> = vpx
        .sounds
        .iter()
        .map(|sound| {
            let mut json = SoundDataJson::from_sound_data(sound);
            let name_sanitized = sanitize_filename(&sound.name);
            if name_sanitized != sound.name {
                info!(
                    "Sound name {} contained invalid characters, sanitized to {}",
                    sound.name, &name_sanitized
                );
                json.name_dedup = Some(name_sanitized.clone());
            }
            let lower_name = name_sanitized.to_lowercase();
            if sound_names_lower.contains(&lower_name) {
                sound_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", sound.name, sound_names_dupe_counter);
                info!(
                    "Sound name {} is not unique, renaming file to {}",
                    name_sanitized, &name_dedup
                );
                json.name_dedup = Some(name_dedup);
            }
            sound_names_lower.insert(lower_name);

            let actual_name = json.name_dedup.as_ref().unwrap_or(&name_sanitized);
            let file_name = format!("{}.{}", actual_name, sound.ext());
            json_sounds.push(json);
            (file_name, sound)
        })
        .collect();
    serde_json::to_writer_pretty(&mut sounds_index_file, &json_sounds)?;

    let sounds_dir = expanded_dir.as_ref().join("sounds");
    fs.create_dir_all(&sounds_dir)?;
    sounds.iter().try_for_each(|(sound_file_name, sound)| {
        let sound_path = sounds_dir.join(sound_file_name);
        if !fs.exists(&sound_path) {
            let mut file = fs.create_file(&sound_path)?;
            file.write_all(&write_sound(sound))
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Two sounds with the same name detected, should not happen: {}",
                    sound_path.display()
                ),
            ))
        }
    })?;
    Ok(())
}

fn read_sounds<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<SoundData>> {
    let sounds_json_path = expanded_dir.as_ref().join("sounds.json");
    if !fs.exists(&sounds_json_path) {
        info!("No sounds.json found");
        return Ok(vec![]);
    }
    let sounds_json: Vec<SoundDataJson> = read_json(&sounds_json_path, fs)?;
    let sounds_dir = expanded_dir.as_ref().join("sounds");
    let sounds: io::Result<Vec<SoundData>> = sounds_json
        .into_iter()
        .map(|sound_data_json| {
            let mut sound = sound_data_json.to_sound_data();
            let file_name = sound_data_json.name_dedup.as_ref().unwrap_or(&sound.name);
            let full_file_name = format!("{}.{}", file_name, sound.ext());
            let file_path = sounds_dir.join(full_file_name);
            if fs.exists(&file_path) {
                let sound_data = fs.read_file(&file_path)?;
                read_sound(&sound_data, &mut sound);
                Ok(sound)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Sound file not found: {}", file_path.display()),
                ))
            }
        })
        .collect();
    sounds
}

fn write_fonts<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let fonts_json_path = expanded_dir.as_ref().join("fonts.json");
    let mut fonts_index_file = fs.create_file(&fonts_json_path)?;
    let fonts_index: Vec<FontDataJson> =
        vpx.fonts.iter().map(FontDataJson::from_font_data).collect();
    serde_json::to_writer_pretty(&mut fonts_index_file, &fonts_index)?;

    let fonts_dir = expanded_dir.as_ref().join("fonts");
    fs.create_dir_all(&fonts_dir)?;
    vpx.fonts.iter().try_for_each(|font| {
        let sanitized_name = sanitize_filename(&font.name);
        let file_name = format!("{}.{}", sanitized_name, font.ext());
        let font_path = fonts_dir.join(file_name);
        let mut file = fs.create_file(&font_path)?;
        file.write_all(&font.data)
    })?;
    Ok(())
}

fn read_fonts<P: AsRef<Path>>(expanded_dir: &P, fs: &dyn FileSystem) -> io::Result<Vec<FontData>> {
    let fonts_index_path = expanded_dir.as_ref().join("fonts.json");
    if !fs.exists(&fonts_index_path) {
        info!("No fonts.json found");
        return Ok(vec![]);
    }
    let fonts_json: Vec<FontDataJson> = read_json(fonts_index_path, fs)?;
    let fonts_index: Vec<FontData> = fonts_json
        .iter()
        .map(|font_data_json| font_data_json.to_font_data())
        .collect();
    let fonts_dir = expanded_dir.as_ref().join("fonts");
    let fonts: io::Result<Vec<FontData>> = fonts_index
        .into_iter()
        .map(|mut font| {
            let sanitized_name = sanitize_filename(&font.name);
            let file_name = format!("{}.{}", sanitized_name, font.ext());
            let font_path = fonts_dir.join(file_name);
            if fs.exists(&font_path) {
                let font_data = fs.read_file(&font_path)?;
                font.data = font_data;
                Ok(font)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Font file not found: {}", font_path.display()),
                ))
            }
        })
        .collect();
    fonts
}

fn write_materials<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(materials) = &vpx.gamedata.materials {
        let materials_path = expanded_dir.as_ref().join("materials.json");
        let mut materials_file = fs.create_file(&materials_path)?;
        let materials_index: Vec<MaterialJson> =
            materials.iter().map(MaterialJson::from_material).collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    }
    Ok(())
}

fn read_materials<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<Material>>> {
    let materials_path = expanded_dir.as_ref().join("materials.json");
    if !fs.exists(&materials_path) {
        return Ok(None);
    }
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<MaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<Material> = materials_index
        .into_iter()
        .map(|m| MaterialJson::to_material(&m))
        .collect();
    Ok(Some(materials))
}

fn write_old_materials<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    let mut materials_file = fs.create_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = vpx
        .gamedata
        .materials_old
        .iter()
        .map(SaveMaterialJson::from_save_material)
        .collect();
    serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    Ok(())
}

fn read_old_materials<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<SaveMaterial>> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    if !fs.exists(&materials_path) {
        return Ok(vec![]);
    }
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SaveMaterial> = materials_index
        .into_iter()
        .map(|m| SaveMaterialJson::to_save_material(&m))
        .collect();
    Ok(materials)
}

fn write_old_materials_physics<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(materials) = &vpx.gamedata.materials_physics_old {
        let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
        let mut materials_file = fs.create_file(&materials_path)?;
        let materials_index: Vec<SavePhysicsMaterialJson> = materials
            .iter()
            .map(SavePhysicsMaterialJson::from_save_physics_material)
            .collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    }
    Ok(())
}

fn read_old_materials_physics<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<SavePhysicsMaterial>>> {
    let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
    if !fs.exists(&materials_path) {
        return Ok(None);
    }
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<SavePhysicsMaterialJson> =
        serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SavePhysicsMaterial> = materials_index
        .into_iter()
        .map(|m| SavePhysicsMaterialJson::to_save_physics_material(&m))
        .collect();
    Ok(Some(materials))
}

/// Since it's common to change layer visibility we don't want that to cause a
/// difference in the item json, therefore we write this info in the index.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct GameItemInfoJson {
    file_name: String,
    // most require these, only lightsequencer does not
    #[serde(skip_serializing_if = "Option::is_none")]
    is_locked: Option<bool>,
    // most require these, only lightsequencer does not
    #[serde(skip_serializing_if = "Option::is_none")]
    editor_layer: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    editor_layer_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    editor_layer_visibility: Option<bool>,
}

/// Abstraction for making sure file names are unique
#[derive(Default)]
struct FileNameGen {
    used_names_lowercase: HashSet<String>,
}

impl FileNameGen {
    fn ensure_unique(&mut self, file_name: String) -> String {
        let lower_name = file_name.to_lowercase();
        if !self.used_names_lowercase.contains(&lower_name) {
            self.used_names_lowercase.insert(lower_name.clone());
            return file_name;
        }

        let mut counter = 1;
        let mut unique_name;
        loop {
            // There is a chance that the name we give is already used in one of the next files.
            // Therefore, we use double underscores to increase the chance it is unique.
            unique_name = format!("{file_name}__{counter}");
            let unique_name_lower = unique_name.to_lowercase();
            if !self.used_names_lowercase.contains(&unique_name_lower) {
                self.used_names_lowercase.insert(unique_name_lower);
                break;
            }
            counter += 1;
        }
        unique_name
    }
}

fn write_gameitems<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    fs.create_dir_all(&gameitems_dir)?;
    let mut file_name_gen = FileNameGen::default();
    let mut files: Vec<GameItemInfoJson> = Vec::new();
    let mut files_to_write: Vec<(String, usize)> = Vec::new();

    for (idx, gameitem) in vpx.gameitems.iter().enumerate() {
        let file_name = gameitem_filename_stem(&mut file_name_gen, gameitem);
        let file_name_json = format!("{}.json", &file_name);
        let gameitem_info = GameItemInfoJson {
            file_name: file_name_json.clone(),
            is_locked: gameitem.is_locked(),
            editor_layer: gameitem.editor_layer(),
            editor_layer_name: gameitem.editor_layer_name().clone(),
            editor_layer_visibility: gameitem.editor_layer_visibility(),
        };
        files.push(gameitem_info);

        let gameitem_path = gameitems_dir.join(&file_name_json);
        if fs.exists(&gameitem_path) {
            return Err(WriteError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("GameItem file already exists: {}", gameitem_path.display()),
            )));
        }

        files_to_write.push((file_name, idx));
    }

    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    let mut gameitems_index_file = fs.create_file(&gameitems_index_path)?;
    serde_json::to_writer_pretty(&mut gameitems_index_file, &files)?;

    let gameitems_ref = &vpx.gameitems;
    let gameitems_dir_clone = gameitems_dir.clone();

    let write_item = |(file_name, idx): &(String, usize)| -> Result<(), WriteError> {
        let file_name_json = format!("{}.json", file_name);
        let path = gameitems_dir_clone.join(&file_name_json);
        let gameitem = &gameitems_ref[*idx];

        let json_bytes = serde_json::to_vec_pretty(gameitem).map_err(WriteError::Json)?;
        fs.write_file(&path, &json_bytes)?;

        write_gameitem_binaries(&gameitems_dir_clone, gameitem, file_name, fs)?;

        Ok(())
    };

    #[cfg(feature = "parallel")]
    let results: Vec<Result<(), WriteError>> = files_to_write.par_iter().map(write_item).collect();

    #[cfg(not(feature = "parallel"))]
    let results: Vec<Result<(), WriteError>> = files_to_write.iter().map(write_item).collect();

    // Propagate the first error if any
    for r in results {
        r?;
    }

    Ok(())
}

fn gameitem_filename_stem(file_name_gen: &mut FileNameGen, gameitem: &GameItemEnum) -> String {
    let mut name = gameitem.name().to_string();
    if name.is_empty() {
        name = "unnamed".to_string();
    }
    // escape any characters that are not allowed in file names, for any os
    name = name.replace(|c: char| !c.is_alphanumeric(), "_");
    let file_name = format!("{}.{}", gameitem.type_name(), name);
    file_name_gen.ensure_unique(file_name)
}

fn compress_data(data: &[u8]) -> io::Result<Vec<u8>> {
    // before 10.6.1, compression was always LZW
    // "abuses the VP-Image-LZW compressor"
    // see https://github.com/vpinball/vpinball/commit/09f5510d676cd6b204350dfc4a93b9bf93284c56
    // Using default compression (level 6) instead of best (level 9) for better performance
    // Level 6 provides a good balance between speed and compression ratio
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn write_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem: &GameItemEnum,
    json_file_name: &str,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let GameItemEnum::Primitive(primitive) = gameitem
        && let Some(ReadMesh { vertices, indices }) = &primitive.read_mesh()?
    {
        let obj_path = gameitems_dir.join(format!("{json_file_name}.obj"));
        write_obj(
            gameitem.name().to_string(),
            vertices,
            indices,
            &obj_path,
            fs,
        )
        .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;

        if let Some(animation_frames) = &primitive.compressed_animation_vertices_data {
            if let Some(compressed_lengths) = &primitive.compressed_animation_vertices_len {
                let zipped = animation_frames.iter().zip(compressed_lengths.iter());
                write_animation_frames_to_objs(
                    gameitems_dir,
                    gameitem,
                    json_file_name,
                    vertices,
                    indices,
                    zipped,
                    fs,
                )?;
            } else {
                return Err(WriteError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Animation frames should always come with counts: {json_file_name}"),
                )));
            }
        }
    }
    Ok(())
}

fn write_animation_frames_to_objs(
    gameitems_dir: &Path,
    gameitem: &GameItemEnum,
    json_file_name: &str,
    vertices: &[([u8; 32], Vertex3dNoTex2)],
    indices: &[i64],
    zipped: Zip<Iter<Vec<u8>>, Iter<u32>>,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    for (i, (compressed_frame, compressed_length)) in zipped.enumerate() {
        let animation_frame_vertices =
            read_vpx_animation_frame(compressed_frame, compressed_length);
        let full_vertices = replace_vertices(vertices, animation_frame_vertices)?;
        let file_name_without_ext = json_file_name.trim_end_matches(".json");
        let file_name = animation_frame_file_name(file_name_without_ext, i);
        let obj_path = gameitems_dir.join(file_name);
        write_obj(
            gameitem.name().to_string(),
            &full_vertices,
            indices,
            &obj_path,
            fs,
        )
        .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
    }
    Ok(())
}

fn replace_vertices(
    vertices: &[([u8; 32], Vertex3dNoTex2)],
    animation_frame_vertices: Result<Vec<VertData>, WriteError>,
) -> Result<Vec<([u8; 32], Vertex3dNoTex2)>, WriteError> {
    // combine animation_vertices with the vertices and indices from the mesh
    let full_vertices = vertices
        .iter()
        .zip(animation_frame_vertices?.iter())
        .map(|((_, vertex), animation_vertex)| {
            let mut full_vertex: Vertex3dNoTex2 = (*vertex).clone();
            full_vertex.x = animation_vertex.x;
            full_vertex.y = animation_vertex.y;
            full_vertex.z = -animation_vertex.z;
            full_vertex.nx = animation_vertex.nx;
            full_vertex.ny = animation_vertex.ny;
            full_vertex.nz = -animation_vertex.nz;
            // TODO we don't have a full representation of the vertex, so we use a zeroed hash
            ([0u8; 32], full_vertex)
        })
        .collect::<Vec<_>>();
    Ok(full_vertices)
}

pub trait BytesMutExt {
    fn put_f32_le_nan_as_zero(&mut self, value: f32);
}

impl BytesMutExt for BytesMut {
    fn put_f32_le_nan_as_zero(&mut self, value: f32) {
        if value.is_nan() {
            // DieHard_272.vpx primitive "BM_pAirDuctGate" has a NaN value for nx
            // with value like [113, 93, 209, 255] in the vpx.
            // NaN is translated to 0.0 when exporting in vpinball windows.
            self.put_f32_le(0.0);
        } else {
            self.put_f32_le(value);
        }
    }
}

fn write_vertex(
    buff: &mut BytesMut,
    vertex: &Vertex3dNoTex2,
    vpx_vertex_normal_data: &Option<[u8; 12]>,
) {
    buff.put_f32_le(vertex.x);
    buff.put_f32_le(vertex.y);
    buff.put_f32_le(vertex.z);
    // normals
    if let Some(bytes) = vpx_vertex_normal_data {
        buff.put_slice(bytes);
    } else {
        buff.put_f32_le_nan_as_zero(vertex.nx);
        buff.put_f32_le_nan_as_zero(vertex.ny);
        buff.put_f32_le_nan_as_zero(vertex.nz);
    }
    // texture coordinates
    buff.put_f32_le(vertex.tu);
    buff.put_f32_le(vertex.tv);
}

fn write_vertex_index_for_vpx(bytes_per_index: u8, vpx_indices: &mut BytesMut, vertex_index: i64) {
    if bytes_per_index == 2 {
        vpx_indices.put_u16_le(vertex_index as u16);
    } else {
        vpx_indices.put_u32_le(vertex_index as u32);
    }
}

fn read_gameitems<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<GameItemEnum>> {
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    if !fs.exists(&gameitems_index_path) {
        info!("No gameitems.json found");
        return Ok(vec![]);
    }
    let gameitems_index: Vec<GameItemInfoJson> = read_json(gameitems_index_path, fs)?;
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");

    let read_item = |gameitem_info: GameItemInfoJson| -> io::Result<GameItemEnum> {
        read_game_item(gameitem_info, &gameitems_dir, fs)
    };

    #[cfg(feature = "parallel")]
    let results: Vec<io::Result<GameItemEnum>> =
        gameitems_index.into_par_iter().map(read_item).collect();

    #[cfg(not(feature = "parallel"))]
    let results: Vec<io::Result<GameItemEnum>> =
        gameitems_index.into_iter().map(read_item).collect();

    let mut out = Vec::with_capacity(results.len());
    for r in results {
        out.push(r?);
    }
    Ok(out)
}

#[instrument(skip(fs, gameitems_dir, gameitem_info), fields(path = ?&gameitem_info.file_name))]
fn read_game_item(
    gameitem_info: GameItemInfoJson,
    gameitems_dir: &Path,
    fs: &dyn FileSystem,
) -> io::Result<GameItemEnum> {
    let file_name = gameitem_info.file_name;
    let gameitem_path = gameitems_dir.join(&file_name);

    if !fs.exists(&gameitem_path) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("GameItem file not found: {}", gameitem_path.display()),
        ));
    }

    // read json and restore index-only metadata
    let mut item: GameItemEnum = read_json(&gameitem_path, fs)?;
    item.set_locked(gameitem_info.is_locked);
    item.set_editor_layer(gameitem_info.editor_layer);
    item.set_editor_layer_name(gameitem_info.editor_layer_name);
    item.set_editor_layer_visibility(gameitem_info.editor_layer_visibility);

    // read associated binaries (must be thread-safe; they operate on distinct files)
    read_gameitem_binaries(&gameitems_dir, file_name, item, fs)
}

fn read_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem_file_name: String,
    mut item: GameItemEnum,
    fs: &dyn FileSystem,
) -> io::Result<GameItemEnum> {
    if let GameItemEnum::Primitive(primitive) = &mut item {
        let gameitem_file_name = gameitem_file_name.trim_end_matches(".json");
        let obj_path = gameitems_dir.join(format!("{gameitem_file_name}.obj"));
        if fs.exists(&obj_path) {
            let (vertices_len, indices_len, compressed_vertices, compressed_indices) =
                read_obj(&obj_path, fs)?;
            primitive.num_vertices = Some(vertices_len as u32);
            primitive.compressed_vertices_len = Some(compressed_vertices.len() as u32);
            primitive.compressed_vertices_data = Some(compressed_vertices);
            primitive.num_indices = Some(indices_len as u32);
            primitive.compressed_indices_len = Some(compressed_indices.len() as u32);
            primitive.compressed_indices_data = Some(compressed_indices);
        }
        let frame0_file_name = animation_frame_file_name(gameitem_file_name, 0);
        let frame0_path = gameitems_dir.join(frame0_file_name);
        if fs.exists(&frame0_path) {
            let mut frame = 0;
            let mut frames = Vec::new();
            loop {
                let frame_path =
                    gameitems_dir.join(animation_frame_file_name(gameitem_file_name, frame));
                if fs.exists(&frame_path) {
                    let animation_frame = read_obj_as_frame(&frame_path, fs)?;
                    frames.push(animation_frame);
                    frame += 1;
                } else {
                    break;
                }
            }

            let mut compressed_lengths: Vec<u32> = Vec::with_capacity(frames.len());
            let mut compressed_animation_vertices: Vec<Vec<u8>> = Vec::with_capacity(frames.len());
            for animation_frame_vertices in frames {
                let mut buff = BytesMut::with_capacity(
                    animation_frame_vertices.len() * VertData::SERIALIZED_SIZE,
                );
                for vertex in animation_frame_vertices {
                    write_animation_vertex_data(&mut buff, &vertex);
                }
                let compressed_frame = compress_data(&buff)?;
                compressed_lengths.push(compressed_frame.len() as u32);
                compressed_animation_vertices.push(compressed_frame);
            }
            primitive.compressed_animation_vertices_len = Some(compressed_lengths);
            primitive.compressed_animation_vertices_data = Some(compressed_animation_vertices);
        }
    }
    Ok(item)
}

fn animation_frame_file_name(gameitem_file_name: &str, index: usize) -> String {
    format!("{gameitem_file_name}_anim_{index}.obj")
}

#[instrument(skip(fs))]
fn read_obj(obj_path: &Path, fs: &dyn FileSystem) -> io::Result<(usize, usize, Vec<u8>, Vec<u8>)> {
    let _span = info_span!("fs_read").entered();
    let obj_data = fs.read_file(obj_path)?;
    drop(_span);

    let _parse_span = info_span!("parse_obj").entered();
    let mut reader = io::BufReader::new(io::Cursor::new(obj_data));
    let ObjData {
        name: _,
        vertices,
        texture_coordinates,
        normals,
        indices,
    } = obj_read_obj(&mut reader).map_err(|e| {
        io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e))
    })?;
    drop(_parse_span);

    let _convert_span = info_span!("convert_vertices", count = vertices.len()).entered();
    let mut vpx_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for ((v, vt), vn) in vertices
        .iter()
        .zip(texture_coordinates.iter())
        .zip(normals.iter())
    {
        let (normal, vpx_vertex_normal_data) = vn;
        let nx = normal.0 as f32;
        let ny = normal.1 as f32;
        let nz = -(normal.2 as f32);

        let vertext = Vertex3dNoTex2 {
            x: v.0 as f32,
            y: v.1 as f32,
            z: -(v.2 as f32),
            nx,
            ny,
            nz,
            tu: vt.0 as f32,
            tv: vt.1.unwrap_or(0.0) as f32,
        };
        write_vertex(&mut vpx_vertices, &vertext, vpx_vertex_normal_data);
    }
    drop(_convert_span);

    let _index_span = info_span!("convert_indices", count = indices.len()).entered();
    let bytes_per_index: u8 = if vertices.len() > MAX_VERTICES_FOR_2_BYTE_INDEX {
        4
    } else {
        2
    };
    let mut vpx_indices = BytesMut::with_capacity(indices.len() * bytes_per_index as usize);
    for chunk in indices.chunks(3) {
        let v1 = chunk[0];
        let v2 = chunk[1];
        let v3 = chunk[2];
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v3);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v2);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v1);
    }
    drop(_index_span);

    let vertices_len = vertices.len();
    let indices_len = indices.len();

    let _compress_span = info_span!(
        "compress_data",
        vertex_bytes = vpx_vertices.len(),
        index_bytes = vpx_indices.len()
    )
    .entered();

    #[cfg(feature = "parallel")]
    let (compressed_vertices, compressed_indices) = rayon::join(
        || compress_data(&vpx_vertices),
        || compress_data(&vpx_indices),
    );

    #[cfg(not(feature = "parallel"))]
    let (compressed_vertices, compressed_indices) =
        (compress_data(&vpx_vertices), compress_data(&vpx_indices));

    let compressed_vertices = compressed_vertices?;
    let compressed_indices = compressed_indices?;
    drop(_compress_span);

    Ok((
        vertices_len,
        indices_len,
        compressed_vertices,
        compressed_indices,
    ))
}

#[instrument(skip(fs))]
fn read_obj_as_frame(obj_path: &Path, fs: &dyn FileSystem) -> io::Result<Vec<VertData>> {
    let obj_data = fs.read_file(obj_path)?;
    let mut reader = io::BufReader::new(io::Cursor::new(obj_data));
    let ObjData {
        name: _,
        vertices: obj_vertices,
        texture_coordinates: _,
        normals,
        indices: _,
    } = obj_read_obj(&mut reader).map_err(|e| {
        io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e))
    })?;
    let mut vertices: Vec<VertData> = Vec::with_capacity(obj_vertices.len());
    for (v, vn) in obj_vertices.iter().zip(normals.iter()) {
        let (normal, _) = vn;
        let nx = normal.0 as f32;
        let ny = normal.1 as f32;
        let nz = -(normal.2 as f32);
        let vertext = VertData {
            x: v.0 as f32,
            y: v.1 as f32,
            z: -(v.2 as f32),
            nx,
            ny,
            nz,
        };
        vertices.push(vertext);
    }
    Ok(vertices)
}

fn write_info<P: AsRef<Path>>(
    vpx: &&VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let json_path = expanded_dir.as_ref().join("info.json");
    let mut json_file = fs.create_file(&json_path)?;
    let info = info_to_json(&vpx.info, &vpx.custominfotags);
    serde_json::to_writer_pretty(&mut json_file, &info)?;
    Ok(())
}

fn read_info<P: AsRef<Path>>(
    expanded_dir: &P,
    screenshot: Option<Vec<u8>>,
    fs: &dyn FileSystem,
) -> io::Result<(TableInfo, CustomInfoTags)> {
    let info_path = expanded_dir.as_ref().join("info.json");
    if !fs.exists(&info_path) {
        return Ok((TableInfo::default(), CustomInfoTags::default()));
    }
    let value: Value = read_json(&info_path, fs)?;
    let (info, custominfotags) = json_to_info(value, screenshot)?;
    Ok((info, custominfotags))
}

fn read_collections<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<Collection>> {
    let collections_path = expanded_dir.as_ref().join("collections.json");
    if !fs.exists(&collections_path) {
        info!("No collections.json found");
        return Ok(vec![]);
    }
    let value = read_json(collections_path, fs)?;
    let collections: Vec<Collection> = json_to_collections(value)?;
    Ok(collections)
}

fn read_renderprobes<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<RenderProbeWithGarbage>>> {
    let renderprobes_path = expanded_dir.as_ref().join("renderprobes.json");
    if !fs.exists(&renderprobes_path) {
        return Ok(None);
    }
    let value: Vec<RenderProbeJson> = read_json(renderprobes_path, fs)?;
    let renderprobes = value.iter().map(|v| v.to_renderprobe()).collect();
    Ok(Some(renderprobes))
}

fn write_renderprobes<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(renderprobes) = &vpx.gamedata.render_probes {
        let renderprobes_path = expanded_dir.as_ref().join("renderprobes.json");
        let mut renderprobes_file = fs.create_file(&renderprobes_path)?;
        let renderprobes_index: Vec<RenderProbeJson> = renderprobes
            .iter()
            .map(RenderProbeJson::from_renderprobe)
            .collect();
        serde_json::to_writer_pretty(&mut renderprobes_file, &renderprobes_index)?;
    }
    Ok(())
}

pub fn extract_directory_list(vpx_file_path: &Path) -> Vec<String> {
    let root_dir_path_str = vpx_file_path.with_extension("");
    let root_dir_path = Path::new(&root_dir_path_str);
    let root_dir_parent = root_dir_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut comp = cfb::open(vpx_file_path).unwrap();
    let version = version::read_version(&mut comp).unwrap();
    let gamedata = read_gamedata(&mut comp, &version).unwrap();

    let mut files: Vec<String> = Vec::new();

    let images_path = root_dir_path.join("images");
    let images_size = gamedata.images_size;
    for index in 0..images_size {
        let path = format!("GameStg/Image{index}");
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let mut reader = BiffReader::new(&input);
        let img = ImageData::biff_read(&mut reader);

        let mut jpeg_path = images_path.clone();
        let ext = img.ext();

        jpeg_path.push(format!("{}.{}", img.name, ext));

        files.push(jpeg_path.to_string_lossy().to_string());
    }
    if images_size == 0 {
        files.push(
            images_path
                .join(std::path::MAIN_SEPARATOR_STR)
                .to_string_lossy()
                .to_string(),
        );
    }

    let sounds_size = gamedata.sounds_size;
    let sounds_path = root_dir_path.join("sounds");
    for index in 0..sounds_size {
        let path = format!("GameStg/Sound{index}");
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let mut reader = BiffReader::new(&input);
        let sound = sound::read(&version, &mut reader);

        let ext = sound.ext();
        let mut sound_path = sounds_path.clone();
        sound_path.push(format!("{}.{}", sound.name, ext));

        files.push(sound_path.to_string_lossy().to_string());
    }
    if sounds_size == 0 {
        files.push(
            sounds_path
                .join(std::path::MAIN_SEPARATOR_STR)
                .to_string_lossy()
                .to_string(),
        );
    }

    let fonts_size = gamedata.fonts_size;
    let fonts_path = root_dir_path.join("fonts");
    for index in 0..fonts_size {
        let path = format!("GameStg/Font{index}");
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let font = font::read(&input);

        let ext = font.ext();
        let mut font_path = fonts_path.clone();
        font_path.push(format!("Font{}.{}.{}", index, font.name, ext));

        files.push(font_path.to_string_lossy().to_string());
    }
    if fonts_size == 0 {
        files.push(fonts_path.to_string_lossy().to_string());
    }

    let entries = retrieve_entries_from_compound_file(&comp);
    entries.iter().for_each(|path| {
        // write the steam directly to a file
        let file_path = root_dir_path.join(&path[1..]);
        // println!("Writing to {}", file_path.display());
        files.push(file_path.to_string_lossy().to_string());
    });

    let gameitems_path = root_dir_path.join("gameitems");
    let gameitems_size = gamedata.gameitems_size;
    let mut file_name_gen = FileNameGen::default();
    for index in 0..gameitems_size {
        let path = format!("GameStg/GameItem{index}");
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let gameitem = gameitem::read(&input);
        let mut gameitem_path = gameitems_path.clone();
        let file_name_stem = gameitem_filename_stem(&mut file_name_gen, &gameitem);
        gameitem_path.push(format!("{file_name_stem}.json"));
        files.push(gameitem_path.to_string_lossy().to_string());
    }

    files.sort();

    // These files are made by:

    // -extract_script
    files.push(
        root_dir_path
            .join("script.vbs")
            .to_string_lossy()
            .to_string(),
    );
    // -extract_collections
    files.push(
        root_dir_path
            .join("collections.json")
            .to_string_lossy()
            .to_string(),
    );
    // -extract_info
    files.push(
        root_dir_path
            .join("TableInfo.json")
            .to_string_lossy()
            .to_string(),
    );

    files = files
        .into_iter()
        .map(|file_path| {
            if let Some(relative_path) = file_path.strip_prefix(&root_dir_parent) {
                relative_path.to_string()
            } else {
                file_path.clone()
            }
        })
        .collect::<Vec<String>>();

    files
}

fn retrieve_entries_from_compound_file(comp: &CompoundFile<File>) -> Vec<String> {
    let entries: Vec<String> = comp
        .walk()
        .filter(|entry| {
            entry.is_stream()
                && !entry.path().starts_with("/TableInfo")
                && !entry.path().starts_with("/GameStg/MAC")
                && !entry.path().starts_with("/GameStg/Version")
                && !entry.path().starts_with("/GameStg/GameData")
                && !entry.path().starts_with("/GameStg/CustomInfoTags")
                && !entry
                    .path()
                    .to_string_lossy()
                    .starts_with("/GameStg/GameItem")
                && !entry.path().to_string_lossy().starts_with("/GameStg/Font")
                && !entry.path().to_string_lossy().starts_with("/GameStg/Image")
                && !entry.path().to_string_lossy().starts_with("/GameStg/Sound")
                && !entry
                    .path()
                    .to_string_lossy()
                    .starts_with("/GameStg/Collection")
        })
        .map(|entry| {
            let path = entry.path();
            let path = path.to_str().unwrap();
            //println!("{} {} {}", path, entry.is_stream(), entry.len());
            path.to_owned()
        })
        .collect();

    entries
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::filesystem::MemoryFileSystem;
    use crate::vpx::gameitem;
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::primitive::Primitive;
    use crate::vpx::image::ImageDataJpeg;
    use crate::vpx::sound::{OutputTarget, WaveForm};
    use crate::vpx::tableinfo::TableInfo;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::io::BufReader;
    //use testdir::testdir;
    use testresult::TestResult;

    // Encoded data for 2x2 argb with alpha always 0xFF because the vpinball
    // bmp export does not support alpha channel.
    // See lzw_writer tests on what colors these are.
    const LZW_COMPRESSED_DATA: [u8; 14] =
        [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4];

    #[test]
    pub fn test_write_read_bmp() -> TestResult {
        let fs = MemoryFileSystem::default();
        let bmp_path = Path::new("test_image.bmp");

        write_image_bmp(bmp_path, &LZW_COMPRESSED_DATA, 2, 2, &fs)?;

        let file_bytes = fs.read_file(bmp_path)?;
        let read_compressed_data = read_image_bmp(&file_bytes)?;

        assert_eq!(2, read_compressed_data.width);
        assert_eq!(2, read_compressed_data.height);

        assert_eq!(
            LZW_COMPRESSED_DATA,
            *read_compressed_data.lzw_compressed_data
        );
        Ok(())
    }

    #[test]
    pub fn test_swap_red_and_blue() {
        let rgba = vec![1, 2, 3, 255];
        let bgra = swap_red_and_blue(&rgba);
        assert_eq!(bgra, vec![3, 2, 1, 255]);
        // a second time should be the same as the original
        let rgba2 = swap_red_and_blue(&bgra);
        assert_eq!(rgba2, rgba);
    }

    const SCREENSHOT_DATA: &[u8] = include_bytes!("../../testdata/1x1.png");

    #[test]
    pub fn test_expand_write_read() -> TestResult {
        let fs = MemoryFileSystem::default();

        // read 1x1.png as a Vec<u8>
        let screenshot = SCREENSHOT_DATA.to_vec();

        let version = Version::new(1074);

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
        write_fs(&vpx, &path, &fs)?;

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
    fn test_file_name_gen() {
        let mut file_name_gen = FileNameGen::default();
        let first = file_name_gen.ensure_unique("test".to_string());
        assert_eq!("test".to_string(), first);
        let second = file_name_gen.ensure_unique("test".to_string());
        assert_eq!("test__1".to_string(), second);
        let other = file_name_gen.ensure_unique("test1".to_string());
        assert_eq!("test1".to_string(), other);
        let future = file_name_gen.ensure_unique("test__2".to_string());
        assert_eq!("test__2".to_string(), future);
        let last = file_name_gen.ensure_unique("test".to_string());
        assert_eq!("test__3".to_string(), last);
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_read_image_dimensions_png_as_hdr_stream() {
        // this file is actually a png file but with hdr extension
        // see https://github.com/francisdb/vpin/issues/110
        let hdr_path = Path::new("testdata").join("wrongly_labeled_png.hdr");
        let file = File::open(&hdr_path).unwrap();
        let reader = BufReader::new(file);
        let dimensions =
            read_image_dimensions_from_file_steam("wrongly_labeled_png.hdr", reader).unwrap();

        assert_eq!(dimensions, Some((512, 256)));
    }

    #[test]
    fn test_read_image_dimensions_fail_invalid_unknown() {
        let cursor = io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.zero", reader).unwrap();

        assert_eq!(dimensions, None);
    }

    #[test]
    fn test_read_image_dimensions_fail_invalid_png() {
        let cursor = io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.png", reader).unwrap();

        assert_eq!(dimensions, None);
    }
}
