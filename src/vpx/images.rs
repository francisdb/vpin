//! Bulk changes to the images of a table: converting the deprecated
//! bitmap and png formats to webp.
//!
//! [`VPX::images_to_webp`] works on the parsed table, so it applies to a
//! table however it was loaded, and [`ImageData::to_webp`] does the same
//! for a single image. [`crate::vpx::VpxFile::images_to_webp`] converts in
//! place in a file. vpinball itself suggests the conversion: it stopped
//! writing bitmaps in 10.8.1, and lossless webp stores the same pixels
//! much smaller.

use super::VPX;
use super::image::{ImageData, ImageDataJpeg, vpx_image_to_dynamic_image};
use ::image::codecs::jpeg::JpegEncoder;
use ::image::{DynamicImage, ImageFormat, ImageReader};
use log::warn;
use std::io;

/// An image that was re-encoded as webp, see [`VPX::images_to_webp`]
#[derive(Debug, PartialEq, Clone)]
pub struct ImageToWebpConversion {
    pub name: String,
    pub old_extension: String,
    pub new_extension: String,
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
}
