// Example showing how to export an entire VPX table as a single GLB file
//
// The resulting GLB can be opened in any 3D viewer or editor like Blender.

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::expanded::export_glb;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read a VPX file
    let vpx_path = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("Usage: cargo run --example export_table_glb <path_to_vpx>");
            std::process::exit(1);
        }
    };

    if !vpx_path.exists() {
        eprintln!("Error: File not found: {}", vpx_path.display());
        std::process::exit(1);
    }

    println!("Reading VPX file: {}", vpx_path.display());
    let vpx = vpx::read(&vpx_path)?;

    println!(
        "Table: {}",
        vpx.info
            .table_name
            .as_ref()
            .unwrap_or(&"unknown".to_string())
    );

    // Count the different game item types
    let mut primitive_count = 0;
    let mut wall_count = 0;
    let mut ramp_count = 0;
    let mut rubber_count = 0;
    let mut flasher_count = 0;

    for item in &vpx.gameitems {
        match item {
            vpin::vpx::gameitem::GameItemEnum::Primitive(_) => primitive_count += 1,
            vpin::vpx::gameitem::GameItemEnum::Wall(_) => wall_count += 1,
            vpin::vpx::gameitem::GameItemEnum::Ramp(_) => ramp_count += 1,
            vpin::vpx::gameitem::GameItemEnum::Rubber(_) => rubber_count += 1,
            vpin::vpx::gameitem::GameItemEnum::Flasher(_) => flasher_count += 1,
            _ => {}
        }
    }

    println!("\nGame items that will be exported:");
    println!("  Primitives: {}", primitive_count);
    println!("  Walls: {}", wall_count);
    println!("  Ramps: {}", ramp_count);
    println!("  Rubbers: {}", rubber_count);
    println!("  Flashers: {}", flasher_count);

    // Print lighting info
    println!("\nLighting settings:");
    println!(
        "  Light emission scale: {} (VPinball HDR multiplier)",
        vpx.gamedata.light_emission_scale
    );
    println!(
        "  Global emission scale: {} (overall brightness)",
        vpx.gamedata.global_emission_scale
    );
    println!(
        "  Env emission scale: {} (environment map brightness)",
        vpx.gamedata.env_emission_scale
    );
    let combined = vpx.gamedata.light_emission_scale * vpx.gamedata.global_emission_scale;
    let light_intensity = combined * 0.001; // Scale factor to candelas
    println!(
        "  -> Combined: {} -> ~{:.0} candelas in glTF",
        combined, light_intensity
    );

    // Export to GLB
    let glb_path = vpx_path.with_extension("glb");
    println!("\nExporting to: {}", glb_path.display());

    export_glb(&vpx, &glb_path, &RealFileSystem)?;

    // Get file size
    let metadata = std::fs::metadata(&glb_path)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    println!("✓ Export complete!");
    println!("  File size: {:.2} MB", size_mb);
    println!(
        "\nYou can now open \"{}\" in Blender or any other 3D viewer.",
        glb_path.display()
    );

    Ok(())
}
