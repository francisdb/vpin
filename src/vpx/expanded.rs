use std::io::{self, Read, Write};
use std::{fs::File, path::Path};

use cfb::CompoundFile;

use super::{extract_script, read_gamedata, tableinfo, Version};

use super::collection::{self, Collection};
use super::font;
use super::gamedata::GameData;
use super::gameitem;
use super::sound;
use super::sound::write_sound;
use super::version;
use crate::vpx::biff::{BiffRead, BiffReader};
use crate::vpx::image::ImageData;
use crate::vpx::jsonmodel::{collections_json, table_json};

pub fn extract(vpx_file_path: &Path, expanded_path: &Path) -> std::io::Result<()> {
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

    let info = table_json(&table_info);

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
mod tests {
    // use super::*;
    // use std::path::PathBuf;
    // use testdir::testdir;
}
