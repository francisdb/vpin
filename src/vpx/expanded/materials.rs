//! Material reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::material::{
    Material, MaterialJson, SaveMaterial, SaveMaterialJson, SavePhysicsMaterial,
    SavePhysicsMaterialJson,
};
use std::io;
use std::path::Path;

use super::WriteError;

pub(super) fn write_materials<P: AsRef<Path>>(
    materials: Option<&Vec<Material>>,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(materials) = materials {
        let materials_path = expanded_dir.as_ref().join("materials.json");
        let mut materials_file = fs.create_file(&materials_path)?;
        let materials_index: Vec<MaterialJson> =
            materials.iter().map(MaterialJson::from_material).collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    }
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
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<MaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<Material> = materials_index
        .into_iter()
        .map(|m| MaterialJson::to_material(&m))
        .collect();
    Ok(Some(materials))
}

pub(super) fn write_old_materials<P: AsRef<Path>>(
    materials_old: &[SaveMaterial],
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    let mut materials_file = fs.create_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = materials_old
        .iter()
        .map(SaveMaterialJson::from_save_material)
        .collect();
    serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
    Ok(())
}

pub(super) fn read_old_materials<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<SaveMaterial>> {
    let materials_path = expanded_dir.as_ref().join("materials-old.json");
    if !fs.exists(&materials_path) {
        return Ok(vec![]);
    }
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<SaveMaterialJson> = serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SaveMaterial> = materials_index
        .into_iter()
        .map(|m| SaveMaterialJson::to_save_material(&m))
        .collect();
    Ok(materials)
}

pub(super) fn write_old_materials_physics<P: AsRef<Path>>(
    materials_physics_old: Option<&Vec<SavePhysicsMaterial>>,
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let Some(materials) = materials_physics_old {
        let materials_path = expanded_dir.as_ref().join("materials-physics-old.json");
        let mut materials_file = fs.create_file(&materials_path)?;
        let materials_index: Vec<SavePhysicsMaterialJson> = materials
            .iter()
            .map(SavePhysicsMaterialJson::from_save_physics_material)
            .collect();
        serde_json::to_writer_pretty(&mut materials_file, &materials_index)?;
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
    let mut materials_file = fs.open_file(&materials_path)?;
    let materials_index: Vec<SavePhysicsMaterialJson> =
        serde_json::from_reader(&mut materials_file)?;
    let materials: Vec<SavePhysicsMaterial> = materials_index
        .into_iter()
        .map(|m| SavePhysicsMaterialJson::to_save_physics_material(&m))
        .collect();
    Ok(Some(materials))
}
