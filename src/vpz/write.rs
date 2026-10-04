//! The files of a [`Vpz`]

use super::json;
use super::names::{StemPool, sanitize_file_name};
use super::read::{
    COLLECTIONS_DIR, FONTS_DIR, IMAGES_DIR, MANIFEST, MATERIALS_DIR, MESH, MESHES_DIR, NAME,
    NAME_LISTS, PARTS_DIR, RENDER_PROBES_DIR, SCRIPT, SOUNDS_DIR, TABLE, TYPE, VBS_SCRIPT, invalid,
};
use super::{Asset, NamedDocument, Vpz};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::io;

#[derive(Default)]
struct Output {
    files: BTreeMap<String, Vec<u8>>,
}

impl Output {
    fn add(&mut self, path: String, data: Vec<u8>) -> io::Result<()> {
        match self.files.entry(path) {
            Entry::Vacant(entry) => {
                entry.insert(data);
                Ok(())
            }
            Entry::Occupied(entry) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("the pack holds two files at {}", entry.key()),
            )),
        }
    }

    fn add_json(&mut self, path: String, document: Map<String, Value>) -> io::Result<()> {
        let data =
            json::to_vec(&Value::Object(document)).map_err(|e| invalid(format!("{path}: {e}")))?;
        self.add(path, data)
    }
}

fn to_properties<T: Serialize>(what: &str, value: &T) -> io::Result<Map<String, Value>> {
    match serde_json::to_value(value) {
        Ok(Value::Object(properties)) => Ok(properties),
        Ok(_) => Err(invalid(format!("{what} does not serialize to an object"))),
        Err(e) => Err(invalid(format!("{what}: {e}"))),
    }
}

/// The `name` a document needs, as vpinball reads it: the file stem
/// gives the name, or for an entity of a `table.json` name list, the list
/// entry whose sanitized form is the stem. A name neither recovers, from a
/// sanitized or collision suffixed file name, is written in the document.
fn written_name<'a>(name: &'a str, stem: &str, listed: bool) -> Option<&'a str> {
    let recovered = if listed {
        sanitize_file_name(name) == stem
    } else {
        name == stem
    };
    (!recovered).then_some(name)
}

/// A document with its `$type` first, then the `name` if any, then the
/// properties
fn document(
    type_name: &str,
    name: Option<&str>,
    properties: &Map<String, Value>,
) -> Map<String, Value> {
    let mut document = Map::with_capacity(properties.len() + 2);
    document.insert(TYPE.to_string(), Value::from(type_name));
    if let Some(name) = name {
        document.insert(NAME.to_string(), Value::from(name));
    }
    for (key, value) in properties {
        document.insert(key.clone(), value.clone());
    }
    document
}

fn names<'a>(names: impl Iterator<Item = &'a String>) -> Value {
    Value::Array(names.cloned().map(Value::String).collect())
}

pub(super) fn to_files(vpz: &Vpz) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut out = Output::default();

    let mut manifest = Map::new();
    manifest.insert(TYPE.to_string(), Value::from("manifest"));
    manifest.extend(to_properties(MANIFEST, &vpz.manifest)?);
    out.add_json(MANIFEST.to_string(), manifest)?;

    if let Some(table) = &vpz.table {
        let mut properties = table.properties.clone();
        let lists = [
            names(vpz.parts.iter().map(|part| &part.name)),
            names(vpz.collections.iter().map(|collection| &collection.name)),
            names(vpz.materials.iter().map(|material| &material.name)),
            names(vpz.render_probes.iter().map(|probe| &probe.name)),
        ];
        // insert keeps the position of a key the table already has
        for ((key, _), list) in NAME_LISTS.iter().zip(lists) {
            properties.insert(key.to_string(), list);
        }
        if vpz.script.is_some() {
            properties.insert(VBS_SCRIPT.to_string(), Value::from(SCRIPT));
        } else {
            properties.shift_remove(VBS_SCRIPT);
        }
        out.add_json(TABLE.to_string(), document("table", None, &properties))?;
    }
    if let Some(script) = &vpz.script {
        out.add(SCRIPT.to_string(), script.as_bytes().to_vec())?;
    }

    // the name lists of table.json resolve sanitized file names
    let listed = vpz.table.is_some();
    write_named(&mut out, MATERIALS_DIR, "material", &vpz.materials, listed)?;
    write_named(
        &mut out,
        RENDER_PROBES_DIR,
        "renderprobe",
        &vpz.render_probes,
        listed,
    )?;
    write_named(
        &mut out,
        COLLECTIONS_DIR,
        "collection",
        &vpz.collections,
        listed,
    )?;

    let mut part_stems = StemPool::default();
    let mut mesh_stems = StemPool::default();
    for part in &vpz.parts {
        let stem = part_stems.unique(&part.name);
        let mut properties = part.properties.clone();
        if let Some(mesh) = &part.mesh {
            let mesh_path = format!("{MESHES_DIR}/{}.glb", mesh_stems.unique(&part.name));
            properties.insert(MESH.to_string(), Value::from(mesh_path.clone()));
            out.add(mesh_path, mesh.clone())?;
        } else {
            properties.shift_remove(MESH);
        }
        out.add_json(
            format!("{PARTS_DIR}/{stem}.json"),
            document(
                part.part_type.as_str(),
                written_name(&part.name, &stem, listed),
                &properties,
            ),
        )?;
    }

    write_assets(&mut out, IMAGES_DIR, "image", &vpz.images)?;
    write_assets(&mut out, SOUNDS_DIR, "sound", &vpz.sounds)?;
    write_assets(&mut out, FONTS_DIR, "font", &vpz.fonts)?;

    for (path, data) in &vpz.other_files {
        out.add(path.clone(), data.clone())?;
    }
    Ok(out.files)
}

fn write_named(
    out: &mut Output,
    folder: &str,
    type_name: &str,
    entities: &[NamedDocument],
    listed: bool,
) -> io::Result<()> {
    let mut stems = StemPool::default();
    for entity in entities {
        let stem = stems.unique(&entity.name);
        out.add_json(
            format!("{folder}/{stem}.json"),
            document(
                type_name,
                written_name(&entity.name, &stem, listed),
                &entity.properties,
            ),
        )?;
    }
    Ok(())
}

fn write_assets<S: Serialize>(
    out: &mut Output,
    folder: &str,
    type_name: &str,
    assets: &[Asset<S>],
) -> io::Result<()> {
    let mut stems = StemPool::default();
    for asset in assets {
        let stem = stems.unique(&asset.name);
        let data_path = if asset.extension.is_empty() {
            format!("{folder}/{stem}")
        } else {
            format!("{folder}/{stem}.{}", asset.extension)
        };
        let sidecar_path = format!("{folder}/{stem}.json");
        let properties = to_properties(&sidecar_path, &asset.sidecar)?;
        out.add(data_path, asset.data.clone())?;
        out.add_json(
            sidecar_path,
            document(
                type_name,
                written_name(&asset.name, &stem, false),
                &properties,
            ),
        )?;
    }
    Ok(())
}
