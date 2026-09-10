//! Material reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::gamedata::GameData;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::material::{
    self, Material, MaterialJson, SaveMaterial, SaveMaterialJson, SavePhysicsMaterial,
    SavePhysicsMaterialJson,
};
use log::warn;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

use super::WriteError;

pub(super) fn write_materials<P: AsRef<Path>>(
    materials: &[Material],
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let materials_path = expanded_dir.as_ref().join("materials.json");
    let mut materials_file = fs.create_buffered_file(&materials_path)?;
    let materials_index: Vec<MaterialJson> =
        materials.iter().map(MaterialJson::from_material).collect();
    serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    materials_file.flush()?;
    Ok(())
}

pub(super) fn read_materials<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<Material>>> {
    let materials_path = expanded_dir.as_ref().join("materials.json");
    if !fs.exists(&materials_path) {
        return Ok(None);
    }
    let mut materials_file = fs.open_buffered_file(&materials_path)?;
    let materials_index: Vec<MaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<Material> = materials_index
        .into_iter()
        .map(|m| MaterialJson::to_material(&m))
        .collect();
    Ok(Some(materials))
}

pub(super) fn write_legacy_materials<P: AsRef<Path>>(
    materials_old: &[SaveMaterial],
    materials_physics_old: Option<&Vec<SavePhysicsMaterial>>,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    write_old_materials(materials_old, expanded_dir, fs)?;
    write_old_materials_physics(materials_physics_old, expanded_dir, fs)
}

fn write_old_materials<P: AsRef<Path>>(
    materials_old: &[SaveMaterial],
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    let mut materials_file = fs.create_buffered_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = materials_old
        .iter()
        .map(SaveMaterialJson::from_save_material)
        .collect();
    serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    materials_file.flush()?;
    Ok(())
}

pub(super) fn read_old_materials<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<SaveMaterial>>> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    if !fs.exists(&materials_path) {
        return Ok(None);
    }
    let mut materials_file = fs.open_buffered_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SaveMaterial> = materials_index
        .into_iter()
        .map(|m| SaveMaterialJson::to_save_material(&m))
        .collect();
    Ok(Some(materials))
}

fn write_old_materials_physics<P: AsRef<Path>>(
    materials_physics_old: Option<&Vec<SavePhysicsMaterial>>,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(materials) = materials_physics_old {
        let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
        let mut materials_file = fs.create_buffered_file(&materials_path)?;
        let materials_index: Vec<SavePhysicsMaterialJson> = materials
            .iter()
            .map(SavePhysicsMaterialJson::from_save_physics_material)
            .collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
        materials_file.flush()?;
    }
    Ok(())
}

pub(super) fn read_old_materials_physics<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Option<Vec<SavePhysicsMaterial>>> {
    let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
    if !fs.exists(&materials_path) {
        return Ok(None);
    }
    let mut materials_file = fs.open_buffered_file(&materials_path)?;
    let materials_index: Vec<SavePhysicsMaterialJson> =
        serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SavePhysicsMaterial> = materials_index
        .into_iter()
        .map(|m| SavePhysicsMaterialJson::to_save_physics_material(&m))
        .collect();
    Ok(Some(materials))
}

/// Fails when a material has a name vpinball cannot store: the editor
/// never creates one, so it can only come from an edited json, and the
/// fixed material record would silently cut it while the physics record
/// next to it is matched by name
pub(super) fn check_material_names<'a>(names: impl Iterator<Item = &'a str>) -> io::Result<()> {
    for name in names {
        if let Err(problem) = material::check_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Material name {name:?} {problem}"),
            ));
        }
    }
    Ok(())
}

/// Caps every material reference of the table to what vpinball stores,
/// like the editor does when a name is entered, and logs each distinct
/// name that changed. A reference longer than any material name can hold
/// could never match one, so this only turns a dangling reference into
/// the one the author meant.
pub(super) fn cap_material_references(gamedata: &mut GameData, gameitems: &mut [GameItemEnum]) {
    let mut capped: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut cap = |reference: &mut String| {
        if let Cow::Owned(stored) = material::stored_name(reference) {
            let entry = capped
                .entry(reference.clone())
                .or_insert_with(|| (stored.clone(), 0));
            entry.1 += 1;
            *reference = stored;
        }
    };
    cap(&mut gamedata.playfield_material);
    for item in gameitems.iter_mut() {
        for reference in item.material_references_mut() {
            cap(reference);
        }
    }
    for (original, (stored, count)) in capped {
        warn!(
            "Material reference {original:?} capped to {stored:?} in {count} place(s), vpinball stores at most {} Latin-1 characters",
            material::MAX_NAME_LENGTH
        );
    }
}
