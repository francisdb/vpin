// Example showing how to export an entire VPX table as a Wavefront OBJ + MTL.
//
// By default the result is tuned for use in DCC tools like Blender or
// MeshLab: positions in metres, textures extracted to an `images/` folder,
// duplicate `newmtl` blocks collapsed, top+side walls split into separate
// face groups so each gets its own material/texture.
//
//   path/to/table_export/
//   |-- table.obj
//   |-- table.mtl
//   `-- images/
//       `-- <texture-name>.<ext>
//
// Pass `--vpinball-strict` to instead emit output that matches vpinball's
// own `File -> Export -> OBJ Mesh` quirks (no textures, raw VPU positions,
// duplicate `newmtl` blocks). Useful for diffing against a reference OBJ
// produced by vpinball itself.

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::export::obj_export::{ExportUnits, ObjExportOptions, export_obj};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --example export_table_obj <path_to_vpx> [units] [--vpinball-strict]"
        );
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  path_to_vpx         Path to the .vpx file to export");
        eprintln!("  units               Output unit: 'm' (default), 'mm', 'cm', or 'vpu'");
        eprintln!("  --vpinball-strict   Match vpinball's own OBJ exporter (no textures, raw VPU,");
        eprintln!(
            "                      duplicate `newmtl` blocks). Overrides any units argument."
        );
        std::process::exit(1);
    }

    let vpx_path = PathBuf::from(&args[1]);
    if !vpx_path.exists() {
        eprintln!("Error: file not found: {}", vpx_path.display());
        std::process::exit(1);
    }

    let mut units: Option<ExportUnits> = None;
    let mut strict = false;
    for arg in args.iter().skip(2) {
        match arg.to_lowercase().as_str() {
            "vpu" => units = Some(ExportUnits::Vpu),
            "mm" => units = Some(ExportUnits::Mm),
            "cm" => units = Some(ExportUnits::Cm),
            "m" => units = Some(ExportUnits::M),
            "--vpinball-strict" => strict = true,
            other => {
                eprintln!(
                    "Error: unknown argument '{other}'. Use vpu/mm/cm/m or --vpinball-strict."
                );
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

    let mut options = if strict {
        ObjExportOptions::vpinball_strict()
    } else {
        ObjExportOptions::default()
    };
    if let Some(u) = units {
        options.units = u;
    }

    println!(
        "Exporting to {} (units: {:?}, mode: {})",
        obj_path.display(),
        options.units,
        if strict { "vpinball-strict" } else { "default" },
    );
    export_obj(&vpx, &obj_path, &RealFileSystem, &options)?;

    let obj_size = std::fs::metadata(&obj_path)?.len();
    let mtl_path = obj_path.with_extension("mtl");
    let mtl_size = std::fs::metadata(&mtl_path)?.len();

    println!("Done.");
    println!("  obj:    {} ({} bytes)", obj_path.display(), obj_size);
    println!("  mtl:    {} ({} bytes)", mtl_path.display(), mtl_size);
    if options.extract_textures {
        println!("  images: {}", out_dir.join("images").display());
    }

    Ok(())
}
