//! Decoding and re-encoding the images of a table: reading the stored
//! pixels whatever the format, converting the deprecated bitmap format to
//! webp and re-encoding pngs as the smaller webp.
//!
//! vpinball stopped writing bitmaps (`BITS` records, LZW compressed) in
//! 10.8.1; when it loads one it decodes it, drops an all opaque alpha
//! channel and keeps it as a lossless webp, so a table saved by a current
//! vpinball has no bitmaps left. [`ImageData::bitmap_to_webp`] does the
//! same for a single image, [`fix::bitmaps_to_webp`](crate::vpx::fix::bitmaps_to_webp)
//! for every bitmap of a parsed table, so it applies to a table however
//! it was loaded. [`ImageData::png_to_webp`] and
//! [`fix::pngs_to_webp`](crate::vpx::fix::pngs_to_webp) go one step
//! further and re-encode pngs, which vpinball reads as they are, as
//! lossless webp where that is smaller. [`ImageData::tga_to_webp`] and
//! [`fix::tgas_to_webp`](crate::vpx::fix::tgas_to_webp) do the same for
//! tga, found by its tga 2.0 footer since the format has no signature at
//! the start. The `image` crate cannot sniff a tga (it does not read the
//! footer), but vpin holds the whole image in memory and does, then
//! decodes it as a tga explicitly. [`ImageData::decode`] identifies a tga
//! the same way, so every reader of a table sees it.
//! [`crate::vpx::VpxFile::images_to_webp`] does the bitmap and png
//! conversions in place in a file.
//!
//! Neither FlexDMD implementation reads a bitmap image out of a table
//! (they only read the encoded `JPEG` record), so nothing that worked is
//! lost by the conversion.

use super::image::{ImageData, vpx_image_to_dynamic_image};
use super::pinbinary::PinBinary;
use ::image::codecs::jpeg::JpegEncoder;
use ::image::{DynamicImage, ImageFormat, ImageReader};
use std::io;

/// The tga 2.0 footer: the signature, a full stop and a nul byte. It ends
/// a tga file and is the only content signal the format has, since tga has
/// no signature at the start.
const TGA_FOOTER: &[u8] = b"TRUEVISION-XFILE.\0";

/// Whether the encoded bytes are a tga, by its 2.0 footer. The older
/// footerless tga cannot be told apart from arbitrary bytes by content, so
/// it is recognised by its `.tga` extension instead.
fn is_tga(data: &[u8]) -> bool {
    data.ends_with(TGA_FOOTER)
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
            // tga has no leading signature, so the crate's sniffer cannot
            // find it; its 2.0 footer is the content signal, and the `.tga`
            // extension the fallback for the older footerless format
            let guess = if is_tga(&jpeg.data) || self.ext().eq_ignore_ascii_case("tga") {
                reader.set_format(ImageFormat::Tga);
                false
            } else {
                if let Some(format) = ImageFormat::from_extension(self.ext()) {
                    reader.set_format(format);
                }
                true
            };
            // the default limit of 512 MB rejects the 8k float bakes of
            // recent tables
            reader.no_limits();
            if guess {
                reader = reader.with_guessed_format()?;
            }
            return reader
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

    /// Re-encodes a png image as lossless webp when that is smaller, which
    /// it is for most. The png is recognised by its content, not its
    /// extension. Returns `false` when the image is not a png, when its
    /// pixels are more than 8 bits deep, which webp cannot hold, or when
    /// the webp would not be smaller.
    ///
    /// # Errors
    ///
    /// When the png does not decode, or webp cannot encode it, such as a
    /// side over 16383 pixels.
    pub fn png_to_webp(&mut self) -> io::Result<bool> {
        const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
        let Some(bytes_before) = self
            .jpeg
            .as_ref()
            .filter(|jpeg| jpeg.data.starts_with(PNG_SIGNATURE))
            .map(|jpeg| jpeg.data.len())
        else {
            return Ok(false);
        };
        let decoded = self.decode()?;
        if !matches!(
            decoded,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) {
            return Ok(false);
        }
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        if webp.len() >= bytes_before {
            return Ok(false);
        }
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(true)
    }

    /// Re-encodes a tga image as lossless webp when that is smaller, which
    /// it is for the uncompressed and run-length tga vpinball tables carry.
    /// This goes beyond what vpinball does on its own, so it is a size
    /// lever with no audit finding behind it.
    ///
    /// A tga has no signature at the start, but the tga 2.0 format ends
    /// with the TRUEVISION-XFILE footer, so that is its content signal,
    /// checked the same way by [`ImageData::decode`]. Reading the footer is
    /// a tail read, which the `image` crate avoids for streaming callers
    /// but vpin can do because it holds the whole image in memory. The
    /// `.tga` extension is only a fallback for the older footerless format;
    /// an image with neither is left alone, since a footerless tga cannot
    /// be told apart from arbitrary bytes.
    ///
    /// Returns `false` when the image is not a tga, when its pixels are
    /// more than 8 bits deep, which webp cannot hold, or when the webp
    /// would not be smaller.
    ///
    /// # Errors
    ///
    /// When the tga does not decode, or webp cannot encode it.
    pub fn tga_to_webp(&mut self) -> io::Result<bool> {
        let named_tga = self.ext().eq_ignore_ascii_case("tga");
        let Some(bytes_before) = self
            .jpeg
            .as_ref()
            .filter(|jpeg| is_tga(&jpeg.data) || named_tga)
            .map(|jpeg| jpeg.data.len())
        else {
            return Ok(false);
        };
        // decode() identifies the tga the same way, so it decodes as one
        let decoded = self.decode()?;
        if !matches!(
            decoded,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) {
            return Ok(false);
        }
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        if webp.len() >= bytes_before {
            return Ok(false);
        }
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(true)
    }

    /// Replaces the image content; the hash vpinball keeps of the encoded
    /// bytes is dropped since it no longer matches
    fn set_data(&mut self, data: Vec<u8>, extension: &str, width: u32, height: u32) {
        let jpeg = self.jpeg.get_or_insert_with(|| PinBinary {
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
pub(crate) mod tests {
    use super::*;
    use crate::vpx::image::ImageDataBits;
    use crate::vpx::lzw::to_lzw_blocks;
    use ::image::RgbaImage;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    pub(crate) fn pixels(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            ::image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        })
    }

    pub(crate) fn encoded_image(
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
            jpeg: Some(PinBinary {
                path: format!("C:\\images\\{name}.{extension}"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            md5_hash: Some([7; 16]),
            ..Default::default()
        })
    }

    pub(crate) fn bitmap_image(name: &str, width: u32, height: u32) -> ImageData {
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

    pub(crate) fn link_image(name: &str) -> ImageData {
        ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.png"),
            width: 4000,
            height: 4000,
            link: Some(1),
            ..Default::default()
        }
    }

    /// A tga image stored under the given extension. The image crate's tga
    /// encoder writes no footer, so the tga 2.0 footer is appended when
    /// asked, which is how vpinball's tga files (and the survey's) carry
    /// it and how [`ImageData::tga_to_webp`] recognises one.
    pub(crate) fn tga_image(
        name: &str,
        extension: &str,
        width: u32,
        height: u32,
        footer: bool,
    ) -> TestResult<ImageData> {
        let mut data = encode(
            &DynamicImage::ImageRgba8(pixels(width, height)),
            ImageFormat::Tga,
            0,
        )?;
        if footer {
            data.extend_from_slice(b"TRUEVISION-XFILE.\0");
        }
        Ok(ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.{extension}"),
            width,
            height,
            jpeg: Some(PinBinary {
                path: format!("C:\\images\\{name}.{extension}"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            md5_hash: Some([7; 16]),
            ..Default::default()
        })
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

    /// A png stored with the fastest, unfiltered compression, the way a
    /// tool in a hurry writes one
    pub(crate) fn loose_png(name: &str, width: u32, height: u32) -> TestResult<ImageData> {
        use ::image::codecs::png::{CompressionType, FilterType, PngEncoder};
        let mut data = Vec::new();
        pixels(width, height).write_with_encoder(PngEncoder::new_with_quality(
            &mut data,
            CompressionType::Fast,
            FilterType::NoFilter,
        ))?;
        Ok(ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.png"),
            width,
            height,
            jpeg: Some(PinBinary {
                path: format!("C:\\images\\{name}.png"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            md5_hash: Some([7; 16]),
            ..Default::default()
        })
    }

    #[test]
    fn a_png_becomes_a_smaller_lossless_webp() -> TestResult {
        let mut image = loose_png("png", 64, 64)?;
        let before = image.decode()?.to_rgba8();
        let bytes_before = image.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len());
        assert!(image.png_to_webp()?);
        assert_eq!(image.ext(), "webp");
        assert_eq!(image.path, "C:\\images\\png.webp");
        assert!(image.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len()) < bytes_before);
        assert_eq!(image.md5_hash, None);
        assert_eq!(image.decode()?.to_rgba8(), before);
        // a second run has nothing to do
        assert!(!image.png_to_webp()?);
        Ok(())
    }

    #[test]
    fn pngs_webp_cannot_hold_losslessly_are_left_alone() -> TestResult {
        let deep = ::image::ImageBuffer::from_fn(8, 8, |x, y| {
            ::image::Rgb([(x * 4000) as u16, (y * 4000) as u16, 60000])
        });
        let mut data = Vec::new();
        DynamicImage::ImageRgb16(deep)
            .write_to(&mut io::Cursor::new(&mut data), ImageFormat::Png)?;
        let mut image = ImageData {
            name: "deep".to_string(),
            path: "C:\\images\\deep.png".to_string(),
            width: 8,
            height: 8,
            jpeg: Some(PinBinary {
                path: "C:\\images\\deep.png".to_string(),
                name: "deep".to_string(),
                internal_name: None,
                data,
            }),
            ..Default::default()
        };
        assert!(!image.png_to_webp()?);
        assert_eq!(image.ext(), "png");
        Ok(())
    }

    #[test]
    fn only_png_content_is_re_encoded() -> TestResult {
        // a jpeg saved under a png name, which happens
        let mut image = encoded_image("jpg", "jpg", 40, 20)?;
        image.change_extension("png");
        assert!(!image.png_to_webp()?);
        assert!(!bitmap_image("bmp", 8, 8).png_to_webp()?);
        assert!(!link_image("link").png_to_webp()?);
        Ok(())
    }

    #[test]
    fn a_tga_becomes_a_smaller_lossless_webp() -> TestResult {
        let mut image = tga_image("tga", "tga", 64, 64, true)?;
        let before = image.decode()?.to_rgba8();
        let bytes_before = image.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len());
        assert!(image.tga_to_webp()?);
        assert_eq!(image.ext(), "webp");
        assert_eq!(image.path, "C:\\images\\tga.webp");
        assert!(image.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len()) < bytes_before);
        assert_eq!(image.md5_hash, None);
        assert_eq!(image.decode()?.to_rgba8(), before);
        // a second run has nothing to do
        assert!(!image.tga_to_webp()?);
        Ok(())
    }

    #[test]
    fn a_tga_is_recognised_by_its_footer_whatever_the_extension() -> TestResult {
        // content decides, like png: the footer identifies a tga even when
        // the name says otherwise
        let mut image = tga_image("footered", "png", 64, 64, true)?;
        assert!(image.tga_to_webp()?);
        assert_eq!(image.ext(), "webp");
        Ok(())
    }

    #[test]
    fn a_tga_extension_is_the_fallback_when_there_is_no_footer() -> TestResult {
        // the older footerless tga has no content signal, so the `.tga`
        // extension is the fallback that identifies it
        let mut image = tga_image("art", "tga", 64, 64, false)?;
        assert!(image.tga_to_webp()?);
        assert_eq!(image.ext(), "webp");
        Ok(())
    }

    #[test]
    fn a_tga_that_is_neither_named_nor_footered_is_left_alone() -> TestResult {
        // no .tga name and no footer: it cannot be told apart from
        // arbitrary bytes, so it is not touched
        let mut image = tga_image("unmarked", "png", 64, 64, false)?;
        assert!(!image.tga_to_webp()?);
        assert_eq!(image.ext(), "png");
        assert_eq!(image.md5_hash, Some([7; 16]));
        Ok(())
    }

    #[test]
    fn only_tga_content_is_re_encoded() -> TestResult {
        // a png and a bitmap are neither named .tga nor carry the footer
        assert!(!loose_png("png", 40, 20)?.tga_to_webp()?);
        assert!(!bitmap_image("bmp", 8, 8).tga_to_webp()?);
        assert!(!link_image("link").tga_to_webp()?);
        Ok(())
    }
}
