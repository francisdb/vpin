//! Building a [`Vpz`] from the files of a pack

use super::names::{sanitize_file_name, sanitize_file_name_utf8};
use super::{
    Asset, Document, FILE_FORMAT, FILE_VERSION, Manifest, NamedDocument, Part, PartType, Vpz,
};
use log::warn;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

pub(super) const MANIFEST: &str = "manifest.json";
pub(super) const TABLE: &str = "table.json";
pub(super) const SCRIPT: &str = "script.vbs";
pub(super) const TYPE: &str = "$type";
pub(super) const NAME: &str = "name";
pub(super) const MESH: &str = "mesh";
pub(super) const VBS_SCRIPT: &str = "vbs_script";

pub(super) const PARTS_DIR: &str = "parts";
pub(super) const COLLECTIONS_DIR: &str = "collections";
pub(super) const MATERIALS_DIR: &str = "materials";
pub(super) const RENDER_PROBES_DIR: &str = "renderprobes";
pub(super) const IMAGES_DIR: &str = "images";
pub(super) const SOUNDS_DIR: &str = "sounds";
pub(super) const FONTS_DIR: &str = "fonts";
pub(super) const MESHES_DIR: &str = "meshes";

/// The `table.json` keys listing the entity names, with their folder
pub(super) const NAME_LISTS: [(&str, &str); 4] = [
    ("parts", PARTS_DIR),
    ("collections", COLLECTIONS_DIR),
    ("materials", MATERIALS_DIR),
    ("renderprobes", RENDER_PROBES_DIR),
];

pub(super) fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(super) fn from_files(mut files: BTreeMap<String, Vec<u8>>) -> io::Result<Vpz> {
    let manifest = files.remove(MANIFEST).ok_or_else(|| {
        invalid(format!(
            "not a VPZ pack: {MANIFEST} is missing ({} files)",
            files.len()
        ))
    })?;
    let manifest = read_manifest(&manifest)?;

    let table = files
        .remove(TABLE)
        .map(|bytes| parse_object(TABLE, &bytes))
        .transpose()?
        .map(|mut properties| {
            properties.shift_remove(TYPE);
            Document { properties }
        });

    let script = read_script(&mut files, table.as_ref())?;

    let name_list = |key: &str| -> io::Result<Vec<String>> {
        let Some(table) = &table else {
            return Ok(Vec::new());
        };
        match table.properties.get(key) {
            None => Ok(Vec::new()),
            Some(Value::Array(names)) => names
                .iter()
                .map(|name| {
                    name.as_str().map(str::to_string).ok_or_else(|| {
                        invalid(format!("{TABLE}: {key} holds a non-string name {name}"))
                    })
                })
                .collect(),
            Some(other) => Err(invalid(format!(
                "{TABLE}: {key} should be a list of names, got {other}"
            ))),
        }
    };
    let [
        parts_list,
        collections_list,
        materials_list,
        render_probes_list,
    ] = NAME_LISTS.map(|(key, _)| name_list(key));

    let materials = read_named(&mut files, MATERIALS_DIR, &materials_list?)?
        .into_iter()
        .map(NamedEntry::into_named_document)
        .collect();
    let render_probes = read_named(&mut files, RENDER_PROBES_DIR, &render_probes_list?)?
        .into_iter()
        .map(NamedEntry::into_named_document)
        .collect();
    let collections = read_named(&mut files, COLLECTIONS_DIR, &collections_list?)?
        .into_iter()
        .map(NamedEntry::into_named_document)
        .collect();
    let parts = read_named(&mut files, PARTS_DIR, &parts_list?)?
        .into_iter()
        .map(|entry| read_part(&mut files, entry))
        .collect::<io::Result<_>>()?;

    let images = read_assets(&mut files, IMAGES_DIR)?;
    let sounds = read_assets(&mut files, SOUNDS_DIR)?;
    let fonts = read_assets(&mut files, FONTS_DIR)?;

    Ok(Vpz {
        manifest,
        table,
        script,
        parts,
        collections,
        materials,
        render_probes,
        images,
        sounds,
        fonts,
        other_files: files,
    })
}

fn parse_object(path: &str, bytes: &[u8]) -> io::Result<Map<String, Value>> {
    match serde_json::from_slice(bytes) {
        Ok(Value::Object(properties)) => Ok(properties),
        Ok(other) => Err(invalid(format!(
            "{path} should hold a JSON object, got {}",
            json_kind(&other)
        ))),
        Err(e) => Err(invalid(format!("{path}: {e}"))),
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn from_properties<T: DeserializeOwned>(
    path: &str,
    properties: Map<String, Value>,
) -> io::Result<T> {
    serde_json::from_value(Value::Object(properties)).map_err(|e| invalid(format!("{path}: {e}")))
}

fn read_manifest(bytes: &[u8]) -> io::Result<Manifest> {
    let mut properties = parse_object(MANIFEST, bytes)?;
    properties.shift_remove(TYPE);
    let format = properties.get("file_format").and_then(Value::as_str);
    if format != Some(FILE_FORMAT) {
        return Err(invalid(format!(
            "not a VPZ pack: {MANIFEST} has file_format {}, expected {FILE_FORMAT:?}",
            properties
                .get("file_format")
                .map_or_else(|| "missing".to_string(), Value::to_string)
        )));
    }
    if let Some(version) = properties.get("file_version").and_then(Value::as_u64)
        && version > u64::from(FILE_VERSION)
    {
        return Err(invalid(format!(
            "VPZ file_version {version} is newer than the supported {FILE_VERSION}"
        )));
    }
    from_properties(MANIFEST, properties)
}

/// The script file `table.json` references, or `script.vbs` in a pack
/// without a reference. A reference that is not a file of the pack is the
/// script itself, as vpinball reads it.
fn read_script(
    files: &mut BTreeMap<String, Vec<u8>>,
    table: Option<&Document>,
) -> io::Result<Option<String>> {
    let reference = table.and_then(|table| table.properties.get(VBS_SCRIPT));
    let path = match reference {
        None => SCRIPT.to_string(),
        Some(Value::String(path)) => path.clone(),
        Some(other) => {
            return Err(invalid(format!(
                "{TABLE}: {VBS_SCRIPT} should be a string, got {}",
                json_kind(other)
            )));
        }
    };
    match files.remove(&path) {
        Some(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|e| invalid(format!("{path} is not UTF-8: {e}"))),
        None if reference.is_some() => Ok(Some(path)),
        None => Ok(None),
    }
}

/// A JSON document of a named entity folder
struct NamedEntry {
    path: String,
    name: String,
    properties: Map<String, Value>,
}

impl NamedEntry {
    fn into_named_document(mut self) -> NamedDocument {
        self.properties.shift_remove(TYPE);
        NamedDocument {
            name: self.name,
            properties: self.properties,
        }
    }
}

fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn file_stem(path: &str) -> &str {
    let (_, file_name) = split_path(path);
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name)
}

fn file_extension(path: &str) -> &str {
    let (_, file_name) = split_path(path);
    Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
}

fn take_folder(files: &mut BTreeMap<String, Vec<u8>>, folder: &str) -> Vec<(String, Vec<u8>)> {
    let prefix = format!("{folder}/");
    let paths: Vec<String> = files
        .range(prefix.clone()..)
        .take_while(|(path, _)| path.starts_with(&prefix))
        .map(|(path, _)| path.clone())
        .collect();
    paths
        .into_iter()
        .filter_map(|path| files.remove_entry(&path))
        .collect()
}

/// Reads the JSON documents of a folder, ordered by the name list of
/// `table.json` and followed by the documents it does not list, in file
/// name order. The name is the `name` property or else the file stem.
fn read_named(
    files: &mut BTreeMap<String, Vec<u8>>,
    folder: &str,
    list: &[String],
) -> io::Result<Vec<NamedEntry>> {
    let mut entries = Vec::new();
    for (path, bytes) in take_folder(files, folder) {
        if file_extension(&path) != "json" {
            files.insert(path, bytes);
            continue;
        }
        let mut properties = parse_object(&path, &bytes)?;
        let name = match properties.shift_remove(NAME) {
            None => None,
            Some(Value::String(name)) => Some(name),
            Some(other) => {
                return Err(invalid(format!(
                    "{path}: {NAME} should be a string, got {}",
                    json_kind(&other)
                )));
            }
        };
        entries.push(Some((path, name, properties)));
    }

    let mut ordered = Vec::with_capacity(entries.len());
    for list_name in list {
        // the file vpinball resolves the name to, a document carrying the
        // name (a renamed file), and last the resolved file whatever name
        // it carries, as vpinball does
        let expected = [
            sanitize_file_name(list_name),
            sanitize_file_name_utf8(list_name),
        ];
        let resolved = |path: &str| expected.iter().any(|stem| file_stem(path) == stem);
        let find = |matches: &dyn Fn(&str, Option<&String>) -> bool| {
            entries.iter().position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|(path, name, _)| matches(path, name.as_ref()))
            })
        };
        let position = find(&|path, name| resolved(path) && name.is_none_or(|n| n == list_name))
            .or_else(|| find(&|_, name| name == Some(list_name)))
            .or_else(|| find(&|path, _| resolved(path)));
        match position.and_then(|position| entries[position].take()) {
            Some((path, name, properties)) => ordered.push(NamedEntry {
                path,
                name: name.unwrap_or_else(|| list_name.clone()),
                properties,
            }),
            None => warn!("No file found in {folder}/ for the {TABLE} entry {list_name:?}"),
        }
    }
    ordered.extend(
        entries
            .into_iter()
            .flatten()
            .map(|(path, name, properties)| NamedEntry {
                name: name.unwrap_or_else(|| file_stem(&path).to_string()),
                path,
                properties,
            }),
    );
    Ok(ordered)
}

fn read_part(files: &mut BTreeMap<String, Vec<u8>>, entry: NamedEntry) -> io::Result<Part> {
    let NamedEntry {
        path,
        name,
        mut properties,
    } = entry;
    let part_type = match properties.shift_remove(TYPE) {
        Some(Value::String(part_type)) => PartType::from(part_type),
        Some(other) => {
            return Err(invalid(format!(
                "{path}: {TYPE} should be a string, got {}",
                json_kind(&other)
            )));
        }
        None => return Err(invalid(format!("{path}: the part has no {TYPE}"))),
    };
    let mesh = match properties.shift_remove(MESH) {
        Some(Value::String(mesh_path)) => Some(
            files
                .remove(&mesh_path)
                .ok_or_else(|| invalid(format!("{path}: the mesh file {mesh_path} is missing")))?,
        ),
        Some(other) => {
            return Err(invalid(format!(
                "{path}: {MESH} should be a path, got {}",
                json_kind(&other)
            )));
        }
        // a primitive without a reference uses the mesh named after it, if any
        None if part_type == PartType::Primitive => {
            [sanitize_file_name(&name), sanitize_file_name_utf8(&name)]
                .iter()
                .find_map(|stem| files.remove(&format!("{MESHES_DIR}/{stem}.glb")))
        }
        None => None,
    };
    Ok(Part {
        name,
        part_type,
        properties,
        mesh,
    })
}

/// Reads the assets of a folder: each sidecar with the data file sharing
/// its stem, then the data files without a sidecar. A sidecar without a
/// data file stays in [`Vpz::other_files`].
fn read_assets<S: DeserializeOwned + Default>(
    files: &mut BTreeMap<String, Vec<u8>>,
    folder: &str,
) -> io::Result<Vec<Asset<S>>> {
    let mut entries: Vec<Option<(String, Vec<u8>)>> =
        take_folder(files, folder).into_iter().map(Some).collect();
    let mut assets = Vec::new();
    for sidecar_index in 0..entries.len() {
        let Some((sidecar_path, _)) = &entries[sidecar_index] else {
            continue;
        };
        if file_extension(sidecar_path) != "json" {
            continue;
        }
        let (parent, _) = split_path(sidecar_path);
        let stem = file_stem(sidecar_path);
        let data_index = entries.iter().position(|entry| {
            entry.as_ref().is_some_and(|(path, _)| {
                file_extension(path) != "json"
                    && split_path(path).0 == parent
                    && file_stem(path) == stem
            })
        });
        let Some(data_index) = data_index else {
            warn!("No data file found for the sidecar {sidecar_path}");
            continue;
        };
        let (sidecar_path, sidecar_bytes) = entries[sidecar_index].take().unwrap_or_default();
        let (data_path, data) = entries[data_index].take().unwrap_or_default();
        let mut properties = parse_object(&sidecar_path, &sidecar_bytes)?;
        properties.shift_remove(TYPE);
        let name = match properties.shift_remove(NAME) {
            None => file_stem(&sidecar_path).to_string(),
            Some(Value::String(name)) => name,
            Some(other) => {
                return Err(invalid(format!(
                    "{sidecar_path}: {NAME} should be a string, got {}",
                    json_kind(&other)
                )));
            }
        };
        assets.push(Asset {
            name,
            extension: file_extension(&data_path).to_string(),
            data,
            sidecar: from_properties(&sidecar_path, properties)?,
        });
    }
    for (path, data) in entries.into_iter().flatten() {
        if file_extension(&path) == "json" {
            files.insert(path, data);
            continue;
        }
        assets.push(Asset {
            name: file_stem(&path).to_string(),
            extension: file_extension(&path).to_string(),
            data,
            sidecar: S::default(),
        });
    }
    Ok(assets)
}
