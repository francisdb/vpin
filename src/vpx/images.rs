//! Bulk changes to the images of a table: converting the deprecated bitmap
//! and png formats to webp, and shrinking images for devices with a
//! texture size limit.
//!
//! Both work on the parsed [`VPX`], so they apply to a table however it was
//! loaded, and [`ImageData`] offers the same operations for a single image.
//! [`crate::vpx::VpxFile::images_to_webp`] converts in place in a file.
//!
//! # Shrinking for mobile
//!
//! vpinball scales every texture above its "Maximum texture dimension"
//! setting down when a table loads (1536 on mobile by default; people on
//! phones with little memory go as low as 768). [`VPX::shrink_images`]
//! applies the same rule once, in the file: the longest side of an image
//! is brought down to the limit, keeping the aspect ratio and the image
//! format, so the device neither decodes the full size image nor keeps it
//! in memory. Images it must not touch are left alone and reported:
//!
//! - color grade lookup tables, whose 256x16 layout the shader depends on
//! - images the caller excludes by name, which is how to protect FlexDMD
//!   artwork: vpinball does not know which images a script hands to
//!   FlexDMD, and scaled ones render wrong
//! - links to files outside the table
//! - images that do not decode, such as exr bakes with DWAA compression,
//!   which the exr decoder does not support yet
//!
//! Re-encoding a resampled image can take more bytes than the original,
//! typically for lossless formats and for jpegs that shrink only a little.
//! Such an image is kept as it was unless the caller allows growth, and
//! every result carries the byte counts.

use super::VPX;
use super::image::{ImageData, ImageDataJpeg, vpx_image_to_dynamic_image};
use ::image::codecs::jpeg::JpegEncoder;
use ::image::imageops::FilterType;
use ::image::{DynamicImage, ImageFormat, ImageReader};
use log::warn;
use std::fmt;
use std::io;

/// vpinball's default maximum texture dimension on mobile
pub const MOBILE_MAX_TEXTURE_DIMENSION: u32 = 1536;

/// An image that was re-encoded as webp, see [`VPX::images_to_webp`]
#[derive(Debug, PartialEq, Clone)]
pub struct ImageToWebpConversion {
    pub name: String,
    pub old_extension: String,
    pub new_extension: String,
}

/// How [`VPX::shrink_images`] shrinks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrinkOptions {
    /// The longest side an image may have; larger images are scaled down
    /// keeping their aspect ratio
    pub max_dimension: u32,
    /// Names of images to leave alone, compared case insensitively. Use it
    /// for FlexDMD artwork, which breaks when scaled
    pub exclude: Vec<String>,
    /// Quality for re-encoding jpeg images, 1 to 100
    pub jpeg_quality: u8,
    /// Keep a shrunk image even when its encoding grew. Off by default:
    /// vpinball scales the texture down on load anyway, so a larger file
    /// buys nothing
    pub allow_growth: bool,
}

impl Default for ShrinkOptions {
    fn default() -> Self {
        ShrinkOptions {
            max_dimension: MOBILE_MAX_TEXTURE_DIMENSION,
            exclude: Vec::new(),
            jpeg_quality: 90,
            allow_growth: false,
        }
    }
}

/// What [`VPX::shrink_images`] did with one image
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageShrink {
    /// The image was scaled down and re-encoded
    Shrunk {
        name: String,
        /// Width and height before
        from: (u32, u32),
        /// Width and height after
        to: (u32, u32),
        bytes_before: usize,
        bytes_after: usize,
        /// The extension after re-encoding; a bitmap becomes a webp
        extension: String,
    },
    /// The image was left as it was
    Skipped { name: String, reason: SkipReason },
}

/// Why [`VPX::shrink_images`] left an image alone
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// The image is a link to a file outside the table
    Link,
    /// The image is a color grade lookup table, which has a fixed layout
    ColorGradeLut,
    /// The caller excluded the image by name
    Excluded,
    /// The image format cannot be written back
    UnsupportedFormat(String),
    /// The image data could not be decoded
    Undecodable(String),
    /// Re-encoding the shrunk image took more bytes than the original;
    /// lossless formats often do after resampling
    WouldGrow {
        bytes_before: usize,
        bytes_after: usize,
    },
}

impl fmt::Display for ImageShrink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageShrink::Shrunk {
                name,
                from,
                to,
                bytes_before,
                bytes_after,
                extension,
            } => write!(
                f,
                "image {name:?}: {}x{} -> {}x{} {extension}, {bytes_before} -> {bytes_after} bytes",
                from.0, from.1, to.0, to.1
            ),
            ImageShrink::Skipped { name, reason } => write!(f, "image {name:?} skipped: {reason}"),
        }
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::Link => write!(f, "links to a file outside the table"),
            SkipReason::ColorGradeLut => write!(f, "color grade lookup table"),
            SkipReason::Excluded => write!(f, "excluded"),
            SkipReason::UnsupportedFormat(ext) => write!(f, "cannot write {ext} images"),
            SkipReason::Undecodable(detail) => write!(f, "cannot decode: {detail}"),
            SkipReason::WouldGrow {
                bytes_before,
                bytes_after,
            } => write!(f, "would grow from {bytes_before} to {bytes_after} bytes"),
        }
    }
}

impl VPX {
    /// Re-encodes every bitmap and png image as lossless webp, which is
    /// what vpinball itself suggests: it stopped writing bitmaps in 10.8.1
    /// and webp stores the same pixels much smaller.
    ///
    /// Images that fail to decode are skipped with a warning. Returns the
    /// conversions that were made.
    pub fn images_to_webp(&mut self) -> Vec<ImageToWebpConversion> {
        let mut conversions = Vec::new();
        for image in &mut self.images {
            let old_extension = image.ext().to_lowercase();
            match image.to_webp() {
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

    /// Scales every image above `options.max_dimension` down to it, see
    /// the [module documentation](self) for what is left alone.
    pub fn shrink_images(&mut self, options: &ShrinkOptions) -> Vec<ImageShrink> {
        let lut_name = self.gamedata.image_color_grade.to_lowercase();
        let mut results = Vec::new();
        for image in &mut self.images {
            let name = image.name.clone();
            let lower = name.to_lowercase();
            let skipped = |reason| ImageShrink::Skipped {
                name: name.clone(),
                reason,
            };
            if image.is_link() {
                results.push(skipped(SkipReason::Link));
                continue;
            }
            if options.exclude.iter().any(|e| e.to_lowercase() == lower) {
                results.push(skipped(SkipReason::Excluded));
                continue;
            }
            // any 256x16 image is a lookup table a script may switch to
            if (!lut_name.is_empty() && lower == lut_name)
                || (image.width, image.height) == (256, 16)
            {
                results.push(skipped(SkipReason::ColorGradeLut));
                continue;
            }
            let from = (image.width, image.height);
            let bytes_before = image.data_len();
            let original = (!options.allow_growth).then(|| image.clone());
            match image.shrink_to_fit(options.max_dimension, options.jpeg_quality) {
                Ok(Some(to)) => {
                    let bytes_after = image.data_len();
                    if let Some(original) = original
                        && bytes_after > bytes_before
                    {
                        *image = original;
                        results.push(skipped(SkipReason::WouldGrow {
                            bytes_before,
                            bytes_after,
                        }));
                        continue;
                    }
                    results.push(ImageShrink::Shrunk {
                        name,
                        from,
                        to,
                        bytes_before,
                        bytes_after,
                        extension: image.ext().to_lowercase(),
                    });
                }
                Ok(None) => {}
                Err(e) if e.kind() == io::ErrorKind::Unsupported => {
                    results.push(skipped(SkipReason::UnsupportedFormat(
                        image.ext().to_lowercase(),
                    )));
                }
                Err(e) => results.push(skipped(SkipReason::Undecodable(e.to_string()))),
            }
        }
        results
    }
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

    /// Re-encodes a bitmap or png image as lossless webp. Returns `false`
    /// when the image is something else already, or a link.
    ///
    /// # Errors
    ///
    /// When the image data does not decode.
    pub fn to_webp(&mut self) -> io::Result<bool> {
        let is_png = self.ext().eq_ignore_ascii_case("png");
        if self.is_link() || !(self.bits.is_some() || is_png) {
            return Ok(false);
        }
        let decoded = self.decode()?;
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(true)
    }

    /// Scales the image down so its longest side is at most
    /// `max_dimension`, keeping the aspect ratio and the format. A bitmap
    /// becomes a webp. Returns the new size, or `None` when the image
    /// already fit or is a link.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::Unsupported`] when the format cannot be written,
    /// otherwise when the image data does not decode.
    pub fn shrink_to_fit(
        &mut self,
        max_dimension: u32,
        jpeg_quality: u8,
    ) -> io::Result<Option<(u32, u32)>> {
        if self.is_link() || self.width.max(self.height) <= max_dimension {
            return Ok(None);
        }
        let (extension, format) = if self.bits.is_some() {
            ("webp".to_string(), ImageFormat::WebP)
        } else {
            let extension = self.ext().to_lowercase();
            let format = ImageFormat::from_extension(&extension)
                .filter(ImageFormat::writing_enabled)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        format!("cannot write {extension} images"),
                    )
                })?;
            (extension, format)
        };
        let decoded = self.decode()?;
        let (width, height) = fit(decoded.width(), decoded.height(), max_dimension);
        let resized = resize(&decoded, width, height);
        let data = encode(&resized, format, jpeg_quality)?;
        self.set_data(data, &extension, width, height);
        Ok(Some((width, height)))
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

    /// The size of the stored image data
    fn data_len(&self) -> usize {
        match (&self.jpeg, &self.bits) {
            (Some(jpeg), _) => jpeg.data.len(),
            (None, Some(bits)) => bits.lzw_compressed_data.len(),
            (None, None) => 0,
        }
    }
}

/// The size vpinball scales a texture to: the longest side becomes
/// `max_dimension`, the other side follows the aspect ratio
fn fit(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let longest = u64::from(width.max(height));
    let scale = |side: u32| ((u64::from(side) * u64::from(max_dimension)) / longest).max(1) as u32;
    (scale(width), scale(height))
}

/// Resizes with a high quality filter. The resampler clamps every channel
/// to 0..1, which would flatten an hdr or exr bake, so float images are
/// scaled to that range first and back afterwards.
fn resize(image: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    let filter = FilterType::Lanczos3;
    match image {
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
            let mut float = image.to_rgba32f();
            let peak = float
                .pixels()
                .flat_map(|pixel| pixel.0[..3].iter().copied())
                .fold(1.0f32, f32::max);
            if peak > 1.0 {
                for pixel in float.pixels_mut() {
                    for channel in &mut pixel.0[..3] {
                        *channel /= peak;
                    }
                }
            }
            let mut resized = ::image::imageops::resize(&float, width, height, filter);
            if peak > 1.0 {
                for pixel in resized.pixels_mut() {
                    for channel in &mut pixel.0[..3] {
                        *channel *= peak;
                    }
                }
            }
            match image {
                DynamicImage::ImageRgb32F(_) => {
                    DynamicImage::ImageRgb32F(DynamicImage::ImageRgba32F(resized).to_rgb32f())
                }
                _ => DynamicImage::ImageRgba32F(resized),
            }
        }
        _ => image.resize_exact(width, height, filter),
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
    use ::image::{Rgb, Rgb32FImage, RgbaImage};
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
        assert!(image.to_webp()?);
        assert_eq!(image.ext(), "webp");
        assert_eq!(image.path, "C:\\images\\bmp.webp");
        assert!(image.bits.is_none());
        assert_eq!(
            image.jpeg.as_ref().map(|j| j.path.as_str()),
            Some("C:\\images\\bmp.webp")
        );
        assert_eq!(image.decode()?.to_rgba8(), before);
        // a second run has nothing to do
        assert!(!image.to_webp()?);
        Ok(())
    }

    #[test]
    fn a_png_becomes_a_webp_and_a_jpeg_stays() -> TestResult {
        let mut png = encoded_image("png", "png", 40, 20)?;
        let before = png.decode()?.to_rgba8();
        assert!(png.to_webp()?);
        assert_eq!(png.ext(), "webp");
        assert_eq!(png.decode()?.to_rgba8(), before);
        assert_eq!(png.md5_hash, None);

        let mut jpg = encoded_image("jpg", "jpg", 40, 20)?;
        assert!(!jpg.to_webp()?);
        assert_eq!(jpg.ext(), "jpg");
        assert!(!link_image("link").to_webp()?);
        Ok(())
    }

    #[test]
    fn a_table_converts_its_images() -> TestResult {
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(bitmap_image("bmp", 8, 8));
        vpx.add_or_replace_image(encoded_image("png", "png", 8, 8)?);
        vpx.add_or_replace_image(encoded_image("jpg", "jpg", 8, 8)?);
        let conversions = vpx.images_to_webp();
        assert_eq!(
            conversions,
            vec![
                ImageToWebpConversion {
                    name: "bmp".to_string(),
                    old_extension: "bmp".to_string(),
                    new_extension: "webp".to_string(),
                },
                ImageToWebpConversion {
                    name: "png".to_string(),
                    old_extension: "png".to_string(),
                    new_extension: "webp".to_string(),
                },
            ]
        );
        assert!(vpx.images.iter().all(|image| image.bits.is_none()));
        Ok(())
    }

    #[test]
    fn fit_keeps_the_aspect_ratio_like_vpinball() {
        assert_eq!(fit(4000, 2000, 1536), (1536, 768));
        assert_eq!(fit(100, 3000, 1536), (51, 1536));
        assert_eq!(fit(3000, 3000, 768), (768, 768));
        assert_eq!(fit(5000, 1, 1000), (1000, 1));
    }

    #[test]
    fn shrinking_keeps_the_format() -> TestResult {
        for extension in ["png", "jpg", "webp", "tga", "gif"] {
            let mut image = encoded_image(extension, extension, 200, 100)?;
            assert_eq!(image.shrink_to_fit(50, 90)?, Some((50, 25)));
            assert_eq!(image.ext(), extension, "{extension}");
            assert_eq!((image.width, image.height), (50, 25));
            let decoded = image.decode()?;
            assert_eq!((decoded.width(), decoded.height()), (50, 25), "{extension}");
            assert_eq!(image.md5_hash, None);
            // it fits now
            assert_eq!(image.shrink_to_fit(50, 90)?, None);
        }
        Ok(())
    }

    #[test]
    fn shrinking_float_images_keeps_them_float() -> TestResult {
        for (extension, format) in [("exr", ImageFormat::OpenExr), ("hdr", ImageFormat::Hdr)] {
            let float = Rgb32FImage::from_fn(64, 32, |x, _| Rgb([x as f32 / 64.0, 2.0, 0.5]));
            let data = encode(&DynamicImage::ImageRgb32F(float), format, 0)?;
            let mut image = ImageData {
                name: extension.to_string(),
                path: format!("bake.{extension}"),
                width: 64,
                height: 32,
                jpeg: Some(ImageDataJpeg {
                    path: format!("bake.{extension}"),
                    name: extension.to_string(),
                    internal_name: None,
                    data,
                }),
                ..Default::default()
            };
            assert_eq!(image.shrink_to_fit(16, 90)?, Some((16, 8)));
            assert_eq!(image.ext(), extension);
            let decoded = image.decode()?;
            assert!(
                matches!(
                    decoded,
                    DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_)
                ),
                "{extension} decoded as {:?}",
                decoded.color()
            );
            // values above 1.0 survive
            assert!(
                decoded.to_rgb32f().pixels().any(|p| p[1] > 1.5),
                "{extension}"
            );
        }
        Ok(())
    }

    #[test]
    fn an_image_that_would_grow_is_kept() -> TestResult {
        // a flat image is tiny as png; resampling it barely changes the
        // pixels, but a small enough limit must still make it smaller
        let flat = |name: &str| {
            let mut image = encoded_image(name, "png", 400, 400).expect("encodes");
            let pixels = RgbaImage::from_pixel(400, 400, ::image::Rgba([200, 100, 50, 255]));
            image.jpeg.as_mut().expect("has data").data =
                encode(&DynamicImage::ImageRgba8(pixels), ImageFormat::Png, 0).expect("encodes");
            image
        };
        let mut vpx = VPX::default();
        vpx.add_or_replace_image(flat("flat"));
        // a jpeg that shrinks by a few pixels grows through re-encoding
        vpx.add_or_replace_image(encoded_image("photo", "jpg", 400, 400)?);
        let before = vpx.images[1].jpeg.as_ref().expect("has data").data.clone();

        let photo_result = |results: &[ImageShrink]| {
            results
                .iter()
                .find(|result| match result {
                    ImageShrink::Shrunk { name, .. } | ImageShrink::Skipped { name, .. } => {
                        name == "photo"
                    }
                })
                .cloned()
                .expect("a result for the photo")
        };
        let results = vpx.shrink_images(&ShrinkOptions {
            max_dimension: 398,
            ..Default::default()
        });
        assert!(
            matches!(
                photo_result(&results),
                ImageShrink::Skipped {
                    reason: SkipReason::WouldGrow { .. },
                    ..
                }
            ),
            "{}",
            photo_result(&results)
        );
        // the original was kept as it was
        assert_eq!(vpx.images[1].width, 400);
        assert_eq!(vpx.images[1].jpeg.as_ref().expect("has data").data, before);
        assert!(
            photo_result(&results)
                .to_string()
                .starts_with("image \"photo\" skipped: would grow from")
        );

        let results = vpx.shrink_images(&ShrinkOptions {
            max_dimension: 398,
            allow_growth: true,
            ..Default::default()
        });
        assert!(matches!(photo_result(&results), ImageShrink::Shrunk { .. }));
        assert_eq!(vpx.images[1].width, 398);
        Ok(())
    }

    #[test]
    fn a_shrunk_bitmap_becomes_a_webp() -> TestResult {
        let mut image = bitmap_image("bmp", 200, 100);
        assert_eq!(image.shrink_to_fit(100, 90)?, Some((100, 50)));
        assert_eq!(image.ext(), "webp");
        assert!(image.bits.is_none());
        Ok(())
    }

    #[test]
    fn a_table_shrinks_what_it_may() -> TestResult {
        let mut vpx = VPX::default();
        vpx.gamedata.image_color_grade = "MyLut".to_string();
        vpx.add_or_replace_image(encoded_image("playfield", "png", 400, 200)?);
        vpx.add_or_replace_image(encoded_image("small", "png", 100, 50)?);
        vpx.add_or_replace_image(encoded_image("MyLUT", "png", 300, 300)?);
        vpx.add_or_replace_image(encoded_image("other_lut", "png", 256, 16)?);
        vpx.add_or_replace_image(encoded_image("dmd_font", "png", 400, 400)?);
        vpx.add_or_replace_image(link_image("linked"));
        let mut unsupported = encoded_image("weird", "png", 400, 400)?;
        unsupported.path = "weird.dds".to_string();
        vpx.add_or_replace_image(unsupported);
        let mut broken = encoded_image("broken", "png", 400, 400)?;
        broken.jpeg.as_mut().expect("has data").data = vec![1, 2, 3];
        vpx.add_or_replace_image(broken);

        let results = vpx.shrink_images(&ShrinkOptions {
            max_dimension: 200,
            exclude: vec!["DMD_Font".to_string()],
            ..Default::default()
        });
        let bytes = |name: &str| {
            vpx.images
                .iter()
                .find(|image| image.name == name)
                .and_then(|image| image.jpeg.as_ref())
                .map(|jpeg| jpeg.data.len())
                .unwrap_or(0)
        };
        let ImageShrink::Shrunk {
            bytes_before,
            bytes_after,
            ..
        } = &results[0]
        else {
            panic!("expected the playfield to shrink, got {}", results[0]);
        };
        assert!(bytes_after < bytes_before);
        assert_eq!(*bytes_after, bytes("playfield"));
        assert_eq!(
            results,
            vec![
                ImageShrink::Shrunk {
                    name: "playfield".to_string(),
                    from: (400, 200),
                    to: (200, 100),
                    bytes_before: *bytes_before,
                    bytes_after: *bytes_after,
                    extension: "png".to_string(),
                },
                ImageShrink::Skipped {
                    name: "MyLUT".to_string(),
                    reason: SkipReason::ColorGradeLut,
                },
                ImageShrink::Skipped {
                    name: "other_lut".to_string(),
                    reason: SkipReason::ColorGradeLut,
                },
                ImageShrink::Skipped {
                    name: "dmd_font".to_string(),
                    reason: SkipReason::Excluded,
                },
                ImageShrink::Skipped {
                    name: "linked".to_string(),
                    reason: SkipReason::Link,
                },
                ImageShrink::Skipped {
                    name: "weird".to_string(),
                    reason: SkipReason::UnsupportedFormat("dds".to_string()),
                },
                ImageShrink::Skipped {
                    name: "broken".to_string(),
                    reason: SkipReason::Undecodable(
                        results
                            .iter()
                            .find_map(|r| match r {
                                ImageShrink::Skipped {
                                    name,
                                    reason: SkipReason::Undecodable(detail),
                                } if name == "broken" => Some(detail.clone()),
                                _ => None,
                            })
                            .unwrap_or_default()
                    ),
                },
            ]
        );
        assert_eq!(
            results[0].to_string(),
            format!(
                "image \"playfield\": 400x200 -> 200x100 png, {bytes_before} -> {bytes_after} bytes"
            )
        );
        assert_eq!(
            results[3].to_string(),
            "image \"dmd_font\" skipped: excluded"
        );
        Ok(())
    }
}
