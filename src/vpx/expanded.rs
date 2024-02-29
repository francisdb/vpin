use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::io::{self, Read, Write};
use std::{fs::File, path::Path};

use cfb::CompoundFile;
use serde_json::Value;

use super::{extract_script, read_custominfotags, read_gamedata, tableinfo, Version, VPX};

use super::collection::{self, Collection};
use super::font;
use super::gamedata::{GameData, GameDataJson};
use super::gameitem;
use super::sound;
use super::sound::{read_sound, write_sound, SoundData, SoundDataJson};
use super::version;
use crate::vpx::biff::{BiffRead, BiffReader};
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::font::{FontData, FontDataJson};
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::image::{ImageData, ImageDataJson};
use crate::vpx::jsonmodel::{collections_json, info_to_json, json_to_collections, json_to_info};
use crate::vpx::tableinfo::TableInfo;

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl Display for ReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(error) => write!(f, "IO error: {}", error),
            ReadError::Json(error) => write!(f, "JSON error: {}", error),
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        ReadError::Io(error)
    }
}

impl From<serde_json::Error> for ReadError {
    fn from(error: serde_json::Error) -> Self {
        ReadError::Json(error)
    }
}

pub enum WriteError {
    Io(io::Error),
    Json(serde_json::Error),
}

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

pub fn extract(vpx_file_path: &Path, expanded_path: &Path) -> io::Result<()> {
    let vbs_path = expanded_path.join("script.vbs");

    let mut root_dir = std::fs::DirBuilder::new();
    root_dir.recursive(true);
    root_dir.create(expanded_path).unwrap();

    let mut comp = cfb::open(vpx_file_path).unwrap();
    let version = version::read_version(&mut comp).unwrap();
    let gamedata = read_gamedata(&mut comp, &version).unwrap();

    extract_info(&mut comp, expanded_path)?;

    extract_script(&gamedata, &vbs_path)?;
    println!("VBScript file written to\n  {}", &vbs_path.display());
    extract_binaries(&mut comp, expanded_path);
    extract_images(&mut comp, &gamedata, expanded_path);
    extract_sounds(&mut comp, &gamedata, expanded_path, &version);
    extract_fonts(&mut comp, &gamedata, expanded_path);
    extract_gameitems(&mut comp, &gamedata, expanded_path);
    extract_collections(&mut comp, &gamedata, expanded_path);

    // let mut file_version = String::new();
    // comp.open_stream("/GameStg/Version")
    //     .unwrap()
    //     .read_to_string(&mut file_version)
    //     .unwrap();
    // println!("File version: {}", file_version);

    // let mut stream = comp.open_stream(inner_path).unwrap();
    // io::copy(&mut stream, &mut io::stdout()).unwrap();
    Ok(())
}

pub fn write<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    // write the version as utf8 to version.txt
    let version_path = expanded_dir.as_ref().join("version.txt");
    let mut version_file = std::fs::File::create(&version_path)?;
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
    let mut collections_json_file = std::fs::File::create(&collections_json_path)?;
    let json_collections = collections_json(&vpx.collections);
    serde_json::to_writer_pretty(&mut collections_json_file, &json_collections)?;
    write_gameitems(&vpx, expanded_dir)?;
    write_images(&vpx, expanded_dir)?;
    write_sounds(&vpx, expanded_dir)?;
    write_fonts(&vpx, expanded_dir)?;
    write_game_data(&vpx, expanded_dir)?;
    Ok(())
}

pub fn read<P: AsRef<Path>>(expanded_dir: &P) -> Result<VPX, ReadError> {
    // read the version
    let version_path = expanded_dir.as_ref().join("version.txt");
    let mut version_file = std::fs::File::open(&version_path)?;
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
        let mut screenshot_file = std::fs::File::open(&screenshot)?;
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
    let gamedata = read_game_data(expanded_dir)?;

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
    let mut game_data_file = std::fs::File::create(&game_data_path)?;
    let json = GameDataJson::from_game_data(&vpx.gamedata);
    serde_json::to_writer_pretty(&mut game_data_file, &json)?;
    Ok(())
}

fn read_game_data<P: AsRef<Path>>(expanded_dir: &P) -> Result<GameData, ReadError> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let mut game_data_file = std::fs::File::open(&game_data_path)?;
    let game_data_json: GameDataJson = serde_json::from_reader(&mut game_data_file)?;
    Ok(game_data_json.to_game_data())
}

fn write_images<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    // create an image index
    let images_index_path = expanded_dir.as_ref().join("images.json");
    let mut images_index_file = std::fs::File::create(&images_index_path)?;
    let images_index: Vec<ImageDataJson> = vpx
        .images
        .iter()
        .map(|image| ImageDataJson::from_image_data(image))
        .collect();
    serde_json::to_writer_pretty(&mut images_index_file, &images_index)?;

    let images_dir = expanded_dir.as_ref().join("images");
    std::fs::create_dir_all(&images_dir)?;
    for image in &vpx.images {
        if let Some(jpeg) = &image.jpeg {
            let file_name = format!("{}.{}", image.name, image.ext());
            let jpeg_path = images_dir.join(file_name);
            let mut jpeg_file = std::fs::File::create(&jpeg_path)?;
            jpeg_file.write_all(&jpeg.data)?;
        }
        // TODO write other image types
    }
    Ok(())
}

fn read_images<P: AsRef<Path>>(expanded_dir: &P) -> Result<Vec<ImageData>, ReadError> {
    // TODO do we actually need an index?
    let images_index_path = expanded_dir.as_ref().join("images.json");
    if !images_index_path.exists() {
        return Ok(vec![]);
    }
    let mut images_index_file = std::fs::File::open(&images_index_path)?;
    let images_index_json: Vec<ImageDataJson> = serde_json::from_reader(&mut images_index_file)?;
    let images_index: Vec<ImageData> = images_index_json
        .iter()
        .map(|image_data_json| image_data_json.to_image_data())
        .collect();
    // for each item in the index read the items
    let images_dir = expanded_dir.as_ref().join("images");
    let images: Result<Vec<ImageData>, ReadError> = images_index
        .into_iter()
        .map(|mut image| {
            // read the file
            let file_name = format!("{}.{}", image.name, image.ext());
            let image_path = images_dir.join(file_name);
            if image_path.exists() {
                let mut image_file = std::fs::File::open(&image_path)?;
                let mut image_data = Vec::new();
                image_file.read_to_end(&mut image_data)?;
                if let Some(jpg) = &mut image.jpeg {
                    jpg.data = image_data;
                }
                // TODO else fail?
                Ok(image)
            } else {
                Err(ReadError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Image file not found: {}", image_path.display()),
                )))
            }
        })
        .collect();
    images
}

fn write_sounds<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let sounds_index_path = expanded_dir.as_ref().join("sounds.json");
    let mut sounds_index_file = std::fs::File::create(&sounds_index_path)?;
    let images_index: Vec<SoundDataJson> = vpx
        .sounds
        .iter()
        .map(|sound| SoundDataJson::from_sound_data(sound))
        .collect();
    serde_json::to_writer_pretty(&mut sounds_index_file, &images_index)?;

    let sounds_dir = expanded_dir.as_ref().join("sounds");
    std::fs::create_dir_all(&sounds_dir)?;
    vpx.sounds.iter().try_for_each(|sound| {
        let sound_file_name = format!("{}.{}", sound.name, sound.ext());
        let sound_path = sounds_dir.join(sound_file_name);
        let mut file = File::create(sound_path)?;
        file.write_all(&write_sound(&sound))
    })?;
    Ok(())
}

fn read_sounds<P: AsRef<Path>>(expanded_dir: &P) -> Result<Vec<SoundData>, ReadError> {
    let sounds_json_path = expanded_dir.as_ref().join("sounds.json");
    if !sounds_json_path.exists() {
        return Ok(vec![]);
    }
    let mut sounds_index_file = std::fs::File::open(&sounds_json_path)?;
    let sounds_json: Vec<SoundDataJson> = serde_json::from_reader(&mut sounds_index_file)?;
    let sounds: Vec<SoundData> = sounds_json
        .iter()
        .map(|sound_data_json| sound_data_json.to_sound_data())
        .collect();
    // for each item in the index read the items
    let sounds_dir = expanded_dir.as_ref().join("sounds");
    let sounds: Result<Vec<SoundData>, ReadError> = sounds
        .into_iter()
        .map(|mut sound| {
            let sound_file_name = format!("{}.{}", sound.name, sound.ext());
            let sound_path = sounds_dir.join(sound_file_name);
            let mut sound_file = File::open(&sound_path)?;
            let mut sound_data = Vec::new();
            sound_file.read_to_end(&mut sound_data)?;
            read_sound(&sound_data, &mut sound);
            Ok(sound)
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
    for font in &vpx.fonts {
        // TODO write the font data
    }
    Ok(())
}

fn read_fonts<P: AsRef<Path>>(expanded_dir: &P) -> Result<Vec<font::FontData>, ReadError> {
    let fonts_index_path = expanded_dir.as_ref().join("fonts.json");
    let mut fonts_index_file = std::fs::File::open(&fonts_index_path)?;
    let fonts_json: Vec<FontDataJson> = serde_json::from_reader(&mut fonts_index_file)?;
    let fonts_index: Vec<FontData> = fonts_json
        .iter()
        .map(|font_data_json| font_data_json.to_font_data())
        .collect();
    // for each item in the index read the items
    let fonts_dir = expanded_dir.as_ref().join("fonts");
    let fonts: Result<Vec<font::FontData>, ReadError> = fonts_index
        .into_iter()
        .map(|font| {
            // TODO load data
            Ok(font)
        })
        .collect();
    fonts
}

fn write_gameitems<P: AsRef<Path>>(vpx: &VPX, expanded_dir: &P) -> Result<(), WriteError> {
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    std::fs::create_dir_all(&gameitems_dir)?;
    let mut used_names: HashSet<String> = HashSet::new();
    let mut files: Vec<String> = Vec::new();
    let mut id_gen = 0;
    for gameitem in &vpx.gameitems {
        let mut name = gameitem.name().to_string();
        if name.is_empty() {
            name = "unnamed".to_string();
        }
        if used_names.contains(&name) {
            name = format!("{}_{}", name, id_gen);
            id_gen += 1;
        }
        used_names.insert(name.clone());
        let file_name = format!("{}.{}.json", gameitem.type_name(), name);
        files.push(file_name.clone());
        let gameitem_path = gameitems_dir.join(file_name);
        let gameitem_file = std::fs::File::create(&gameitem_path)?;
        serde_json::to_writer_pretty(&gameitem_file, &gameitem)?;
    }
    // write the gameitems index as array with names being the type and the name
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    let mut gameitems_index_file = std::fs::File::create(&gameitems_index_path)?;
    serde_json::to_writer_pretty(&mut gameitems_index_file, &files)?;

    Ok(())
}

fn read_gameitems<P: AsRef<Path>>(expanded_dir: &P) -> Result<Vec<GameItemEnum>, ReadError> {
    let gameitems_index_path = expanded_dir.as_ref().join("gameitems.json");
    let mut gameitems_index_file = std::fs::File::open(&gameitems_index_path)?;
    let gameitems_index: Vec<String> = serde_json::from_reader(&mut gameitems_index_file)?;
    // for each item in the index read the items
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    let gameitems: Result<Vec<GameItemEnum>, ReadError> = gameitems_index
        .into_iter()
        .map(|gameitem_file_name| {
            let gameitem_path = gameitems_dir.join(gameitem_file_name);
            if gameitem_path.exists() {
                let mut gameitem_file = std::fs::File::open(&gameitem_path)?;
                let mut gameitem_string = String::new();
                gameitem_file.read_to_string(&mut gameitem_string)?;
                let res: GameItemEnum = serde_json::from_str(&gameitem_string)?;
                Ok(res)
            } else {
                Err(ReadError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("GameItem file not found: {}", gameitem_path.display()),
                )))
            }
        })
        .collect();
    gameitems
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
) -> Result<(TableInfo, CustomInfoTags), ReadError> {
    let info_path = expanded_dir.as_ref().join("info.json");
    let mut info_file = std::fs::File::open(&info_path)?;
    let value: Value = serde_json::from_reader(&mut info_file)?;
    let (info, custominfotags) = json_to_info(value, screenshot)?;
    Ok((info, custominfotags))
}

fn read_collections<P: AsRef<Path>>(expanded_dir: &P) -> Result<Vec<Collection>, ReadError> {
    let collections_path = expanded_dir.as_ref().join("collections.json");
    let mut collections_file = std::fs::File::open(&collections_path)?;
    let value = serde_json::from_reader(&mut collections_file)?;
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

fn extract_info(comp: &mut CompoundFile<File>, root_dir_path: &Path) -> std::io::Result<()> {
    let json_path = root_dir_path.join("TableInfo.json");
    let mut json_file = std::fs::File::create(&json_path).unwrap();
    let table_info = tableinfo::read_tableinfo(comp)?;
    let custom_info_tags = read_custominfotags(comp)?;
    // TODO can we avoid the clone?
    let screenshot = table_info
        .screenshot
        .as_ref()
        .unwrap_or(&Vec::new())
        .clone();
    if !screenshot.is_empty() {
        let screenshot_path = root_dir_path.join("screenshot.bin");
        let mut screenshot_file = std::fs::File::create(screenshot_path).unwrap();
        screenshot_file.write_all(&screenshot).unwrap();
    }

    let info = info_to_json(&table_info, &custom_info_tags);

    serde_json::to_writer_pretty(&mut json_file, &info).unwrap();
    println!("Info file written to\n  {}", &json_path.display());
    Ok(())
}

fn extract_images(comp: &mut CompoundFile<File>, gamedata: &GameData, root_dir_path: &Path) {
    let images_size = gamedata.images_size;

    let images_path = root_dir_path.join("images");
    std::fs::create_dir_all(&images_path).unwrap();

    println!(
        "Writing {} images to\n  {}",
        images_size,
        images_path.display()
    );

    for index in 0..images_size {
        let path = format!("GameStg/Image{}", index);
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let mut reader = BiffReader::new(&input);
        let img = ImageData::biff_read(&mut reader);
        match &img.jpeg {
            Some(jpeg) => {
                let ext = img.ext();
                let mut jpeg_path = images_path.clone();
                jpeg_path.push(format!("Image{}.{}.{}", index, img.name, ext));
                //dbg!(&jpeg_path);
                let mut file = std::fs::File::create(jpeg_path).unwrap();
                file.write_all(&jpeg.data).unwrap();
            }
            None => {
                println!("Image {} has no jpeg data", index)
                // nothing to do here
            }
        }
    }
}

fn extract_collections(comp: &mut CompoundFile<File>, gamedata: &GameData, root_dir_path: &Path) {
    let collections_size = gamedata.collections_size;

    let collections_json_path = root_dir_path.join("collections.json");
    println!(
        "Writing {} collections to\n  {}",
        collections_size,
        collections_json_path.display()
    );

    let collections: Vec<Collection> = (0..collections_size)
        .map(|index| {
            let path = format!("GameStg/Collection{}", index);
            let mut input = Vec::new();
            comp.open_stream(&path)
                .unwrap()
                .read_to_end(&mut input)
                .unwrap();
            collection::read(&input)
        })
        .collect();

    let json_collections = collections_json(&collections);
    let mut json_file = std::fs::File::create(collections_json_path).unwrap();
    serde_json::to_writer_pretty(&mut json_file, &json_collections).unwrap();
}

fn extract_sounds(
    comp: &mut CompoundFile<File>,
    gamedata: &GameData,
    root_dir_path: &Path,
    file_version: &Version,
) {
    let sounds_size = gamedata.sounds_size;
    let sounds_path = root_dir_path.join("sounds");
    std::fs::create_dir_all(&sounds_path).unwrap();

    println!(
        "Writing {} sounds to\n  {}",
        sounds_size,
        sounds_path.display()
    );

    for index in 0..sounds_size {
        let path = format!("GameStg/Sound{}", index);
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        let mut reader = BiffReader::new(&input);
        let sound = sound::read(file_version, &mut reader);

        let ext = sound.ext();
        let mut sound_path = sounds_path.clone();
        sound_path.push(format!("Sound{}.{}.{}", index, sound.name, ext));
        //dbg!(&jpeg_path);
        let mut file = std::fs::File::create(sound_path).unwrap();
        file.write_all(&write_sound(&sound)).unwrap();
    }
}

fn extract_fonts(comp: &mut CompoundFile<File>, gamedata: &GameData, root_dir_path: &Path) {
    let fonts_size = gamedata.fonts_size;

    let fonts_path = root_dir_path.join("fonts");
    std::fs::create_dir_all(&fonts_path).unwrap();

    println!(
        "Writing {} fonts to\n  {}",
        fonts_size,
        fonts_path.display()
    );

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
        //dbg!(&jpeg_path);
        let mut file = std::fs::File::create(font_path).unwrap();
        file.write_all(&font.data).unwrap();
    }
}

fn extract_gameitems(comp: &mut CompoundFile<File>, gamedata: &GameData, root_dir_path: &Path) {
    let gameitems_size = gamedata.gameitems_size;

    let gameitems_path = root_dir_path.join("gameitems");
    std::fs::create_dir_all(&gameitems_path).unwrap();

    println!(
        "Writing {} gameitems to\n  {}",
        gameitems_size,
        gameitems_path.display()
    );

    for index in 0..gameitems_size {
        let path = format!("GameStg/GameItem{}", index);
        let mut input = Vec::new();
        comp.open_stream(&path)
            .unwrap()
            .read_to_end(&mut input)
            .unwrap();
        //println!("GameItem {} size: {}", path, input.len());
        let _gameitem = gameitem::read(&input);

        //dbg!(gameitem);

        // let ext = gameitem.ext();
        // let mut gameitem_path = gameitems_path.clone();
        // gameitem_path.push(format!("GameItem{}.{}.{}", index, gameitem.name, ext));
        // //dbg!(&jpeg_path);
        // let mut file = std::fs::File::create(gameitem_path).unwrap();
        // file.write_all(&gameitem.data).unwrap();
    }
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

fn extract_binaries(comp: &mut CompoundFile<std::fs::File>, root_dir_path: &Path) {
    // write all remaining entries
    let entries = retrieve_entries_from_compound_file(comp);

    entries.iter().for_each(|path| {
        let mut stream = comp.open_stream(path).unwrap();
        // write the steam directly to a file
        let file_path = root_dir_path.join(&path[1..]);
        // println!("Writing to {}", file_path.display());
        // make sure the parent directory exists
        let parent = file_path.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();
        let mut file = std::fs::File::create(file_path).unwrap();
        io::copy(&mut stream, &mut file).unwrap();
    });

    println!("Binaries written to\n  {}", root_dir_path.display());
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::image::ImageDataJpeg;
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
        let mut screenshot_file = std::fs::File::open("testdata/1x1.png")?;
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
            gamedata: Default::default(),
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
            images: vec![ImageData {
                name: "test image".to_string(),
                inme: None,
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
                    inme: None,
                    data: vec![0, 1, 2, 3],
                }),
                bits: None,
            }],
            sounds: vec![],
            fonts: vec![],
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
