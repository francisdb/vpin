//! Game item reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::gameitem::GameItemEnum;
use log::info;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use tracing::instrument;

use super::primitives::{read_gameitem_binaries, write_gameitem_binaries};
use super::util::read_json;
use super::{ExpandOptions, WriteError};

/// Since it's common to change layer visibility we don't want that to cause a
/// difference in the item json, therefore we write this info in the index.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GameItemInfoJson {
    pub(super) file_name: String,
    // most require these, only lightsequencer does not
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) is_locked: Option<bool>,
    // most require these, only lightsequencer does not
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) editor_layer: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) editor_layer_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) editor_layer_visibility: Option<bool>,
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

pub(super) fn write_gameitems<P: AsRef<Path>>(
    gameitems: &[GameItemEnum],
    expanded_dir: &P,
    options: &ExpandOptions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let gameitems_dir = expanded_dir.as_ref().join("gameitems");
    fs.create_dir_all(&gameitems_dir)?;
    let mut file_name_gen = FileNameGen::default();
    let mut files: Vec<GameItemInfoJson> = Vec::with_capacity(gameitems.len());
    let mut files_to_write: Vec<(String, usize)> = Vec::with_capacity(gameitems.len());

    for (idx, gameitem) in gameitems.iter().enumerate() {
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

    let gameitems_ref = gameitems;
    let gameitems_dir_clone = gameitems_dir.clone();

    let write_item = |(file_name, idx): &(String, usize)| -> Result<(), WriteError> {
        let file_name_json = format!("{}.json", file_name);
        let path = gameitems_dir_clone.join(&file_name_json);
        let gameitem = &gameitems_ref[*idx];

        let json_bytes = serde_json::to_vec_pretty(gameitem).map_err(WriteError::Json)?;
        fs.write_file(&path, &json_bytes)?;

        write_gameitem_binaries(&gameitems_dir_clone, gameitem, file_name, options, fs)?;

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

pub(super) fn read_gameitems<P: AsRef<Path>>(
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
    read_gameitem_binaries(gameitems_dir, file_name, item, fs)
}
