use bytes::{BufMut, BytesMut};
use std::collections::HashSet;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io::{self, BufRead, Read, Seek, Write};
use std::iter::Zip;
use std::slice::Iter;
use std::{fs::File, path::Path};

use cfb::CompoundFile;
use image::DynamicImage;
use serde::de;
use serde_json::Value;

use super::{VPX, Version, gameitem, read_gamedata};

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
use crate::vpx::obj::{ObjData, read_obj_file, write_obj};
use crate::vpx::renderprobe::{RenderProbeJson, RenderProbeWithGarbage};
use crate::vpx::tableinfo::TableInfo;

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
    // write the version as utf8 to version.txt
    let version_path = expanded_dir.as_ref().join("version.txt");
    let mut version_file = File::create(version_path)?;
    let version_string = vpx.version.to_u32_string();
    version_file.write_all(version_string.as_bytes())?;

    // write the screenshot as a png
    if let Some(screenshot) = &vpx.info.screenshot {
        let screenshot_path = expanded_dir.as_ref().join("screenshot.png");
        let mut screenshot_file = File::create(screenshot_path)?;
        screenshot_file.write_all(screenshot)?;
    }

    // write table metadata as json
    write_info(&vpx, expanded_dir)?;

    // collections
    let collections_json_path = expanded_dir.as_ref().join("collections.json");
    let mut collections_json_file = File::create(collections_json_path)?;
    let json_collections = collections_json(&vpx.collections);
    serde_json::to_writer_pretty(&mut collections_json_file, &json_collections)?;
    write_gameitems(vpx, expanded_dir)?;
    write_images(vpx, expanded_dir)?;
    write_sounds(vpx, expanded_dir)?;
    write_fonts(vpx, expanded_dir)?;
    write_game_data(vpx, expanded_dir)?;
    if vpx.gamedata.materials.is_some() {
        write_materials(vpx, expanded_dir)?;
    } else {
        write_old_materials(vpx, expanded_dir)?;
        write_old_materials_physics(vpx, expanded_dir)?;
    }
    write_renderprobes(vpx, expanded_dir)?;
    Ok(())
}

pub fn read<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<VPX> {
    // read the version
    let version_path = expanded_dir.as_ref().join("version.txt");
    if !version_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Version file not found: {}", version_path.display()),
        ));
    }
    let mut version_file = File::open(&version_path)?;
    let mut version_string = String::new();
    version_file.read_to_string(&mut version_string)?;
    let version = Version::parse(&version_string).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Could not parse version {}: {}", &version_string, e),
        )
    })?;

    let screenshot = expanded_dir.as_ref().join("screenshot.png");
    let screenshot = if screenshot.exists() {
        let mut screenshot_file = File::open(&screenshot)?;
        let mut screenshot = Vec::new();
        screenshot_file.read_to_end(&mut screenshot)?;
        Some(screenshot)
    } else {
        None
    };

    let (info, custominfotags) = read_info(expanded_dir, screenshot)?;
    let collections = read_collections(expanded_dir)?;
    let gameitems = read_gameitems(expanded_dir)?;
    let images = read_images(expanded_dir)?;
    let sounds = read_sounds(expanded_dir)?;
    let fonts = read_fonts(expanded_dir)?;
    let mut gamedata = read_game_data(expanded_dir)?;
    gamedata.collections_size = collections.len() as u32;
    gamedata.gameitems_size = gameitems.len() as u32;
    gamedata.images_size = images.len() as u32;
    gamedata.sounds_size = sounds.len() as u32;
    gamedata.fonts_size = fonts.len() as u32;
    let materials_opt = read_materials(expanded_dir)?;
    match materials_opt {
        Some(materials) => {
            // we might want to warn if the other old material files are present
            gamedata.materials_old = materials.iter().map(SaveMaterial::from).collect();
            gamedata.materials_physics_old =
                Some(materials.iter().map(SavePhysicsMaterial::from).collect());
            gamedata.materials_size = materials.len() as u32;
            gamedata.materials = Some(materials);
        }
        None => {
            gamedata.materials_old = read_old_materials(expanded_dir)?;
            gamedata.materials_physics_old = read_old_materials_physics(expanded_dir)?;
            gamedata.materials_size = gamedata.materials_old.len() as u32;
        }
    }
    gamedata.render_probes = read_renderprobes(expanded_dir)?;

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
    Ok(vpx)
}

fn write_game_data<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let mut game_data_file = File::create(game_data_path)?;
    let json = GameDataJson::from_game_data(&vpx.gamedata);
    serde_json::to_writer_pretty(&mut game_data_file, &json)?;
    // write the code to script.vbs
    let script_path = expanded_dir.as_ref().join("script.vbs");
    let mut script_file = File::create(script_path)?;
    let script_bytes: Vec<u8> = vpx.gamedata.code.clone().into();
    script_file.write_all(script_bytes.as_ref())?;
    Ok(())
}

fn read_game_data<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<GameData> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let game_data_json: GameDataJson = read_json(game_data_path)?;
    let mut game_data = game_data_json.to_game_data();
    // read the code from script.vbs, and find out the correct encoding
    let script_path = expanded_dir.as_ref().join("script.vbs");
    if !script_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Script file not found: {}", script_path.display()),
        ));
    }
    let mut script_file = File::open(&script_path)?;
    let mut code = Vec::new();
    script_file.read_to_end(&mut code)?;
    game_data.code = code.into();
    Ok(game_data)
}

fn read_json<P: AsRef<Path>, T>(game_data_path: P) -> io::Result<T>
where
    T: de::DeserializeOwned,
{
    let path = game_data_path.as_ref();
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Game data file not found: {}", path.display()),
        ));
    }
    let mut game_data_file = File::open(&game_data_path)?;
    serde_json::from_reader(&mut game_data_file).map_err(|e| {
        io::Error::other(format!(
            "Failed to parse/read json {}: {}",
            path.display(),
            e
        ))
    })
}

fn write_images<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    // create an image index
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let mut images_index_file = File::create(images_index_path)?;
    // on macOS/windows the file system is case-insensitive
    let mut image_names_lower: HashSet<String> = HashSet::new();
    let mut image_names_dupe_counter = 0;
    let mut json_images = Vec::with_capacity(vpx.sounds.len());
    let images: io::Result<Vec<(String, &ImageData)>> = vpx
        .images
        .iter()
        .map(|image| {
            let mut json = ImageDataJson::from_image_data(image);
            let lower_name = image.name.to_lowercase();
            if image_names_lower.contains(&lower_name) {
                image_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", image.name, image_names_dupe_counter);
                eprintln!(
                    "Image name {} is not unique, renaming file to {}",
                    image.name, &name_dedup
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
                            eprintln!(
                                "Image dimension override for {} in vpx {}x{} vs in image {}x{}",
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
            Ok((file_name, image))
        })
        .collect();
    let images = images?;
    serde_json::to_writer_pretty(&mut images_index_file, &json_images)?;

    let images_dir = expanded_dir.as_ref().join("images");
    std::fs::create_dir_all(&images_dir)?;
    images.iter().try_for_each(|(image_file_name, image)| {
        let file_path = images_dir.join(image_file_name);
        if !file_path.exists() {
            let mut file = File::create(&file_path)?;
            if image.is_link() {
                Ok(())
            } else if let Some(jpeg) = &image.jpeg {
                file.write_all(&jpeg.data)
            } else if let Some(bits) = &image.bits {
                // the extension should be .bmp
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
                )
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Image has no data: {}", file_path.display()),
                ))
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Two images with the same name detected, should not happen: {}",
                    file_path.display()
                ),
            ))
        }
    })?;
    Ok(())
}

fn write_image_bmp(
    file_path: &Path,
    lzw_compressed_data: &[u8],
    width: u32,
    height: u32,
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
        eprintln!(
            "Image {file_name} has non-opaque pixels, writing as RGBA BMP that might not be supported by all applications"
        );
    }
    image_to_save.save(file_path).map_err(|image_error| {
        io::Error::other(format!(
            "Failed to write bitmap to {}: {}",
            file_path.display(),
            image_error
        ))
    })
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

fn read_images<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<ImageData>> {
    // TODO do we actually need an index?
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let images_index_json: Vec<ImageDataJson> = read_json(images_index_path)?;
    let images_dir = expanded_dir.as_ref().join("images");
    let images: io::Result<Vec<ImageData>> = images_index_json
        .into_iter()
        .map(|image_data_json| {
            if image_data_json.is_link() {
                // linked images have no data
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

                // We also support webp in case the image is a png, this was requested by a user
                // https://github.com/francisdb/vpxtool/issues/521
                let mut new_extension = None;
                if image_data_json.ext() == "png" && !file_path.exists() {
                    let file_path_webp = images_dir.join(format!("{file_name}.webp"));
                    if file_path_webp.exists() {
                        new_extension = Some("webp");
                        file_path = file_path_webp;
                    }
                }

                if file_path.exists() {
                    let mut image_file = File::open(&file_path)?;
                    let mut image_data = Vec::new();
                    image_file.read_to_end(&mut image_data)?;
                    let image = if image_data_json.is_bmp() {
                        let read_bmp = read_image_bmp(&image_data)?;
                        // the json serializer makes sure we have a Some with empty data
                        let image_data = ImageDataBits {
                            lzw_compressed_data: read_bmp.lzw_compressed_data,
                        };
                        // For now we don't support width and height overrides for BMPs
                        // as we have not encountered any in the wild.
                        image_data_json.to_image_data(
                            read_bmp.width,
                            read_bmp.height,
                            Some(image_data),
                        )
                    } else {
                        // use image library to get the actual dimensions
                        let dimensions_from_file = read_image_dimensions(&file_path)?;

                        let width = match image_data_json.width {
                            Some(w) => {
                                if let Some((image_w, _)) = dimensions_from_file {
                                    if w != image_w {
                                        eprintln!(
                                            "Image width override for {full_file_name} in json ({w}) vs in image ({image_w})"
                                        );
                                    }
                                }
                                w
                            }
                            None =>
                                match dimensions_from_file {
                                    Some((width_file, _)) => width_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image width not provided and could not be read from file")),
                                }
                        };

                        let height = match image_data_json.height {
                            Some(h) => {
                                if let Some((_, image_h)) = dimensions_from_file {
                                    if h != image_h {
                                        eprintln!(
                                            "Image height override for {full_file_name} in json ({h}) vs in image ({image_h})"
                                        );
                                    }
                                }
                                h
                            }
                            None =>
                                match dimensions_from_file {
                                    Some((_, height_file)) => height_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image height not provided and could not be read from file")),
                                }
                        };

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

fn read_image_dimensions(file_path: &Path) -> io::Result<Option<(u32, u32)>> {
    let decoder = image::ImageReader::open(file_path)?.with_guessed_format()?;
    let dimensions_from_file = match decoder.into_dimensions() {
        Ok(dimensions) => Some(dimensions),
        Err(image_error) => {
            // one issue we encountered is https://github.com/image-rs/image/issues/2231
            eprintln!(
                "Failed to read image dimensions for {}: {}",
                file_path.display(),
                image_error
            );
            None
        }
    };
    Ok(dimensions_from_file)
}

fn read_image_dimensions_from_file_steam<R: BufRead + Seek>(
    file_name: &str,
    reader: R,
) -> io::Result<Option<(u32, u32)>> {
    let dimensions_from_file = match image::ImageFormat::from_path(file_name) {
        Ok(format) => {
            let decoder = image::ImageReader::with_format(reader, format).with_guessed_format()?;
            if Some(format) != decoder.format() {
                eprintln!(
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
                    eprintln!("Failed to read image dimensions for {file_name}: {image_error}");
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to determine image format for {file_name}: {e}");
            None
        }
    };
    Ok(dimensions_from_file)
}

struct ImageBmp {
    width: u32,
    height: u32,
    lzw_compressed_data: Vec<u8>,
}

fn read_image_bmp(data: &[u8]) -> io::Result<ImageBmp> {
    let image = image::load_from_memory_with_format(data, image::ImageFormat::Bmp).map_err(
        |image_error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read BMP image: {image_error}"),
            )
        },
    )?;

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

fn write_sounds<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let sounds_index_path = expanded_dir.as_ref().join("sounds.json");
    let mut sounds_index_file = File::create(sounds_index_path)?;
    // on macOS/windows the file system is case-insensitive
    let mut sound_names_lower: HashSet<String> = HashSet::new();
    let mut sound_names_dupe_counter = 0;
    let mut json_sounds = Vec::with_capacity(vpx.sounds.len());
    let sounds: Vec<(String, &SoundData)> = vpx
        .sounds
        .iter()
        .map(|sound| {
            let mut json = SoundDataJson::from_sound_data(sound);
            let lower_name = sound.name.to_lowercase();
            if sound_names_lower.contains(&lower_name) {
                sound_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", sound.name, sound_names_dupe_counter);
                eprintln!(
                    "Sound name {} is not unique, renaming file to {}",
                    sound.name, &name_dedup
                );
                json.name_dedup = Some(name_dedup);
            }
            sound_names_lower.insert(lower_name);

            let actual_name = json.name_dedup.as_ref().unwrap_or(&sound.name);
            let file_name = format!("{}.{}", actual_name, sound.ext());
            json_sounds.push(json);
            (file_name, sound)
        })
        .collect();
    serde_json::to_writer_pretty(&mut sounds_index_file, &json_sounds)?;

    let sounds_dir = expanded_dir.as_ref().join("sounds");
    std::fs::create_dir_all(&sounds_dir)?;
    sounds.iter().try_for_each(|(sound_file_name, sound)| {
        let sound_path = sounds_dir.join(sound_file_name);
        if !sound_path.exists() {
            let mut file = File::create(sound_path)?;
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

fn read_sounds<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<SoundData>> {
    let sounds_json_path = expanded_dir.as_ref().join("sounds.json");
    if !sounds_json_path.exists() {
        println!("No sounds.json found");
        return Ok(vec![]);
    }
    let sounds_json: Vec<SoundDataJson> = read_json(&sounds_json_path)?;
    // for each item in the index read the items
    let sounds_dir = expanded_dir.as_ref().join("sounds");
    let sounds: io::Result<Vec<SoundData>> = sounds_json
        .into_iter()
        .map(|sound_data_json| {
            let mut sound = sound_data_json.to_sound_data();
            let file_name = sound_data_json.name_dedup.as_ref().unwrap_or(&sound.name);
            let full_file_name = format!("{}.{}", file_name, sound.ext());
            let file_path = sounds_dir.join(full_file_name);
            if file_path.exists() {
                let mut sound_file = File::open(&file_path)?;
                let mut sound_data = Vec::new();
                sound_file.read_to_end(&mut sound_data)?;
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

fn write_fonts<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let fonts_json_path = expanded_dir.as_ref().join("fonts.json");
    let mut fonts_index_file = File::create(fonts_json_path)?;
    let fonts_index: Vec<FontDataJson> =
        vpx.fonts.iter().map(FontDataJson::from_font_data).collect();
    serde_json::to_writer_pretty(&mut fonts_index_file, &fonts_index)?;

    let fonts_dir = expanded_dir.as_ref().join("fonts");
    std::fs::create_dir_all(&fonts_dir)?;
    vpx.fonts.iter().try_for_each(|font| {
        let file_name = format!("{}.{}", font.name, font.ext());
        let font_path = fonts_dir.join(file_name);
        let mut file = File::create(font_path)?;
        file.write_all(&font.data)
    })?;
    Ok(())
}

fn read_fonts<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<FontData>> {
    let fonts_index_path = expanded_dir.as_ref().join("fonts.json");
    if !fonts_index_path.exists() {
        println!("No fonts.json found");
        return Ok(vec![]);
    }
    let fonts_json: Vec<FontDataJson> = read_json(fonts_index_path)?;
    let fonts_index: Vec<FontData> = fonts_json
        .iter()
        .map(|font_data_json| font_data_json.to_font_data())
        .collect();
    // for each item in the index read the items
    let fonts_dir = expanded_dir.as_ref().join("fonts");
    let fonts: io::Result<Vec<FontData>> = fonts_index
        .into_iter()
        .map(|mut font| {
            let file_name = format!("{}.{}", font.name, font.ext());
            let font_path = fonts_dir.join(file_name);
            if font_path.exists() {
                let mut font_file = File::open(&font_path)?;
                let mut font_data = Vec::new();
                font_file.read_to_end(&mut font_data)?;
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

fn write_materials<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    if let Some(materials) = &vpx.gamedata.materials {
        let materials_path = expanded_dir.as_ref().join("materials.json");
        let mut materials_file = File::create(materials_path)?;
        let materials_index: Vec<MaterialJson> =
            materials.iter().map(MaterialJson::from_material).collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    }
    Ok(())
}

fn read_materials<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Option<Vec<Material>>> {
    let materials_path = expanded_dir.as_ref().join("materials.json");
    if !materials_path.exists() {
        return Ok(None);
    }
    let materials_file = File::open(&materials_path)?;
    let materials_index: Vec<MaterialJson> = serde_json::from_reader(materials_file)?;
    let materials: Vec<Material> = materials_index
        .into_iter()
        .map(|m| MaterialJson::to_material(&m))
        .collect();
    Ok(Some(materials))
}

fn write_old_materials<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    let mut materials_file = File::create(materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = vpx
        .gamedata
        .materials_old
        .iter()
        .map(SaveMaterialJson::from_save_material)
        .collect();
    serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    Ok(())
}

fn read_old_materials<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<SaveMaterial>> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    if !materials_path.exists() {
        return Ok(vec![]);
    }
    let materials_file = File::open(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = serde_json::from_reader(materials_file)?;
    let materials: Vec<SaveMaterial> = materials_index
        .into_iter()
        .map(|m| SaveMaterialJson::to_save_material(&m))
        .collect();
    Ok(materials)
}

fn write_old_materials_physics<P: AsRef<Path>>(
    vpx: &VPX,
    expanded_dir: &P,
) -> Result<(), WriteError> {
    if let Some(materials) = &vpx.gamedata.materials_physics_old {
        let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
        let mut materials_file = File::create(materials_path)?;
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
) -> io::Result<Option<Vec<SavePhysicsMaterial>>> {
    let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
    if !materials_path.exists() {
        return Ok(None);
    }
    let materials_file = File::open(&materials_path)?;
    let materials_index: Vec<SavePhysicsMaterialJson> = serde_json::from_reader(materials_file)?;
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

fn write_gameitems<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    std::fs::create_dir_all(&gameitems_dir)?;
    let mut file_name_gen = FileNameGen::default();
    let mut files: Vec<GameItemInfoJson> = Vec::new();
    for gameitem in &vpx.gameitems {
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
        let gameitem_path = gameitems_dir.join(file_name_json);
        // should not happen but we keep the check
        if gameitem_path.exists() {
            return Err(WriteError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("GameItem file already exists: {}", gameitem_path.display()),
            )));
        }
        let gameitem_file = File::create(&gameitem_path)?;
        serde_json::to_writer_pretty(&gameitem_file, &gameitem)?;
        write_gameitem_binaries(&gameitems_dir, gameitem, file_name)?;
    }
    // write the gameitems index as array with names being the type and the name
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    let mut gameitems_index_file = File::create(gameitems_index_path)?;
    serde_json::to_writer_pretty(&mut gameitems_index_file, &files)?;
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
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

/// for primitives we write fields m3cx, m3ci and m3ay's to separate files with bin extension
fn write_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem: &GameItemEnum,
    json_file_name: String,
) -> Result<(), WriteError> {
    if let GameItemEnum::Primitive(primitive) = gameitem {
        // use wavefront-rs to write the vertices and indices
        // we first have to decompress the data as they are stored compressed
        if let Some(ReadMesh { vertices, indices }) = &primitive.read_mesh()? {
            let obj_path = gameitems_dir.join(format!("{json_file_name}.obj"));
            write_obj(gameitem.name().to_string(), vertices, indices, &obj_path)
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;

            if let Some(animation_frames) = &primitive.compressed_animation_vertices_data {
                if let Some(compressed_lengths) = &primitive.compressed_animation_vertices_len {
                    // zip frames with the counts
                    let zipped = animation_frames.iter().zip(compressed_lengths.iter());
                    write_animation_frames_to_objs(
                        gameitems_dir,
                        gameitem,
                        &json_file_name,
                        vertices,
                        indices,
                        zipped,
                    )?;
                } else {
                    return Err(WriteError::Io(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "Animation frames should always come with counts: {json_file_name}"
                        ),
                    )));
                }
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
) -> Result<(), WriteError> {
    for (i, (compressed_frame, compressed_length)) in zipped.enumerate() {
        let animation_frame_vertices =
            read_vpx_animation_frame(compressed_frame, compressed_length);
        let full_vertices = replace_vertices(vertices, animation_frame_vertices)?;
        // The file name of the sequence must be <meshname>_x.obj where x is the frame number.
        let file_name_without_ext = json_file_name.trim_end_matches(".json");
        let file_name = animation_frame_file_name(file_name_without_ext, i);
        let obj_path = gameitems_dir.join(file_name);
        write_obj(
            gameitem.name().to_string(),
            &full_vertices,
            indices,
            &obj_path,
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

fn read_gameitems<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<GameItemEnum>> {
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    if !gameitems_index_path.exists() {
        println!("No gameitems.json found");
        return Ok(vec![]);
    }
    let gameitems_index: Vec<GameItemInfoJson> = read_json(gameitems_index_path)?;
    // for each item in the index read the items
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    let gameitems: io::Result<Vec<GameItemEnum>> = gameitems_index
        .into_iter()
        .map(|gameitem_info| {
            let gameitem_path = gameitems_dir.join(&gameitem_info.file_name);
            if gameitem_path.exists() {
                let mut item: GameItemEnum = read_json(&gameitem_path)?;
                item.set_locked(gameitem_info.is_locked);
                item.set_editor_layer(gameitem_info.editor_layer);
                item.set_editor_layer_name(gameitem_info.editor_layer_name);
                item.set_editor_layer_visibility(gameitem_info.editor_layer_visibility);
                read_gameitem_binaries(&gameitems_dir, gameitem_info.file_name, item)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("GameItem file not found: {}", gameitem_path.display()),
                ))
            }
        })
        .collect();
    gameitems
}

/// for primitives we read fields m3cx, m3ci and m3ay's from separate files with bin extension
fn read_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem_file_name: String,
    mut item: GameItemEnum,
) -> io::Result<GameItemEnum> {
    if let GameItemEnum::Primitive(primitive) = &mut item {
        let gameitem_file_name = gameitem_file_name.trim_end_matches(".json");
        let obj_path = gameitems_dir.join(format!("{gameitem_file_name}.obj"));
        if obj_path.exists() {
            let (vertices_len, indices_len, compressed_vertices, compressed_indices) =
                read_obj(&obj_path)?;
            primitive.num_vertices = Some(vertices_len as u32);
            primitive.compressed_vertices_len = Some(compressed_vertices.len() as u32);
            primitive.compressed_vertices_data = Some(compressed_vertices);
            primitive.num_indices = Some(indices_len as u32);
            primitive.compressed_indices_len = Some(compressed_indices.len() as u32);
            primitive.compressed_indices_data = Some(compressed_indices);
        }
        let frame0_file_name = animation_frame_file_name(gameitem_file_name, 0);
        let frame0_path = gameitems_dir.join(frame0_file_name);
        if frame0_path.exists() {
            // we have animation frames
            let mut frame = 0;
            let mut frames = Vec::new();
            loop {
                let frame_path =
                    gameitems_dir.join(animation_frame_file_name(gameitem_file_name, frame));
                if frame_path.exists() {
                    let animation_frame = read_obj_as_frame(&frame_path)?;
                    frames.push(animation_frame);
                    frame += 1;
                } else {
                    break;
                }
            }

            // TODO we could combine both iterations to reduce memory usage

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

fn read_obj(obj_path: &Path) -> io::Result<(usize, usize, Vec<u8>, Vec<u8>)> {
    let ObjData {
        name: _,
        vertices,
        texture_coordinates,
        normals,
        indices,
    } = read_obj_file(obj_path).map_err(|e| {
        io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e))
    })?;

    // zip the vertices, texture coordinates and normals into a single buffer
    let mut vpx_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for ((v, vt), vn) in vertices
        .iter()
        .zip(texture_coordinates.iter())
        .zip(normals.iter())
    {
        let (normal, vpx_vertex_normal_data) = vn;
        let nx = normal.0 as f32;
        let ny = normal.1 as f32;
        // invert the z axis
        let nz = -(normal.2 as f32);

        let vertext = Vertex3dNoTex2 {
            x: v.0 as f32,
            y: v.1 as f32,
            // invert the z axis
            z: -(v.2 as f32),
            nx,
            ny,
            nz,
            tu: vt.0 as f32,
            tv: vt.1.unwrap_or(0.0) as f32,
        };
        write_vertex(&mut vpx_vertices, &vertext, vpx_vertex_normal_data);
    }
    let bytes_per_index: u8 = if vertices.len() > MAX_VERTICES_FOR_2_BYTE_INDEX {
        4
    } else {
        2
    };
    let mut vpx_indices = BytesMut::new();
    for chunk in indices.chunks(3) {
        // since the z axis is inverted we have to reverse the order of the vertices
        let v1 = chunk[0];
        let v2 = chunk[1];
        let v3 = chunk[2];
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v3);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v2);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v1);
    }
    let vertices_len = vertices.len();
    let incices_len = indices.len();

    let vertices = vpx_vertices.to_vec();
    let indices = vpx_indices.to_vec();

    let compressed_vertices = compress_data(&vertices)?;
    let compressed_indices = compress_data(&indices)?;
    Ok((
        vertices_len,
        incices_len,
        compressed_vertices,
        compressed_indices,
    ))
}

fn read_obj_as_frame(obj_path: &Path) -> io::Result<Vec<VertData>> {
    let ObjData {
        name: _,
        vertices: obj_vertices,
        texture_coordinates: _,
        normals,
        indices: _,
    } = read_obj_file(obj_path).map_err(|e| {
        io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e))
    })?;
    let mut vertices: Vec<VertData> = Vec::with_capacity(obj_vertices.len());
    for (v, vn) in obj_vertices.iter().zip(normals.iter()) {
        let (normal, _) = vn;
        let nx = normal.0 as f32;
        let ny = normal.1 as f32;
        // invert the z axis
        let nz = -(normal.2 as f32);
        let vertext = VertData {
            x: v.0 as f32,
            y: v.1 as f32,
            // invert the z axis
            z: -(v.2 as f32),
            nx,
            ny,
            nz,
        };
        vertices.push(vertext);
    }
    Ok(vertices)
}

fn write_info<P: AsRef<Path>>(vpx: &&VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let json_path = expanded_dir.as_ref().join("info.json");
    let mut json_file = File::create(json_path)?;
    let info = info_to_json(&vpx.info, &vpx.custominfotags);
    serde_json::to_writer_pretty(&mut json_file, &info)?;
    Ok(())
}

fn read_info<P: AsRef<Path>>(
    expanded_dir: &P,
    screenshot: Option<Vec<u8>>,
) -> io::Result<(TableInfo, CustomInfoTags)> {
    let info_path = expanded_dir.as_ref().join("info.json");
    if !info_path.exists() {
        return Ok((TableInfo::default(), CustomInfoTags::default()));
    }
    let value: Value = read_json(&info_path)?;
    let (info, custominfotags) = json_to_info(value, screenshot)?;
    Ok((info, custominfotags))
}

fn read_collections<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<Collection>> {
    let collections_path = expanded_dir.as_ref().join("collections.json");
    if !collections_path.exists() {
        println!("No collections.json found");
        return Ok(vec![]);
    }
    let value = read_json(collections_path)?;
    let collections: Vec<Collection> = json_to_collections(value)?;
    Ok(collections)
}

fn read_renderprobes<P: AsRef<Path>>(
    expanded_dir: &P,
) -> io::Result<Option<Vec<RenderProbeWithGarbage>>> {
    let renderprobes_path = expanded_dir.as_ref().join("renderprobes.json");
    if !renderprobes_path.exists() {
        return Ok(None);
    }
    let value: Vec<RenderProbeJson> = read_json(renderprobes_path)?;
    let renderprobes = value.iter().map(|v| v.to_renderprobe()).collect();
    Ok(Some(renderprobes))
}

fn write_renderprobes<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    if let Some(renderprobes) = &vpx.gamedata.render_probes {
        let renderprobes_path = expanded_dir.as_ref().join("renderprobes.json");
        let mut renderprobes_file = File::create(renderprobes_path)?;
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
    use testdir::testdir;
    use testresult::TestResult;

    // Encoded data for 2x2 argb with alpha always 0xFF because the vpinball
    // bmp export does not support alpha channel.
    // See lzw_writer tests on what colors these are.
    const LZW_COMPRESSED_DATA: [u8; 14] =
        [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4];

    #[test]
    pub fn test_write_read_bmp() -> TestResult {
        let test_dir = testdir!();
        let bmp_path = test_dir.join("test_image.bmp");

        write_image_bmp(&bmp_path, &LZW_COMPRESSED_DATA, 2, 2)?;

        let file_bytes = std::fs::read(&bmp_path)?;
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

    #[test]
    pub fn test_expand_write_read() -> TestResult {
        let expanded_path = testdir!();
        if expanded_path.exists() {
            std::fs::remove_dir_all(&expanded_path)?;
        }
        std::fs::create_dir(&expanded_path)?;

        // read 1x1.png as a Vec<u8>
        let mut screenshot = Vec::new();
        let mut screenshot_file = File::open("testdata/1x1.png")?;
        screenshot_file.read_to_end(&mut screenshot)?;

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

        write(&vpx, &expanded_path)?;

        // the user has updated one image from png to webp
        let image_path = expanded_path.join("images").join("test image replaced.png");
        let new_image_path = image_path.with_extension("webp");
        std::fs::rename(&image_path, &new_image_path)?;

        // adjust the image path in the vpx
        vpx.images[1].change_extension("webp");

        let read = read(&expanded_path)?;

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
    fn test_read_image_dimensions_png_as_hdr() {
        // this file is actually a png file but with hdr extension
        // see https://github.com/francisdb/vpin/issues/110
        let hdr_path = Path::new("testdata").join("wrongly_labeled_png.hdr");
        let dimensions = read_image_dimensions(&hdr_path).unwrap();

        assert_eq!(dimensions, Some((512, 256)));
    }

    #[test]
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
        let cursor = std::io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.zero", reader).unwrap();

        assert_eq!(dimensions, None);
    }

    #[test]
    fn test_read_image_dimensions_fail_invalid_png() {
        let cursor = std::io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.png", reader).unwrap();

        assert_eq!(dimensions, None);
    }
}
