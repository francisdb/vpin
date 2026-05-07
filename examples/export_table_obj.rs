// Example showing how to export an entire VPX table as a Wavefront OBJ + MTL +
// (optionally) an images folder.
//
// Output layout, given `path/to/table.vpx`:
//
//   path/to/table_export/
//   |-- table.obj
//   |-- table.mtl
//   `-- images/         (only with --with-textures)
//       `-- <texture-name>.<ext>
//
// The resulting .obj can be opened in any 3D viewer (Blender, MeshLab, ...).
// Pass `--with-textures` if you want textures to show up in the viewer.

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::export::obj_export::{ExportUnits, ObjExportOptions, export_obj};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --example export_table_obj <path_to_vpx> [units] [--with-textures]"
        );
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  path_to_vpx       Path to the .vpx file to export");
        eprintln!(
            "  units             Output unit: 'vpu' (default, matches vpinball), 'mm', 'cm', or 'm'"
        );
        eprintln!(
            "  --with-textures   Also extract images and reference them from the MTL (for DCC tools)"
        );
        std::process::exit(1);
    }

    let vpx_path = PathBuf::from(&args[1]);
    if !vpx_path.exists() {
        eprintln!("Error: file not found: {}", vpx_path.display());
        std::process::exit(1);
    }

    let mut units = ExportUnits::Vpu;
    let mut with_textures = false;
    for arg in args.iter().skip(2) {
        match arg.to_lowercase().as_str() {
            "vpu" => units = ExportUnits::Vpu,
            "mm" => units = ExportUnits::Mm,
            "cm" => units = ExportUnits::Cm,
            "m" => units = ExportUnits::M,
            "--with-textures" => with_textures = true,
            other => {
                eprintln!("Error: unknown argument '{other}'. Use vpu/mm/cm/m or --with-textures.");
                std::process::exit(1);
            }
        }
    }

    let stem = vpx_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("vpx path has no usable file stem")?
        .to_string();

    let parent = vpx_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let out_dir = parent.join(format!("{stem}_export"));
    let obj_path = out_dir.join(format!("{stem}.obj"));

    println!("Reading VPX file: {}", vpx_path.display());
    let vpx = vpx::read(&vpx_path)?;

    println!(
        "Exporting to {} (units: {units:?}, textures: {})",
        obj_path.display(),
        if with_textures { "yes" } else { "no" },
    );
    let options = ObjExportOptions {
        units,
        extract_textures: with_textures,
        ..ObjExportOptions::default()
    };
    export_obj(&vpx, &obj_path, &RealFileSystem, &options)?;

    let obj_size = std::fs::metadata(&obj_path)?.len();
    let mtl_path = obj_path.with_extension("mtl");
    let mtl_size = std::fs::metadata(&mtl_path)?.len();

    println!("Done.");
    println!("  obj:    {} ({} bytes)", obj_path.display(), obj_size);
    println!("  mtl:    {} ({} bytes)", mtl_path.display(), mtl_size);
    if with_textures {
        println!("  images: {}", out_dir.join("images").display());
    }

    Ok(())
}
