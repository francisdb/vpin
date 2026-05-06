// Example showing how to export an entire VPX table as a Wavefront OBJ + MTL +
// images folder.
//
// Output layout, given `path/to/table.vpx`:
//
//   path/to/table_export/
//   ├── table.obj
//   ├── table.mtl
//   └── images/
//       └── <texture-name>.<ext>
//
// The resulting .obj can be opened in any 3D viewer (Blender, MeshLab, ...).

use std::path::PathBuf;
use vpin::filesystem::RealFileSystem;
use vpin::vpx;
use vpin::vpx::export::obj_export::export_obj;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example export_table_obj <path_to_vpx>");
        std::process::exit(1);
    }

    let vpx_path = PathBuf::from(&args[1]);
    if !vpx_path.exists() {
        eprintln!("Error: file not found: {}", vpx_path.display());
        std::process::exit(1);
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

    println!("Exporting to {}", obj_path.display());
    export_obj(&vpx, &obj_path, &RealFileSystem)?;

    let obj_size = std::fs::metadata(&obj_path)?.len();
    let mtl_path = obj_path.with_extension("mtl");
    let mtl_size = std::fs::metadata(&mtl_path)?.len();

    println!("Done.");
    println!("  obj:    {} ({} bytes)", obj_path.display(), obj_size);
    println!("  mtl:    {} ({} bytes)", mtl_path.display(), mtl_size);
    println!("  images: {}", out_dir.join("images").display());

    Ok(())
}
