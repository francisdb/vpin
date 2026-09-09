//! Prints what changed between two tables, in the terms a table author uses.
//!
//! ```sh
//! cargo run --example diff -- original.vpx modified.vpx
//! ```

use std::path::Path;
use vpin::vpx;
use vpin::vpx::diff::semantic;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, original, modified] = args.as_slice() else {
        eprintln!("usage: diff <original.vpx> <modified.vpx>");
        std::process::exit(2);
    };
    let original = vpx::read(Path::new(original))?;
    let modified = vpx::read(Path::new(modified))?;
    let changes = semantic::diff(&original, &modified);
    if changes.is_empty() {
        println!("no changes");
    }
    for change in &changes {
        match change {
            semantic::Change::Changed { entity, fields } => {
                println!("{entity}:");
                for field in fields {
                    println!("  {field}");
                }
            }
            other => println!("{other}"),
        }
    }
    Ok(())
}
