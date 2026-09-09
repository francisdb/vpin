//! Shrinks the images of a table for a device with a texture size limit
//! and writes the result to a new file.
//!
//! ```sh
//! cargo run --release --example shrink_images -- table.vpx 1536 shrunk.vpx [image to leave alone ...]
//! ```

use std::path::Path;
use vpin::vpx;
use vpin::vpx::images::{ImageShrink, ShrinkOptions};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, max_dimension, output, exclude @ ..] = args.as_slice() else {
        eprintln!(
            "usage: shrink_images <table.vpx> <max dimension> <output.vpx> [excluded image ...]"
        );
        std::process::exit(2);
    };
    let max_dimension: u32 = max_dimension.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("max dimension: {e}"),
        )
    })?;

    let mut table = vpx::read(Path::new(input))?;
    let conversions = table.images_to_webp();
    println!(
        "{} bitmap or png images converted to webp",
        conversions.len()
    );
    let results = table.shrink_images(&ShrinkOptions {
        max_dimension,
        exclude: exclude.to_vec(),
        ..Default::default()
    });
    let (mut before, mut after) = (0, 0);
    for result in &results {
        println!("{result}");
        if let ImageShrink::Shrunk {
            bytes_before,
            bytes_after,
            ..
        } = result
        {
            before += bytes_before;
            after += bytes_after;
        }
    }
    println!("image data {before} -> {after} bytes");
    vpx::write(Path::new(output), &table)?;
    println!(
        "file {} -> {} bytes",
        Path::new(input).metadata()?.len(),
        Path::new(output).metadata()?.len()
    );
    Ok(())
}
