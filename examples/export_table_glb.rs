// Example showing how to export an entire VPX table as a GLB or glTF file
//
// The resulting GLB/glTF can be opened in any 3D viewer or editor like Blender.

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::export::gltf_export::{GltfFormat, export};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=warn (or info, debug) to see warnings
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse arguments
    let args: Vec<String> = std::env::args().collect();

    let (vpx_path, format) = match args.len() {
        2 => (PathBuf::from(&args[1]), GltfFormat::Glb),
        3 => {
            let fmt = match args[2].to_lowercase().as_str() {
                "glb" => GltfFormat::Glb,
                "gltf" => GltfFormat::Gltf,
                _ => {
                    eprintln!("Error: Unknown format '{}'. Use 'glb' or 'gltf'.", args[2]);
                    std::process::exit(1);
                }
            };
            (PathBuf::from(&args[1]), fmt)
        }
        _ => {
            eprintln!("Usage: cargo run --example export_table_glb <path_to_vpx> [format]");
            eprintln!();
            eprintln!("Arguments:");
            eprintln!("  path_to_vpx  Path to the .vpx file to export");
            eprintln!("  format       Output format: 'glb' (default) or 'gltf'");
            eprintln!();
            eprintln!("Examples:");
            eprintln!("  cargo run --example export_table_glb table.vpx");
            eprintln!("  cargo run --example export_table_glb table.vpx glb");
            eprintln!("  cargo run --example export_table_glb table.vpx gltf");
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

    // Determine output path based on format
    let output_path = match format {
        GltfFormat::Glb => vpx_path.with_extension("glb"),
        GltfFormat::Gltf => vpx_path.with_extension("gltf"),
    };

    let format_name = match format {
        GltfFormat::Glb => "GLB",
        GltfFormat::Gltf => "glTF",
    };

    println!(
        "\nExporting to {} format: {}",
        format_name,
        output_path.display()
    );

    export(&vpx, &output_path, &RealFileSystem, format)?;

    // Get file size(s)
    let metadata = std::fs::metadata(&output_path)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    println!("✓ Export complete!");
    println!("  {} file size: {:.2} MB", format_name, size_mb);

    // For glTF, also show the .bin file size
    if format == GltfFormat::Gltf {
        let bin_path = output_path.with_extension("bin");
        if let Ok(bin_metadata) = std::fs::metadata(&bin_path) {
            let bin_size_mb = bin_metadata.len() as f64 / (1024.0 * 1024.0);
            println!("  Binary file size: {:.2} MB", bin_size_mb);
            println!("\nGenerated files:");
            println!("  - {}", output_path.display());
            println!("  - {}", bin_path.display());
        }
    }

    println!(
        "\nYou can now open \"{}\" in Blender or any other 3D viewer.",
        output_path.display()
    );

    Ok(())
}
