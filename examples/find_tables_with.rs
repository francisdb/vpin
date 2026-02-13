// Example showing how to search a folder for VPX tables that contain specific game items
//
// This is useful for finding test tables when working on specific game item support.
//
// Usage:
//   cargo run --example find_tables_with -- decals <path_to_folder>
//   cargo run --example find_tables_with -- balls <path_to_folder>
//   cargo run --example find_tables_with -- all <path_to_folder>

use std::env;
use std::path::PathBuf;
use vpin::vpx;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::ball::Ball;
use vpin::vpx::gameitem::decal::{Decal, DecalType};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchMode {
    Decals,
    Balls,
    All,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=warn (or info, debug) to see warnings
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let mode = match args[1].to_lowercase().as_str() {
        "decals" => SearchMode::Decals,
        "balls" => SearchMode::Balls,
        "all" => SearchMode::All,
        _ => {
            eprintln!("Error: Unknown search mode '{}'\n", args[1]);
            print_usage(&args[0]);
            std::process::exit(1);
        }
    };

    let folder_path = PathBuf::from(&args[2]);

    if !folder_path.exists() {
        eprintln!("Error: Folder not found: {}", folder_path.display());
        std::process::exit(1);
    }

    let mode_str = match mode {
        SearchMode::Decals => "decals",
        SearchMode::Balls => "balls",
        SearchMode::All => "decals and balls",
    };
    println!(
        "Searching for VPX tables with {} in: {}",
        mode_str,
        folder_path.display()
    );
    println!();

    let mut stats = SearchStats::default();

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
        stats.total_tables += 1;

        match vpx::read(vpx_path) {
            Ok(vpx) => {
                let mut table_decals = TableDecals::default();
                let mut table_balls: Vec<&Ball> = Vec::new();

                for item in &vpx.gameitems {
                    match item {
                        GameItemEnum::Decal(decal) => {
                            if mode == SearchMode::Decals || mode == SearchMode::All {
                                if decal.backglass {
                                    table_decals.backglass.push(decal);
                                } else {
                                    match decal.decal_type {
                                        DecalType::Image => table_decals.image.push(decal),
                                        DecalType::Text => table_decals.text.push(decal),
                                    }
                                }
                            }
                        }
                        GameItemEnum::Ball(ball) => {
                            if mode == SearchMode::Balls || mode == SearchMode::All {
                                table_balls.push(ball);
                            }
                        }
                        _ => {}
                    }
                }

                let has_decals = !table_decals.is_empty();
                let has_balls = !table_balls.is_empty();
                let should_display = match mode {
                    SearchMode::Decals => has_decals,
                    SearchMode::Balls => has_balls,
                    SearchMode::All => has_decals || has_balls,
                };

                if should_display {
                    println!("📋 {}", vpx_path.display());

                    // Display decals
                    if has_decals {
                        stats.tables_with_decals += 1;
                        stats.total_image_decals += table_decals.image.len();
                        stats.total_text_decals += table_decals.text.len();
                        stats.total_backglass_decals += table_decals.backglass.len();

                        print_decals(&table_decals);
                    }

                    // Display balls
                    if has_balls {
                        stats.tables_with_balls += 1;
                        stats.total_balls += table_balls.len();

                        print_balls(&table_balls);
                    }

                    println!();
                } else {
                    // Print a dot to show progress for tables without matches
                    use std::io::{Write, stdout};
                    print!(".");
                    stdout().flush().ok();
                }
            }
            Err(e) => {
                eprintln!("⚠️  Error reading {}: {}", vpx_path.display(), e);
            }
        }
    }

    print_summary(&stats, mode);

    Ok(())
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <mode> <path_to_folder>", program);
    eprintln!();
    eprintln!("Modes:");
    eprintln!("  decals  - Search for tables containing decals");
    eprintln!("  balls   - Search for tables containing balls (captive balls)");
    eprintln!("  all     - Search for tables containing decals or balls");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  cargo run --example find_tables_with -- decals /path/to/tables");
    eprintln!("  cargo run --example find_tables_with -- balls /path/to/tables");
    eprintln!("  cargo run --example find_tables_with -- all /path/to/tables");
}

#[derive(Default)]
struct TableDecals<'a> {
    image: Vec<&'a Decal>,
    text: Vec<&'a Decal>,
    backglass: Vec<&'a Decal>,
}

impl TableDecals<'_> {
    fn is_empty(&self) -> bool {
        self.image.is_empty() && self.text.is_empty() && self.backglass.is_empty()
    }
}

#[derive(Default)]
struct SearchStats {
    total_tables: usize,
    tables_with_decals: usize,
    tables_with_balls: usize,
    total_image_decals: usize,
    total_text_decals: usize,
    total_backglass_decals: usize,
    total_balls: usize,
}

fn print_decals(decals: &TableDecals) {
    if !decals.image.is_empty() {
        println!("   Image decals: {}", decals.image.len());
        for decal in &decals.image {
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

    if !decals.text.is_empty() {
        println!("   Text decals: {}", decals.text.len());
        for decal in &decals.text {
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

    if !decals.backglass.is_empty() {
        println!("   Backglass decals: {}", decals.backglass.len());
        for decal in &decals.backglass {
            let type_str = match decal.decal_type {
                DecalType::Image => "image",
                DecalType::Text => "text",
            };
            println!("     - {} [{}]", decal.name, type_str);
        }
    }
}

fn print_balls(balls: &[&Ball]) {
    println!("   Balls: {}", balls.len());
    for ball in balls {
        let image_info = if ball.image.is_empty() {
            "(default)".to_string()
        } else {
            format!("image: {}", ball.image)
        };
        println!(
            "     - {} [radius: {}, pos: ({:.1}, {:.1}, {:.1}), {}]",
            ball.name, ball.radius, ball.pos.x, ball.pos.y, ball.pos.z, image_info
        );
    }
}

fn print_summary(stats: &SearchStats, mode: SearchMode) {
    println!(); // Newline after progress dots
    println!("═══════════════════════════════════════════════════════════");
    println!("Summary:");
    println!("  Total tables scanned: {}", stats.total_tables);

    match mode {
        SearchMode::Decals => {
            println!("  Tables with decals: {}", stats.tables_with_decals);
            println!("  Total image decals: {}", stats.total_image_decals);
            println!(
                "  Total text decals: {} (not supported in glTF export)",
                stats.total_text_decals
            );
            println!(
                "  Total backglass decals: {} (not supported in glTF export)",
                stats.total_backglass_decals
            );
        }
        SearchMode::Balls => {
            println!("  Tables with balls: {}", stats.tables_with_balls);
            println!(
                "  Total balls: {} (captive balls, not yet supported in glTF export)",
                stats.total_balls
            );
        }
        SearchMode::All => {
            println!("  Tables with decals: {}", stats.tables_with_decals);
            println!("  Tables with balls: {}", stats.tables_with_balls);
            println!("  Total image decals: {}", stats.total_image_decals);
            println!(
                "  Total text decals: {} (not supported in glTF export)",
                stats.total_text_decals
            );
            println!(
                "  Total backglass decals: {} (not supported in glTF export)",
                stats.total_backglass_decals
            );
            println!(
                "  Total balls: {} (captive balls, not yet supported in glTF export)",
                stats.total_balls
            );
        }
    }
}
