// Example showing how to extract a VPX file with GLB format for primitive meshes
//
// GLB format provides significantly better performance for large meshes compared to OBJ format.

use std::path::PathBuf;
use vpin::vpx::expanded::ExpandOptions;
use vpin::vpx::{self, expanded::PrimitiveMeshFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read a VPX file
    let vpx_path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
    let vpx = vpx::read(&vpx_path)?;

    println!("Extracting VPX file: {}", vpx_path.display());
    println!(
        "Table: {}",
        vpx.info
            .table_name
            .as_ref()
            .unwrap_or(&"unknown".to_string())
    );

    // Extract with default OBJ format
    let obj_dir = PathBuf::from("extracted_obj");
    if obj_dir.exists() {
        std::fs::remove_dir_all(&obj_dir)?;
    }
    std::fs::create_dir_all(&obj_dir)?;
    let expand_options = ExpandOptions::new()
        .mesh_format(PrimitiveMeshFormat::Obj)
        .generate_derived_meshes(true);
    vpx::expanded::write(&vpx, &obj_dir, &expand_options)?;
    println!("✓ Extracted with OBJ format to: {}", obj_dir.display());

    // Extract with GLB format for better performance on large meshes
    let glb_dir = PathBuf::from("extracted_glb");
    if glb_dir.exists() {
        std::fs::remove_dir_all(&glb_dir)?;
    }
    std::fs::create_dir_all(&glb_dir)?;
    let expand_options = ExpandOptions::new()
        .mesh_format(PrimitiveMeshFormat::Glb)
        .generate_derived_meshes(true);
    vpx::expanded::write(&vpx, &glb_dir, &expand_options)?;
    println!("✓ Extracted with GLB format to: {}", glb_dir.display());

    // Extract with GLTF format (JSON + BIN)
    let gltf_dir = PathBuf::from("extracted_gltf");
    if gltf_dir.exists() {
        std::fs::remove_dir_all(&gltf_dir)?;
    }
    std::fs::create_dir_all(&gltf_dir)?;
    let expand_options = ExpandOptions::new()
        .mesh_format(PrimitiveMeshFormat::Gltf)
        .generate_derived_meshes(true);
    vpx::expanded::write(&vpx, &gltf_dir, &expand_options)?;
    println!("✓ Extracted with GLTF format to: {}", gltf_dir.display());

    // Read back from either format - both OBJ and GLB are supported
    let vpx_from_obj = vpx::expanded::read(&obj_dir)?;
    println!("✓ Read back from OBJ format");

    let _vpx_from_glb = vpx::expanded::read(&glb_dir)?;
    println!("✓ Read back from GLB format");

    let _vpx_from_gltf = vpx::expanded::read(&gltf_dir)?;
    println!("✓ Read back from GLTF format");

    println!("  Game items: {}", vpx_from_obj.gameitems.len());
    println!("  Images: {}", vpx_from_obj.images.len());

    Ok(())
}
