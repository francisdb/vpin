//! Image reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::image::{ImageData, ImageDataJson, swap_red_and_blue, vpx_image_to_dynamic_image};
use crate::vpx::lzw::to_lzw_blocks;
use log::{debug, info, warn};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{self, BufRead, Seek};
use std::path::Path;

use super::WriteError;
use super::util::{read_json, sanitize_filename};

struct ImageBmp {
    width: u32,
    height: u32,
    lzw_compressed_data: Vec<u8>,
}

pub(super) fn write_images<P: AsRef<Path>>(
    images: &[ImageData],
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    info!("Starting image processing - total images: {}", images.len());

    let images_index_path = expanded_dir.as_ref().join("images.json");
    let mut images_index_file = fs.create_file(&images_index_path)?;
    let mut image_names_lower: HashSet<String> = HashSet::new();
    let mut image_names_dupe_counter = 0;
    let mut json_images = Vec::with_capacity(images.len());
    let images_list: io::Result<Vec<(String, &ImageData)>> = images
        .iter()
        .enumerate()
        .map(|(image_index, image)| {
            debug!(
                "Processing image {}/{}: name='{}', size={}x{}",
                image_index + 1,
                images.len(),
                image.name,
                image.width,
                image.height
            );
            let mut json = ImageDataJson::from_image_data(image);
            let name_sanitized = sanitize_filename(&image.name);
            if name_sanitized != image.name {
                info!(
                    "Image name {} contained invalid characters, sanitized to {}",
                    image.name, &name_sanitized
                );
                json.name_dedup = Some(name_sanitized.clone());
            }
            let lower_name = name_sanitized.to_lowercase();
            if image_names_lower.contains(&lower_name) {
                image_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", image.name, image_names_dupe_counter);
                info!(
                    "Image name {} is not unique, renaming file to {}",
                    name_sanitized, &name_dedup
                );
                json.name_dedup = Some(name_dedup);
            }
            image_names_lower.insert(lower_name);

            let actual_name = json.name_dedup.as_ref().unwrap_or(&image.name);
            let file_name = format!("{}.{}", actual_name, image.ext());

            if let Some(jpeg) = &image.jpeg {
                // Only if the actual image dimensions are different from
                // the ones in the vpx file we add them to the json.
                let cursor = io::Cursor::new(&jpeg.data);
                let dimensions_file = read_image_dimensions_from_file_steam(&file_name, cursor)?;
                match dimensions_file {
                    Some((width_file, height_file)) => {
                        if image.width != width_file || image.height != height_file {
                            info!(
                                "Stale image dimensions for {} in vpx {}x{} vs in image {}x{}",
                                file_name, image.width, image.height, width_file, height_file
                            );
                        }
                        if image.width != width_file {
                            json.width = Some(image.width);
                        }
                        if image.height != height_file {
                            json.height = Some(image.height);
                        }
                    }
                    None => {
                        json.width = Some(image.width);
                        json.height = Some(image.height);
                    }
                }
            };
            if image.link.is_some() {
                // Links always store the dimensions in the json
                json.width = Some(image.width);
                json.height = Some(image.height);
            }
            // for bits images we don't store the dimensions in the json as they always match

            json_images.push(json);
            debug!(
                "Successfully processed image {}/{}: '{}'",
                image_index + 1,
                images.len(),
                image.name
            );
            Ok((file_name, image))
        })
        .collect();
    let images_list = images_list?;
    serde_json::to_writer_pretty(&mut images_index_file, &json_images)?;

    let images_dir = expanded_dir.as_ref().join("images");
    fs.create_dir_all(&images_dir)?;
    debug!("Created images directory: {}", images_dir.display());
    info!(
        "Starting to write {} image files to disk",
        images_list.len()
    );

    images_list
        .iter()
        .enumerate()
        .try_for_each(|(file_index, (image_file_name, image))| {
            debug!(
                "Writing image file {}/{}: '{}'",
                file_index + 1,
                images_list.len(),
                image_file_name
            );
            let file_path = images_dir.join(image_file_name);
            debug!("Full file path: {}", file_path.display());

            if !fs.exists(&file_path) {
                if image.is_link() {
                    info!("Image is a link, no data to write");
                    Ok(())
                } else if let Some(jpeg) = &image.jpeg {
                    debug!("Writing JPEG data ({} bytes)", jpeg.data.len());
                    fs.write_file(&file_path, &jpeg.data).map_err(|e| {
                        warn!(
                            "ERROR: Failed to write JPEG data for '{}': {}",
                            file_path.display(),
                            e
                        );
                        e
                    })
                } else if let Some(bits) = &image.bits {
                    debug!(
                        "Writing BMP data (compressed size: {} bytes)",
                        bits.lzw_compressed_data.len()
                    );
                    assert_eq!(
                        image.ext().to_ascii_lowercase(),
                        "bmp",
                        "Images stored as bits should have the extension .bmp"
                    );

                    write_image_bmp(
                        &file_path,
                        &bits.lzw_compressed_data,
                        image.width,
                        image.height,
                        fs,
                    )
                    .map_err(|e| {
                        warn!(
                            "ERROR: Failed to write BMP image '{}': {}",
                            file_path.display(),
                            e
                        );
                        e
                    })
                } else {
                    let err = io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Image has no data: {}", file_path.display()),
                    );
                    warn!("ERROR: {}", err);
                    Err(err)
                }
            } else {
                let err = io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Two images with the same name detected, should not happen: {}",
                        file_path.display()
                    ),
                );
                warn!("ERROR: {}", err);
                Err(err)
            }
        })?;
    info!(
        "Successfully completed writing all {} images",
        images_list.len()
    );
    Ok(())
}

fn write_image_bmp(
    file_path: &Path,
    lzw_compressed_data: &[u8],
    width: u32,
    height: u32,
    fs: &dyn FileSystem,
) -> io::Result<()> {
    let image_to_save = vpx_image_to_dynamic_image(lzw_compressed_data, width, height);
    if image_to_save.color().has_alpha() {
        // One example is the table "Guns N Roses (Data East 1994).vpx"
        // that contains vp9 images with non-255 alpha values.
        // They are actually labeled as sRGBA in the Visual Pinball image manager.
        // However, when Visual Pinball itself exports the image it drops the alpha values.
        let file_name = file_path
            .file_name()
            .map(OsStr::to_string_lossy)
            .unwrap_or_default();
        warn!(
            "Image {file_name} has non-opaque pixels, writing as RGBA BMP that might not be supported by all applications"
        );
    }
    let mut buffer = io::Cursor::new(Vec::new());
    image_to_save
        .write_to(&mut buffer, image::ImageFormat::Bmp)
        .map_err(|image_error| {
            io::Error::other(format!(
                "Failed to encode bitmap {}: {}",
                file_path.display(),
                image_error
            ))
        })?;
    fs.write_file(file_path, buffer.get_ref())
}

pub(super) fn read_images<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<ImageData>> {
    let images_json_path = expanded_dir.as_ref().join("images.json");
    if !fs.exists(&images_json_path) {
        info!("No images.json found");
        return Ok(vec![]);
    }
    let images_json: Vec<ImageDataJson> = read_json(&images_json_path, fs)?;
    let images_dir = expanded_dir.as_ref().join("images");
    let images: io::Result<Vec<ImageData>> = images_json
        .into_iter()
        .map(|image_data_json| {
            if image_data_json.is_link() {
                let image = image_data_json.to_image_data(
                    image_data_json.width.unwrap_or(0),
                    image_data_json.height.unwrap_or(0),
                    None,
                );
                Ok(image)
            } else {
                let file_name = image_data_json
                    .name_dedup
                    .as_ref()
                    .unwrap_or(&image_data_json.name);
                let full_file_name = format!("{}.{}", file_name, image_data_json.ext());
                let mut file_path = images_dir.join(&full_file_name);

                let mut new_extension = None;
                if image_data_json.ext() == "png" && !fs.exists(&file_path) {
                    let file_path_webp = images_dir.join(format!("{file_name}.webp"));
                    if fs.exists(&file_path_webp) {
                        new_extension = Some("webp");
                        file_path = file_path_webp;
                    }
                }

                if fs.exists(&file_path) {
                    let image_data = fs.read_file(&file_path)?;
                    let image = if image_data_json.is_bmp() {
                        use crate::vpx::image::ImageDataBits;
                        let read_bmp = read_image_bmp(&image_data).map_err(|e| {
                            io::Error::new(
                                e.kind(),
                                format!("Failed to read BMP '{}' ({} bytes): {}", file_path.display(), image_data.len(), e)
                            )
                        })?;
                        let bits = ImageDataBits {
                            lzw_compressed_data: read_bmp.lzw_compressed_data,
                        };
                        image_data_json.to_image_data(
                            read_bmp.width,
                            read_bmp.height,
                            Some(bits),
                        )
                    } else {
                        let dimensions_from_file = read_image_dimensions_from_bytes(&full_file_name, &image_data)?;

                        let width = match image_data_json.width {
                            Some(w) => w,
                            None =>
                                match dimensions_from_file {
                                    Some((width_file, _)) => width_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image width not provided and could not be read from file")),
                                }
                        };

                        let height = match image_data_json.height {
                            Some(h) => h,
                            None =>
                                match dimensions_from_file {
                                    Some((_, height_file)) => height_file,
                                    None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Image height not provided and could not be read from file")),
                                }
                        };

                        if let Some((image_w, image_h)) = dimensions_from_file && (width != image_w || height != image_h) {
                            warn!(
                                "Stale image dimensions for {full_file_name} in json {}x{} vs in image {}x{}",
                                width, height, image_w, image_h
                            );
                        }

                        let mut image = image_data_json.to_image_data(width, height, None);
                        if let Some(jpg) = &mut image.jpeg {
                            jpg.data = image_data;
                        }
                        if let Some(new_extension) = new_extension {
                            // we need to change the file extension for the path
                            image.change_extension(new_extension);
                        }
                        image
                    };
                    Ok(image)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Image file not found: {}", file_path.display()),
                    ))
                }
            }
        })
        .collect();
    images
}

fn read_image_dimensions_from_file_steam<R: BufRead + Seek>(
    file_name: &str,
    reader: R,
) -> io::Result<Option<(u32, u32)>> {
    let dimensions_from_file = match image::ImageFormat::from_path(file_name) {
        Ok(format) => {
            let decoder = image::ImageReader::with_format(reader, format).with_guessed_format()?;
            if Some(format) != decoder.format() {
                warn!(
                    "Detected image format {} for [{}] where the extension suggests {:?}",
                    decoder
                        .format()
                        .map_or("unknown".to_string(), |f| format!("{f:?}")),
                    file_name,
                    format,
                );
            }
            match decoder.into_dimensions() {
                Ok(dimensions) => Some(dimensions),
                Err(image_error) => {
                    warn!("Failed to read image dimensions for {file_name}: {image_error}");
                    None
                }
            }
        }
        Err(e) => {
            warn!("Failed to determine image format for {file_name}: {e}");
            None
        }
    };
    Ok(dimensions_from_file)
}

fn read_image_dimensions_from_bytes(
    file_name: &str,
    data: &[u8],
) -> io::Result<Option<(u32, u32)>> {
    let cursor = io::Cursor::new(data);
    read_image_dimensions_from_file_steam(file_name, cursor)
}

fn read_image_bmp(data: &[u8]) -> io::Result<ImageBmp> {
    // Use auto-detection instead of forcing BMP format for better compatibility
    let image = image::load_from_memory(data).map_err(|image_error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to read BMP image: {image_error}"),
        )
    })?;

    let raw_rgba = match image.color() {
        image::ColorType::Rgb8 => image.to_rgba8().into_raw(),
        image::ColorType::Rgba8 => image.to_rgba8().into_raw(),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("BMP image uses {other:?}, expecting Rgb8 or Rgba8 format"),
            ));
        }
    };

    // convert to BGRA
    let raw_bgra: Vec<u8> = swap_red_and_blue(&raw_rgba);

    let image_bmp = ImageBmp {
        width: image.width(),
        height: image.height(),
        lzw_compressed_data: to_lzw_blocks(&raw_bgra),
    };

    Ok(image_bmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::BufReader;

    #[test]
    fn test_read_image_dimensions_fail_invalid_unknown() {
        use std::io;
        let cursor = io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.zero", reader).unwrap();

        assert_eq!(dimensions, None);
    }

    #[test]
    fn test_read_image_dimensions_fail_invalid_png() {
        use std::io;
        let cursor = io::Cursor::new(vec![0; 10]);
        let reader = BufReader::new(cursor);
        let dimensions = read_image_dimensions_from_file_steam("test.png", reader).unwrap();

        assert_eq!(dimensions, None);
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_read_image_dimensions_png_as_hdr_stream() {
        use std::fs::File;
        // this file is actually a png file but with hdr extension
        // see https://github.com/francisdb/vpin/issues/110
        let hdr_path = Path::new("testdata").join("wrongly_labeled_png.hdr");
        let file = File::open(&hdr_path).unwrap();
        let reader = BufReader::new(file);
        let dimensions =
            read_image_dimensions_from_file_steam("wrongly_labeled_png.hdr", reader).unwrap();

        assert_eq!(dimensions, Some((512, 256)));
    }
}
