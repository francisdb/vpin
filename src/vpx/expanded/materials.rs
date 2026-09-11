//! Material reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::material::{
    self, Material, MaterialJson, SaveMaterial, SaveMaterialJson, SavePhysicsMaterial,
    SavePhysicsMaterialJson,
};
use log::warn;
use std::io;
use std::path::Path;

use super::{Output, WriteError};

pub(super) fn write_materials(materials: &[Material], out: &Output) -> Result<(), WriteError> {
    out.write_json(Path::new("materials.json"), || {
        materials
            .iter()
            .map(MaterialJson::from_material)
            .collect::<Vec<_>>()
    })
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

pub(super) fn write_legacy_materials(
    materials_old: &[SaveMaterial],
    materials_physics_old: Option<&Vec<SavePhysicsMaterial>>,
    out: &Output,
) -> Result<(), WriteError> {
    write_old_materials(materials_old, out)?;
    write_old_materials_physics(materials_physics_old, out)
}

fn write_old_materials(materials_old: &[SaveMaterial], out: &Output) -> Result<(), WriteError> {
    out.write_json(Path::new("materials-old.json"), || {
        materials_old
            .iter()
            .map(SaveMaterialJson::from_save_material)
            .collect::<Vec<_>>()
    })
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

fn write_old_materials_physics(
    materials_physics_old: Option<&Vec<SavePhysicsMaterial>>,
    out: &Output,
) -> Result<(), WriteError> {
    if let Some(materials) = materials_physics_old {
        out.write_json(Path::new("materials-physics-old.json"), || {
            materials
                .iter()
                .map(SavePhysicsMaterialJson::from_save_physics_material)
                .collect::<Vec<_>>()
        })?;
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

/// Logs the material names of a 10.8 table that the records older
/// versions read cannot hold. The player uses the 10.8 record, which
/// holds any name, so the table plays; only a pre-10.8 vpinball would
/// see the name cut and its physics lost
pub(super) fn warn_material_names<'a>(names: impl Iterator<Item = &'a str>) {
    for name in names {
        if let Err(problem) = material::check_name(name) {
            warn!("Material name {name:?} {problem}, versions before 10.8 will not find it");
        }
    }
}
