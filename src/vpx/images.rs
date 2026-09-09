//! Bulk changes to the images of a table: converting the deprecated
//! bitmap format to webp.
//!
//! vpinball stopped writing bitmaps (`BITS` records, LZW compressed) in
//! 10.8.1; when it loads one it decodes it, drops an all opaque alpha
//! channel and keeps it as a lossless webp, so a table saved by a current
//! vpinball has no bitmaps left. [`VPX::bitmaps_to_webp`] does the same
//! on the parsed table, so it applies to a table however it was loaded,
//! and [`ImageData::bitmap_to_webp`] does it for a single image.
//! [`crate::vpx::VpxFile::images_to_webp`] converts in place in a file.
//!
//! Neither FlexDMD implementation reads a bitmap image out of a table
//! (they only read the encoded `JPEG` record), so nothing that worked is
//! lost by the conversion.
//!
//! # Png to webp
//!
//! [`VPX::pngs_to_webp`] goes further than vpinball: a png is stored as is
//! by vpinball, but lossless webp holds the same pixels in about three
//! quarters of the bytes. One consumer cannot follow: FlexDMD reads images
//! straight out of the table by the `VPX.name` syntax in the script,
//! decides the asset type from the stored file extension (png, jpg, jpeg
//! or bmp) and decodes with GDI+ on Windows and with an SDL_image built
//! without webp on standalone. Images the script hands to FlexDMD are
//! therefore left as png and reported.

use super::VPX;
use super::image::{ImageData, ImageDataJpeg, vpx_image_to_dynamic_image};
use ::image::codecs::jpeg::JpegEncoder;
use ::image::{DynamicImage, ImageFormat, ImageReader};
use log::warn;
use std::collections::HashSet;
use std::fmt;
use std::io;

/// What [`VPX::pngs_to_webp`] did with one png
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PngToWebp {
    /// The png was re-encoded as lossless webp
    Converted { name: String },
    /// The script hands the image to FlexDMD, which cannot read webp
    UsedByFlexDmd { name: String },
    /// The png data does not decode
    Undecodable { name: String, detail: String },
}

impl fmt::Display for PngToWebp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngToWebp::Converted { name } => write!(f, "image {name:?}: png -> webp"),
            PngToWebp::UsedByFlexDmd { name } => {
                write!(
                    f,
                    "image {name:?} kept as png, the script hands it to FlexDMD"
                )
            }
            PngToWebp::Undecodable { name, detail } => {
                write!(f, "image {name:?} kept, cannot decode: {detail}")
            }
        }
    }
}

/// An image that was re-encoded as webp, see [`VPX::bitmaps_to_webp`]
#[derive(Debug, PartialEq, Clone)]
pub struct ImageToWebpConversion {
    pub name: String,
    pub old_extension: String,
    pub new_extension: String,
}

impl VPX {
    /// Re-encodes every bitmap image as lossless webp, the way vpinball
    /// does when it loads one.
    ///
    /// Images that fail to decode are skipped with a warning. Returns the
    /// conversions that were made.
    pub fn bitmaps_to_webp(&mut self) -> Vec<ImageToWebpConversion> {
        let mut conversions = Vec::new();
        for image in &mut self.images {
            let old_extension = image.ext().to_lowercase();
            match image.bitmap_to_webp() {
                Ok(true) => conversions.push(ImageToWebpConversion {
                    name: image.name.clone(),
                    old_extension,
                    new_extension: "webp".to_string(),
                }),
                Ok(false) => {}
                Err(e) => warn!("Skipping image {}: {e}", image.name),
            }
        }
        conversions
    }
}

impl VPX {
    /// Re-encodes every png image as lossless webp, except the ones the
    /// script hands to FlexDMD, which cannot read webp. See the
    /// [module documentation](self).
    pub fn pngs_to_webp(&mut self) -> Vec<PngToWebp> {
        let flexdmd = flexdmd_images(&self.gamedata.code.string);
        let mut results = Vec::new();
        for image in &mut self.images {
            if !image.ext().eq_ignore_ascii_case("png") || image.is_link() {
                continue;
            }
            let name = image.name.clone();
            if flexdmd.contains(&name.to_lowercase()) {
                results.push(PngToWebp::UsedByFlexDmd { name });
                continue;
            }
            results.push(match image.png_to_webp() {
                Ok(true) => PngToWebp::Converted { name },
                Ok(false) => continue,
                Err(e) => PngToWebp::Undecodable {
                    name,
                    detail: e.to_string(),
                },
            });
        }
        results
    }
}

/// The names of the table images a script hands to FlexDMD, lower cased.
/// FlexDMD addresses them as `VPX.name` inside a string, optionally
/// followed by `&` and filter options or `|` and another image.
pub fn flexdmd_images(script: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for literal in string_literals(script) {
        let lower = literal.to_lowercase();
        let mut rest = lower.as_str();
        while let Some(start) = rest.find("vpx.") {
            let after = &rest[start + 4..];
            let end = after.find(['&', '|']).unwrap_or(after.len());
            let name = after[..end].trim();
            if !name.is_empty() {
                names.insert(name.to_string());
            }
            rest = &after[end..];
        }
    }
    names
}

/// The contents of the double quoted strings in a VBScript, comments
/// excluded; a doubled quote inside a string is one quote
fn string_literals(script: &str) -> Vec<String> {
    let mut literals = Vec::new();
    for line in script.lines() {
        let mut chars = line.chars().peekable();
        let mut current: Option<String> = None;
        while let Some(c) = chars.next() {
            match (&mut current, c) {
                (None, '"') => current = Some(String::new()),
                (None, '\'') => break,
                (None, _) => {}
                (Some(literal), '"') => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        literal.push('"');
                    } else if let Some(literal) = current.take() {
                        literals.push(literal);
                    }
                }
                (Some(literal), c) => literal.push(c),
            }
        }
    }
    literals
}

impl ImageData {
    /// The stored pixels, whatever the format
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::NotFound`] for a link, which has no data in the
    /// table, [`io::ErrorKind::InvalidData`] when the data does not decode.
    pub fn decode(&self) -> io::Result<DynamicImage> {
        if let Some(bits) = &self.bits {
            return vpx_image_to_dynamic_image(&bits.lzw_compressed_data, self.width, self.height);
        }
        if let Some(jpeg) = &self.jpeg {
            // the content decides the format, the extension is the fallback
            // for formats without a signature such as tga and hdr
            let mut reader = ImageReader::new(io::Cursor::new(&jpeg.data));
            if let Some(format) = ImageFormat::from_extension(self.ext()) {
                reader.set_format(format);
            }
            // the default limit of 512 MB rejects the 8k float bakes of
            // recent tables
            reader.no_limits();
            return reader
                .with_guessed_format()?
                .decode()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the image has no data in the table",
        ))
    }

    /// Re-encodes a bitmap image as lossless webp, the way vpinball does
    /// when it loads one. Returns `false` when the image is not a bitmap.
    ///
    /// # Errors
    ///
    /// When the bitmap data does not decode.
    pub fn bitmap_to_webp(&mut self) -> io::Result<bool> {
        if self.bits.is_none() {
            return Ok(false);
        }
        let decoded = self.decode()?;
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(true)
    }

    /// Re-encodes a png image as lossless webp. Returns `false` when the
    /// image is not a png. Mind that FlexDMD cannot read webp, see
    /// [`VPX::pngs_to_webp`] for a conversion that leaves those images
    /// alone.
    ///
    /// # Errors
    ///
    /// When the png data does not decode.
    pub fn png_to_webp(&mut self) -> io::Result<bool> {
        if self.is_link() || !self.ext().eq_ignore_ascii_case("png") {
            return Ok(false);
        }
        let decoded = self.decode()?;
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(true)
    }

    /// Replaces the image content; the hash vpinball keeps of the encoded
    /// bytes is dropped since it no longer matches
    fn set_data(&mut self, data: Vec<u8>, extension: &str, width: u32, height: u32) {
        let jpeg = self.jpeg.get_or_insert_with(|| ImageDataJpeg {
            path: self.path.clone(),
            name: self.name.clone(),
            internal_name: None,
            data: Vec::new(),
        });
        jpeg.data = data;
        self.bits = None;
        self.width = width;
        self.height = height;
        self.md5_hash = None;
        if !self.ext().eq_ignore_ascii_case(extension) {
            self.change_extension(extension);
        }
    }
}

/// Encodes for a format, converting the pixel layout to one the encoder
/// takes: jpeg has no alpha, webp is 8 bit, hdr and exr are float
fn encode(image: &DynamicImage, format: ImageFormat, jpeg_quality: u8) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut cursor = io::Cursor::new(&mut data);
    let result = match format {
        ImageFormat::Jpeg => image
            .to_rgb8()
            .write_with_encoder(JpegEncoder::new_with_quality(&mut cursor, jpeg_quality)),
        ImageFormat::WebP => match image {
            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => {
                image.write_to(&mut cursor, format)
            }
            _ => DynamicImage::ImageRgba8(image.to_rgba8()).write_to(&mut cursor, format),
        },
        ImageFormat::Hdr => {
            DynamicImage::ImageRgb32F(image.to_rgb32f()).write_to(&mut cursor, format)
        }
        ImageFormat::OpenExr => match image {
            DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
                image.write_to(&mut cursor, format)
            }
            _ => DynamicImage::ImageRgba32F(image.to_rgba32f()).write_to(&mut cursor, format),
        },
        ImageFormat::Png => image.write_to(&mut cursor, format),
        _ => DynamicImage::ImageRgba8(image.to_rgba8()).write_to(&mut cursor, format),
    };
    result.map_err(|e| io::Error::other(e.to_string()))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::image::ImageDataBits;
    use crate::vpx::lzw::to_lzw_blocks;
    use ::image::RgbaImage;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn pixels(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            ::image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        })
    }

    fn encoded_image(
        name: &str,
        extension: &str,
        width: u32,
        height: u32,
    ) -> TestResult<ImageData> {
        let format = ImageFormat::from_extension(extension).expect("a known extension");
        let data = encode(&DynamicImage::ImageRgba8(pixels(width, height)), format, 90)?;
        Ok(ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.{extension}"),
            width,
            height,
            jpeg: Some(ImageDataJpeg {
                path: format!("C:\\images\\{name}.{extension}"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            md5_hash: Some([7; 16]),
            ..Default::default()
        })
    }

    fn bitmap_image(name: &str, width: u32, height: u32) -> ImageData {
        // the bitmap format stores BGRA with the alpha at 255
        let mut bgra = pixels(width, height).into_raw();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.bmp"),
            width,
            height,
            bits: Some(ImageDataBits {
                lzw_compressed_data: to_lzw_blocks(&bgra),
            }),
            ..Default::default()
        }
    }

    fn link_image(name: &str) -> ImageData {
        ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.png"),
            width: 4000,
            height: 4000,
            link: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn a_bitmap_becomes_a_lossless_webp() -> TestResult {
        let mut image = bitmap_image("bmp", 40, 20);
        let before = image.decode()?.to_rgba8();
        assert!(image.bitmap_to_webp()?);
        assert_eq!(image.ext(), "webp");
        assert_eq!(image.path, "C:\\images\\bmp.webp");
        assert!(image.bits.is_none());
        assert_eq!(
            image.jpeg.as_ref().map(|j| j.path.as_str()),
            Some("C:\\images\\bmp.webp")
        );
        assert_eq!(image.decode()?.to_rgba8(), before);
        // a second run has nothing to do
        assert!(!image.bitmap_to_webp()?);
        Ok(())
    }

    #[test]
    fn flexdmd_image_names_are_found_in_the_script() {
        let script = r#"
            Dim x
            x = FlexDMD.NewImage("logo", "VPX.Logo_Big&dmd=2")
            FlexDMD.NewVideo("v", "VPX.Anim|VPX.Anim2")
            ' a comment: "VPX.Commented" is not a reference
            y = "say ""hi"" then vpx.spaced name "
        "#;
        let names = flexdmd_images(script);
        assert_eq!(
            names,
            ["logo_big", "anim", "anim2", "spaced name"]
                .into_iter()
                .map(String::from)
                .collect()
        );
        assert!(flexdmd_images("").is_empty());
    }

    #[test]
    fn pngs_become_webp_unless_flexdmd_uses_them() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(encoded_image("Playfield", "png", 8, 8)?);
        vpx.add_or_replace_image(encoded_image("DmdFont", "png", 8, 8)?);
        vpx.add_or_replace_image(encoded_image("Photo", "jpg", 8, 8)?);
        let mut broken = encoded_image("Broken", "png", 8, 8)?;
        broken.jpeg.as_mut().expect("has data").data = vec![1, 2, 3];
        vpx.add_or_replace_image(broken);
        vpx.gamedata
            .set_code("FlexDMD.NewImage(\"f\", \"VPX.dmdfont&dmd=2\")\r\n".to_string());

        let results = vpx.pngs_to_webp();
        assert_eq!(results.len(), 3, "{results:?}");
        assert_eq!(
            results[0],
            PngToWebp::Converted {
                name: "Playfield".to_string()
            }
        );
        assert_eq!(
            results[1],
            PngToWebp::UsedByFlexDmd {
                name: "DmdFont".to_string()
            }
        );
        assert!(matches!(&results[2], PngToWebp::Undecodable { name, .. } if name == "Broken"));
        assert_eq!(vpx.images[0].ext(), "webp");
        assert_eq!(vpx.images[1].ext(), "png");
        assert_eq!(vpx.images[2].ext(), "jpg");
        assert_eq!(vpx.images[3].ext(), "png");
        assert_eq!(
            results[1].to_string(),
            "image \"DmdFont\" kept as png, the script hands it to FlexDMD"
        );
        Ok(())
    }

    #[test]
    fn encoded_images_are_left_alone() -> TestResult {
        for extension in ["png", "jpg"] {
            let mut image = encoded_image(extension, extension, 40, 20)?;
            assert!(!image.bitmap_to_webp()?);
            assert_eq!(image.ext(), extension);
            assert_eq!(image.md5_hash, Some([7; 16]));
        }
        assert!(!link_image("link").bitmap_to_webp()?);
        Ok(())
    }

    #[test]
    fn a_table_converts_its_bitmaps() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(bitmap_image("bmp", 8, 8));
        vpx.add_or_replace_image(encoded_image("png", "png", 8, 8)?);
        vpx.add_or_replace_image(encoded_image("jpg", "jpg", 8, 8)?);
        let conversions = vpx.bitmaps_to_webp();
        assert_eq!(
            conversions,
            vec![ImageToWebpConversion {
                name: "bmp".to_string(),
                old_extension: "bmp".to_string(),
                new_extension: "webp".to_string(),
            }]
        );
        assert!(vpx.images.iter().all(|image| image.bits.is_none()));
        assert_eq!(vpx.images[1].ext(), "png");
        Ok(())
    }
}
