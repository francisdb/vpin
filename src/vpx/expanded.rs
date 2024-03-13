use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::{fs::File, path::Path};

use cfb::CompoundFile;
use serde::de;
use serde_json::Value;

use super::{read_gamedata, Version, VPX};

use super::collection::Collection;
use super::font;
use super::gamedata::{GameData, GameDataJson};
use super::sound;
use super::sound::{read_sound, write_sound, SoundData, SoundDataJson};
use super::version;
use crate::vpx::biff::{BiffRead, BiffReader};
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::font::{FontData, FontDataJson};
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::image::{ImageData, ImageDataBits, ImageDataJson};
use crate::vpx::jsonmodel::{collections_json, info_to_json, json_to_collections, json_to_info};
use crate::vpx::material::{
    Material, MaterialJson, SaveMaterial, SaveMaterialJson, SavePhysicsMaterial,
    SavePhysicsMaterialJson,
};
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
            WriteError::Io(error) => write!(f, "IO error: {}", error),
            WriteError::Json(error) => write!(f, "JSON error: {}", error),
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
    let mut version_file = File::create(&version_path)?;
    let version_string = vpx.version.to_string();
    version_file.write_all(version_string.as_bytes())?;

    // write the screenshot as a png
    if let Some(screenshot) = &vpx.info.screenshot {
        let screenshot_path = expanded_dir.as_ref().join("screenshot.png");
        let mut screenshot_file = std::fs::File::create(&screenshot_path)?;
        screenshot_file.write_all(screenshot)?;
    }

    // write table metadata as json
    write_info(&vpx, expanded_dir)?;

    // collections
    let collections_json_path = expanded_dir.as_ref().join("collections.json");
    let mut collections_json_file = File::create(&collections_json_path)?;
    let json_collections = collections_json(&vpx.collections);
    serde_json::to_writer_pretty(&mut collections_json_file, &json_collections)?;
    write_gameitems(&vpx, expanded_dir)?;
    write_images(&vpx, expanded_dir)?;
    write_sounds(&vpx, expanded_dir)?;
    write_fonts(&vpx, expanded_dir)?;
    write_game_data(&vpx, expanded_dir)?;
    if vpx.gamedata.materials.is_some() {
        write_materials(&vpx, expanded_dir)?;
    } else {
        write_old_materials(&vpx, expanded_dir)?;
        write_old_materials_physics(&vpx, expanded_dir)?;
    }
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
    let mut game_data_file = File::create(&game_data_path)?;
    let json = GameDataJson::from_game_data(&vpx.gamedata);
    serde_json::to_writer_pretty(&mut game_data_file, &json)?;
    // write the code to script.vbs
    let script_path = expanded_dir.as_ref().join("script.vbs");
    let mut script_file = File::create(&script_path)?;
    let script_bytes: Vec<u8> = vpx.gamedata.code.clone().into();
    script_file.write_all(script_bytes.as_ref())?;
    Ok(())
}

fn read_game_data<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<GameData> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let game_data_json: GameDataJson = read_json(&game_data_path)?;
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
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to parse/read json {}: {}", path.display(), e),
        )
    })
}

fn write_images<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    // create an image index
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let mut images_index_file = std::fs::File::create(&images_index_path)?;
    // on macOS/windows the file system is case-insensitive
    let mut image_names_lower: HashSet<String> = HashSet::new();
    let mut image_names_dupe_counter = 0;
    let mut json_images = Vec::with_capacity(vpx.sounds.len());
    let images: Vec<(String, &ImageData)> = vpx
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
            json_images.push(json);
            (file_name, image)
        })
        .collect();
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
                file.write_all(&bits.data)
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

fn read_images<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<ImageData>> {
    // TODO do we actually need an index?
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let images_index_json: Vec<ImageDataJson> = read_json(&images_index_path)?;
    let images_dir = expanded_dir.as_ref().join("images");
    let images: io::Result<Vec<ImageData>> = images_index_json
        .into_iter()
        .map(|image_data_json| {
            let mut image = image_data_json.to_image_data();
            if image.is_link() {
                // linked images have no data
                return Ok(image);
            } else {
                let file_name = image_data_json.name_dedup.as_ref().unwrap_or(&image.name);
                let full_file_name = format!("{}.{}", file_name, image.ext());
                let file_path = images_dir.join(full_file_name);
                if file_path.exists() {
                    let mut image_file = File::open(&file_path)?;
                    let mut image_data = Vec::new();
                    image_file.read_to_end(&mut image_data)?;
                    if let Some(jpg) = &mut image.jpeg {
                        jpg.data = image_data;
                    } else if image.bits.is_some() {
                        // the json serializer makes sure we have a Some with empty data
                        let image_data = ImageDataBits { data: image_data };
                        image.bits = Some(image_data);
                    }
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

fn write_sounds<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let sounds_index_path = expanded_dir.as_ref().join("sounds.json");
    let mut sounds_index_file = std::fs::File::create(&sounds_index_path)?;
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
            file.write_all(&write_sound(&sound))
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
            let file_path = sounds_dir.join(&full_file_name);
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
    let mut fonts_index_file = std::fs::File::create(&fonts_json_path)?;
    let fonts_index: Vec<FontDataJson> = vpx
        .fonts
        .iter()
        .map(|font| FontDataJson::from_font_data(font))
        .collect();
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

fn read_fonts<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<font::FontData>> {
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
        let mut materials_file = File::create(&materials_path)?;
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
    let mut materials_file = File::create(&materials_path)?;
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
        let mut materials_file = File::create(&materials_path)?;
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

fn write_gameitems<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    std::fs::create_dir_all(&gameitems_dir)?;
    let mut used_names_lowercase: HashSet<String> = HashSet::new();
    let mut files: Vec<String> = Vec::new();
    let mut id_gen = 0;
    for gameitem in &vpx.gameitems {
        let mut name = gameitem.name().to_string();
        if name.is_empty() {
            name = "unnamed".to_string();
        }
        // escape any characters that are not allowed in file names, for any os
        name = name.replace(|c: char| !c.is_alphanumeric(), "_");
        let mut file_name = format!("{}.{}", gameitem.type_name(), name);

        let lower_name = file_name.to_lowercase();
        if used_names_lowercase.contains(&lower_name) {
            file_name = format!("{}_{}", file_name, id_gen);
            id_gen += 1;
        }
        used_names_lowercase.insert(lower_name);

        let file_name_json = format!("{}.json", &file_name);
        files.push(file_name_json.clone());
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
    let mut gameitems_index_file = File::create(&gameitems_index_path)?;
    serde_json::to_writer_pretty(&mut gameitems_index_file, &files)?;
    Ok(())
}

/// for primitives we write fields m3cx, m3ci and m3ay's to separate files with bin extension
fn write_gameitem_binaries(
    gameitems_dir: &PathBuf,
    gameitem: &GameItemEnum,
    file_name: String,
) -> Result<(), WriteError> {
    if let GameItemEnum::Primitive(primitive) = gameitem {
        if let Some(m3cx) = &primitive.compressed_vertices_data {
            let m3cx_path = gameitems_dir.join(format!("{}.m3cx.bin", file_name));
            let mut m3cx_file = std::fs::File::create(&m3cx_path)?;
            m3cx_file.write_all(m3cx)?;
        }
        if let Some(m3ci) = &primitive.compressed_indices_data {
            let m3ci_path = gameitems_dir.join(format!("{}.m3ci.bin", file_name));
            let mut m3ci_file = std::fs::File::create(&m3ci_path)?;
            m3ci_file.write_all(m3ci)?;
        }
        if let Some(m3ays) = &primitive.compressed_animation_vertices_data {
            let m3ays_path = gameitems_dir.join(format!("{}.m3ays.bin", file_name));
            let mut m3ays_file = File::create(&m3ays_path)?;
            // write all sequentially, we have the counts in the json
            m3ays
                .iter()
                .try_for_each(|m3ay| m3ays_file.write_all(m3ay))?;
        }
    }
    Ok(())
}

fn read_gameitems<P: AsRef<Path>>(expanded_dir: &P) -> io::Result<Vec<GameItemEnum>> {
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    if !gameitems_index_path.exists() {
        println!("No gameitems.json found");
        return Ok(vec![]);
    }
    let gameitems_index: Vec<String> = read_json(gameitems_index_path)?;
    // for each item in the index read the items
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    let gameitems: io::Result<Vec<GameItemEnum>> = gameitems_index
        .into_iter()
        .map(|gameitem_file_name| {
            let gameitem_path = gameitems_dir.join(&gameitem_file_name);
            if gameitem_path.exists() {
                let item: GameItemEnum = read_json(&gameitem_path)?;
                read_gameitem_binaries(&gameitems_dir, gameitem_file_name, item)
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
    gameitems_dir: &PathBuf,
    gameitem_file_name: String,
    mut item: GameItemEnum,
) -> io::Result<GameItemEnum> {
    if let GameItemEnum::Primitive(primitive) = &mut item {
        let gameitem_file_name = gameitem_file_name.trim_end_matches(".json");
        let m3cx_path = gameitems_dir.join(format!("{}.m3cx.bin", &gameitem_file_name));
        if m3cx_path.exists() {
            let mut m3cx_file = File::open(&m3cx_path)?;
            let mut m3cx = Vec::new();
            m3cx_file.read_to_end(&mut m3cx)?;
            primitive.compressed_vertices_data = Some(m3cx);
        }
        let m3ci_path = gameitems_dir.join(format!("{}.m3ci.bin", &gameitem_file_name));
        if m3ci_path.exists() {
            let mut m3ci_file = File::open(&m3ci_path)?;
            let mut m3ci = Vec::new();
            m3ci_file.read_to_end(&mut m3ci)?;
            primitive.compressed_indices_data = Some(m3ci);
        }
        if let Some(counts) = &primitive.compressed_animation_vertices {
            let m3ays = read_animation_vertices(&gameitems_dir, counts, &gameitem_file_name)?;
            primitive.compressed_animation_vertices_data = Some(m3ays);
        }
    }
    Ok(item)
}

fn read_animation_vertices(
    gameitems_dir: &PathBuf,
    lengths: &Vec<u32>,
    gameitem_file_name: &&str,
) -> io::Result<Vec<Vec<u8>>> {
    let m3ays_path = gameitems_dir.join(format!("{}.m3ays.bin", &gameitem_file_name));
    if m3ays_path.exists() {
        let mut m3ays_file = File::open(&m3ays_path)?;
        // for each primitive.compressed_animation_vertices
        // read the data
        let mut m3ays = Vec::with_capacity(lengths.len());
        lengths.iter().try_for_each(|count| {
            let mut m3ay = vec![0; *count as usize];
            let res = m3ays_file.read_exact(&mut m3ay);
            m3ays.push(m3ay);
            res
        })?;
        Ok(m3ays)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("M3ays file not found: {}", m3ays_path.display()),
        ))
    }
}

fn write_info<P: AsRef<Path>>(vpx: &&VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let json_path = expanded_dir.as_ref().join("info.json");
    let mut json_file = File::create(&json_path)?;
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
        let path = format!("GameStg/Image{}", index);
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
        let path = format!("GameStg/Sound{}", index);
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
        let path = format!("GameStg/Font{}", index);
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let font = font::read(&input);

        let ext = font.ext();
        let mut font_path = fonts_path.clone();
        font_path.push(format!("Font{}.{}.{}", index, font.name, ext));

        files.push(font_path.join("/").to_string_lossy().to_string());
    }
    if fonts_size == 0 {
        files.push(fonts_path.to_string_lossy().to_string());
    }

    let entries = retrieve_entries_from_compound_file(&mut comp);
    entries.iter().for_each(|path| {
        // write the steam directly to a file
        let file_path = root_dir_path.join(&path[1..]);
        // println!("Writing to {}", file_path.display());
        files.push(file_path.to_string_lossy().to_string());
    });

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
    // TODO -extract_gameitems

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

fn retrieve_entries_from_compound_file(comp: &CompoundFile<std::fs::File>) -> Vec<String> {
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
    use crate::vpx::image::ImageDataJpeg;
    use crate::vpx::sound::WaveForm;
    use crate::vpx::tableinfo::TableInfo;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use testresult::TestResult;

    #[test]
    pub fn test_expand_write_read() -> TestResult {
        //let expanded_path = testdir!();
        let expanded_path = PathBuf::from("testing_expanded");
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
        let mut primitive: gameitem::primitive::Primitive = Faker.fake();
        primitive.name = "test primitive".to_string();
        // the compressed_animation_vertices should match the data size
        match &primitive.compressed_animation_vertices_data {
            Some(data) => {
                let mut sizes = Vec::new();
                data.iter().for_each(|d| {
                    sizes.push(d.len() as u32);
                });
                primitive.compressed_animation_vertices = Some(sizes);
            }
            None => primitive.compressed_animation_vertices = None,
        }
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

        let mut gamedata: GameData = Default::default();
        // Since for the json format these are calculated from the file contents we need to set them
        // to a correct value here
        gamedata.gameitems_size = 20;
        gamedata.images_size = 2;
        gamedata.sounds_size = 2;
        gamedata.fonts_size = 2;
        gamedata.collections_size = 2;

        let vpx = VPX {
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
                ImageData {
                    name: "test image 2".to_string(),
                    internal_name: None,
                    path: "test2.png".to_string(),
                    width: 0,
                    height: 0,
                    link: None,
                    alpha_test_value: 0.0,
                    is_opaque: Some(true),
                    is_signed: Some(false),
                    jpeg: None,
                    bits: Some(ImageDataBits {
                        data: vec![0, 1, 2, 3],
                    }),
                },
            ],
            sounds: vec![
                SoundData {
                    name: "test sound".to_string(),
                    path: "test.wav".to_string(),
                    wave_form: WaveForm::new(),
                    data: vec![0, 1, 2, 3],
                    internal_name: "test internal name".to_string(),
                    fade: 0,
                    volume: 0,
                    balance: 0,
                    output_target: 0,
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
                    output_target: 4,
                },
            ],
            fonts: vec![
                font::FontData {
                    name: "test font".to_string(),
                    path: "test.ttf".to_string(),
                    data: vec![0, 1, 2, 3],
                },
                font::FontData {
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
        let read = read(&expanded_path)?;

        assert_eq!(&vpx, &read);
        Ok(())
    }
}
