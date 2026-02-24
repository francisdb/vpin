// Example showing how to export an entire VPX table as a GLB or glTF file
//
// The resulting GLB/glTF can be opened in any 3D viewer or editor like Blender.

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::export::gltf_export::{GltfExportOptions, GltfFormat, export_with_options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=warn (or info, debug) to see warnings
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --example export_table_glb <path_to_vpx> [format] [--invisible]"
        );
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  path_to_vpx   Path to the .vpx file to export");
        eprintln!("  format        Output format: 'glb' (default) or 'gltf'");
        eprintln!("  --invisible   Include invisible items (using KHR_node_visibility extension)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  cargo run --example export_table_glb table.vpx");
        eprintln!("  cargo run --example export_table_glb table.vpx glb");
        eprintln!("  cargo run --example export_table_glb table.vpx gltf");
        eprintln!("  cargo run --example export_table_glb table.vpx glb --invisible");
        std::process::exit(1);
    }

    let vpx_path = PathBuf::from(&args[1]);
    let mut format = GltfFormat::Glb;
    let mut export_invisible = false;

    for arg in args.iter().skip(2) {
        match arg.to_lowercase().as_str() {
            "glb" => format = GltfFormat::Glb,
            "gltf" => format = GltfFormat::Gltf,
            "--invisible" => export_invisible = true,
            other => {
                eprintln!(
                    "Error: Unknown argument '{}'. Use 'glb', 'gltf', or '--invisible'.",
                    other
                );
                std::process::exit(1);
            }
        }
    }

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
        "\nExporting to {} format: {}{}",
        format_name,
        output_path.display(),
        if export_invisible {
            " (including invisible items)"
        } else {
            ""
        }
    );

    let options = GltfExportOptions {
        format,
        export_invisible_items: export_invisible,
    };
    export_with_options(&vpx, &output_path, &RealFileSystem, &options)?;

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
