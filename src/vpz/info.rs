//! The files that describe a pack, read without decoding its parts and
//! assets.

use super::read::read_head;
use super::{Document, Manifest};
use crate::vpx::tableinfo::TableInfo;
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Seek};
use std::path::Path;

/// What describes a pack: the manifest, the table information and the
/// script, read by [`read_info`].
#[derive(Debug, PartialEq)]
pub struct PackInfo {
    /// The pack manifest
    pub manifest: Manifest,
    /// The table information of `table.json` as vpinball loads it, see
    /// [`table_info`]; `None` for a pack without a table
    pub table_info: Option<TableInfo>,
    /// The game script
    pub script: Option<String>,
}

/// Reads what describes a pack in a directory or a `.vpz` zip archive:
/// only `manifest.json`, `table.json` and the script are read, which keeps
/// it fast for listing many packs.
pub fn read_info<P: AsRef<Path>>(path: P) -> io::Result<PackInfo> {
    let path = path.as_ref();
    if path.is_dir() {
        info(read_head(|name| match fs::read(path.join(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        })?)
    } else {
        read_zip_info(io::BufReader::new(fs::File::open(path)?))
    }
}

/// Reads what describes a pack in a `.vpz` zip archive, see [`read_info`].
pub fn read_zip_info<R: Read + Seek>(reader: R) -> io::Result<PackInfo> {
    let mut archive = zip::ZipArchive::new(reader).map_err(io::Error::other)?;
    let names: BTreeMap<String, usize> = (0..archive.len())
        .filter_map(|index| {
            let name = archive.name_for_index(index)?.replace('\\', "/");
            Some((name, index))
        })
        .collect();
    info(read_head(|name| {
        let Some(&index) = names.get(name) else {
            return Ok(None);
        };
        let mut entry = archive.by_index(index).map_err(io::Error::other)?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    })?)
}

fn info(
    (manifest, table, script): (Manifest, Option<Document>, Option<String>),
) -> io::Result<PackInfo> {
    Ok(PackInfo {
        manifest,
        table_info: table.as_ref().map(|table| table_info(table).0),
        script,
    })
}

/// The table information a `table.json` document holds, as vpinball loads
/// it, with the custom info tags as properties, and the tag names in their
/// order. The save date and revision are the stored ones.
pub fn table_info(table: &Document) -> (TableInfo, Vec<String>) {
    let table = &table.properties;
    let text = |key: &str| {
        table
            .get(key)
            .and_then(Json::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let mut info = TableInfo {
        table_name: text("table_name"),
        author_name: text("author"),
        table_version: text("table_version"),
        release_date: text("release_date"),
        author_email: text("author_email"),
        author_website: text("web_site"),
        table_blurb: text("blurb"),
        table_description: text("description"),
        table_rules: text("rules"),
        table_save_date: text("date_saved"),
        table_save_rev: table
            .get("save_rev")
            .and_then(Json::as_u64)
            .map(|rev| rev.to_string()),
        ..TableInfo::default()
    };
    let mut tags = Vec::new();
    if let Some(Json::Object(custom_tags)) = table.get("custom_tags") {
        for (tag, value) in custom_tags {
            tags.push(tag.clone());
            if let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
                info.properties.insert(tag.clone(), value.to_string());
            }
        }
    }
    (info, tags)
}
