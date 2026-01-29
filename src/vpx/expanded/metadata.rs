//! Metadata reading and writing for expanded VPX format (info, collections, gamedata, renderprobes)

use crate::filesystem::FileSystem;
use crate::vpx::collection::Collection;
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::gamedata::{GameData, GameDataJson};
use crate::vpx::jsonmodel::{collections_json, info_to_json, json_to_collections, json_to_info};
use crate::vpx::renderprobe::{RenderProbeJson, RenderProbeWithGarbage};
use crate::vpx::tableinfo::TableInfo;
use log::info;
use serde_json::Value;
use std::io::{self, Write};
use std::path::Path;

use super::WriteError;
use super::util::read_json;

pub(super) fn write_game_data<P: AsRef<Path>>(
    gamedata: &GameData,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let game_data_path = expanded_dir.as_ref().join("gamedata.json");
    let mut game_data_file = fs.create_file(&game_data_path)?;
    let json = GameDataJson::from_game_data(gamedata);
    serde_json::to_writer_pretty(&mut game_data_file, &json)?;
    let script_path = expanded_dir.as_ref().join("script.vbs");
    let mut script_file = fs.create_file(&script_path)?;
    let script_bytes: Vec<u8> = gamedata.code.clone().into();
    script_file.write_all(script_bytes.as_ref())?;
    Ok(())
}

pub(super) fn read_game_data<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<GameData> {
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

pub(super) fn write_info<P: AsRef<Path>>(
    info: &TableInfo,
    custominfotags: &CustomInfoTags,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let json_path = expanded_dir.as_ref().join("info.json");
    let mut json_file = fs.create_file(&json_path)?;
    let info = info_to_json(info, custominfotags);
    serde_json::to_writer_pretty(&mut json_file, &info)?;
    Ok(())
}

pub(super) fn read_info<P: AsRef<Path>>(
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

pub(super) fn write_collections<P: AsRef<Path>>(
    collections: &[Collection],
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let collections_json_path = expanded_dir.as_ref().join("collections.json");
    let mut collections_json_file = fs.create_file(&collections_json_path)?;
    let json_collections = collections_json(collections);
    serde_json::to_writer_pretty(&mut collections_json_file, &json_collections)?;
    Ok(())
}

pub(super) fn read_collections<P: AsRef<Path>>(
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

pub(super) fn write_renderprobes<P: AsRef<Path>>(
    render_probes: Option<&Vec<RenderProbeWithGarbage>>,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(renderprobes) = render_probes {
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

pub(super) fn read_renderprobes<P: AsRef<Path>>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::model::StringWithEncoding;
    use crate::vpx::renderprobe::{RenderProbe, RenderProbeType};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_and_read_info() {
        use crate::filesystem::MemoryFileSystem;
        use std::path::PathBuf;

        let fs = MemoryFileSystem::new();
        let expanded_dir = PathBuf::from("test_info");

        let info = TableInfo {
            table_name: Some("Test Table".to_string()),
            author_name: Some("Test Author".to_string()),
            table_description: Some("Test Description".to_string()),
            ..Default::default()
        };

        let custom_info_tags = vec!["Some custom info tag".to_string()];

        write_info(&info, &custom_info_tags, &expanded_dir, &fs).unwrap();

        let (read_info, read_custom_info_tags) = read_info(&expanded_dir, None, &fs).unwrap();

        assert_eq!(info, read_info);
        assert_eq!(custom_info_tags, read_custom_info_tags);
    }

    #[test]
    fn test_write_and_read_game_data() {
        use crate::filesystem::MemoryFileSystem;
        use std::path::PathBuf;

        let fs = MemoryFileSystem::new();
        let expanded_dir = PathBuf::from("test_gamedata");

        let gamedata = GameData {
            name: "Test gamedata".to_string(),
            code: StringWithEncoding::new("print('Hello, VPX!')"),
            ..Default::default()
        };

        write_game_data(&gamedata, &expanded_dir, &fs).unwrap();

        let read_gamedata = read_game_data(&expanded_dir, &fs).unwrap();

        assert_eq!(gamedata, read_gamedata);
    }

    #[test]
    fn test_write_and_read_collections() {
        use crate::filesystem::MemoryFileSystem;
        use std::path::PathBuf;

        let fs = MemoryFileSystem::new();
        let expanded_dir = PathBuf::from("test_collections");

        let collections = vec![
            Collection {
                name: "Collection1".to_string(),
                items: vec!["ItemA".to_string(), "ItemB".to_string()],
                fire_events: false,
                stop_single_events: false,
                group_elements: false,
            },
            Collection {
                name: "Collection2".to_string(),
                items: vec!["ItemC".to_string()],
                fire_events: true,
                stop_single_events: true,
                group_elements: true,
            },
        ];

        write_collections(&collections, &expanded_dir, &fs).unwrap();

        let read_collections = read_collections(&expanded_dir, &fs).unwrap();

        assert_eq!(collections, read_collections);
    }

    #[test]
    fn test_write_and_read_renderprobes() {
        use crate::filesystem::MemoryFileSystem;
        use std::path::PathBuf;

        let fs = MemoryFileSystem::new();
        let expanded_dir = PathBuf::from("test_renderprobes");

        let mut render_probe = RenderProbe::default();
        render_probe.name = "Test Render Probe".to_string();
        render_probe.type_ = RenderProbeType::PlaneReflection;

        let render_probes = vec![
            RenderProbeWithGarbage {
                render_probe,
                trailing_data: vec![],
            },
            RenderProbeWithGarbage {
                render_probe: Default::default(),
                trailing_data: vec![1, 2, 3, 4, 5],
            },
        ];

        write_renderprobes(Some(&render_probes), &expanded_dir, &fs).unwrap();

        let read_renderprobes = read_renderprobes(&expanded_dir, &fs).unwrap().unwrap();

        assert_eq!(render_probes.len(), read_renderprobes.len());
        for (original, read) in render_probes.iter().zip(read_renderprobes.iter()) {
            assert_eq!(original, read);
        }
    }
}
