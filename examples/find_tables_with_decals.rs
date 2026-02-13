// Example showing how to search a folder for VPX tables that use decals
//
// This is useful for finding test tables when working on decal support.
//
// Usage: cargo run --example find_tables_with_decals -- <path_to_folder>

use std::env;
use std::path::PathBuf;
use vpin::vpx;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::decal::DecalType;
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=warn (or info, debug) to see warnings
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Get folder path from command line argument
    let args: Vec<String> = env::args().collect();
    let folder_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        eprintln!("Usage: {} <path_to_folder>", args[0]);
        eprintln!("Example: cargo run --example find_tables_with_decals -- /path/to/tables");
        std::process::exit(1);
    };

    if !folder_path.exists() {
        eprintln!("Error: Folder not found: {}", folder_path.display());
        std::process::exit(1);
    }

    println!(
        "Searching for VPX tables with decals in: {}",
        folder_path.display()
    );
    println!();

    let mut tables_with_decals = 0;
    let mut total_tables = 0;
    let mut total_image_decals = 0;
    let mut total_text_decals = 0;
    let mut total_backglass_decals = 0;

    for entry in WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("vpx"))
                .unwrap_or(false)
        })
    {
        let vpx_path = entry.path();
        total_tables += 1;

        match vpx::read(vpx_path) {
            Ok(vpx) => {
                let mut image_decals = Vec::new();
                let mut text_decals = Vec::new();
                let mut backglass_decals = Vec::new();

                for item in &vpx.gameitems {
                    if let GameItemEnum::Decal(decal) = item {
                        if decal.backglass {
                            backglass_decals.push(decal);
                        } else {
                            match decal.decal_type {
                                DecalType::Image => image_decals.push(decal),
                                DecalType::Text => text_decals.push(decal),
                            }
                        }
                    }
                }

                let has_decals = !image_decals.is_empty()
                    || !text_decals.is_empty()
                    || !backglass_decals.is_empty();

                if has_decals {
                    tables_with_decals += 1;
                    total_image_decals += image_decals.len();
                    total_text_decals += text_decals.len();
                    total_backglass_decals += backglass_decals.len();

                    println!("📋 {}", vpx_path.display());

                    if !image_decals.is_empty() {
                        println!("   Image decals: {}", image_decals.len());
                        for decal in &image_decals {
                            let image_info = if decal.image.is_empty() {
                                "(no image)".to_string()
                            } else {
                                format!("image: {}", decal.image)
                            };
                            println!(
                                "     - {} [{}x{}, rot: {}°, {}]",
                                decal.name, decal.width, decal.height, decal.rotation, image_info
                            );
                        }
                    }

                    if !text_decals.is_empty() {
                        println!("   Text decals: {}", text_decals.len());
                        for decal in &text_decals {
                            let text_preview: String = decal.text.chars().take(30).collect();
                            let text_display = if decal.text.len() > 30 {
                                format!("{}...", text_preview)
                            } else {
                                text_preview
                            };
                            println!(
                                "     - {} [{}x{}, text: \"{}\"]",
                                decal.name, decal.width, decal.height, text_display
                            );
                        }
                    }

                    if !backglass_decals.is_empty() {
                        println!("   Backglass decals: {}", backglass_decals.len());
                        for decal in &backglass_decals {
                            let type_str = match decal.decal_type {
                                DecalType::Image => "image",
                                DecalType::Text => "text",
                            };
                            println!("     - {} [{}]", decal.name, type_str);
                        }
                    }

                    println!();
                }
            }
            Err(e) => {
                eprintln!("⚠️  Error reading {}: {}", vpx_path.display(), e);
            }
        }
    }

    println!("═══════════════════════════════════════════════════════════");
    println!("Summary:");
    println!("  Total tables scanned: {}", total_tables);
    println!("  Tables with decals: {}", tables_with_decals);
    println!("  Total image decals: {}", total_image_decals);
    println!(
        "  Total text decals: {} (not supported in glTF export)",
        total_text_decals
    );
    println!(
        "  Total backglass decals: {} (not supported in glTF export)",
        total_backglass_decals
    );

    Ok(())
}
