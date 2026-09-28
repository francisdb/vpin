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

/// The format the content of an encoded image names, by its signature, as
/// the short lower case name the audit shows: `png`, `jpeg`, `gif`, `bmp`,
/// `webp`, `hdr`, `exr` and `tga` (by its 2.0 footer), which vpin decodes,
/// and `psd`, `tiff` and `dds`, which vpinball reads through FreeImage but
/// vpin has no decoder for. `None` when no signature matches, which is
/// what a footerless tga looks like.
pub(crate) fn content_format(data: &[u8]) -> Option<&'static str> {
    let format = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if data.starts_with(b"\xFF\xD8\xFF") {
        "jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "gif"
    } else if data.starts_with(b"BM") {
        "bmp"
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if data.starts_with(b"#?RADIANCE") || data.starts_with(b"#?RGBE") {
        "hdr"
    } else if data.starts_with(b"\x76\x2f\x31\x01") {
        "exr"
    } else if data.starts_with(b"8BPS") {
        "psd"
    } else if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        "tiff"
    } else if data.starts_with(b"DDS ") {
        "dds"
    } else if is_tga(data) {
        "tga"
    } else {
        return None;
    };
    Some(format)
}

/// Whether a webp holds lossy pixels: its bitstream is a `VP8 ` chunk
/// rather than the lossless `VP8L`. An extended webp (`VP8X`) is judged by
/// the bitstream chunk that follows its header. Bytes that are not a webp
/// are not lossy
pub(crate) fn webp_is_lossy(data: &[u8]) -> bool {
    if !(data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP")) {
        return false;
    }
    // chunks: a four character code, a little endian size and the
    // payload, padded to an even length
    let mut chunks = data.get(12..).unwrap_or_default();
    while let Some(header) = chunks.get(..8) {
        match &header[..4] {
            b"VP8 " => return true,
            b"VP8L" => return false,
            _ => {}
        }
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        chunks = chunks.get(8 + size + (size & 1)..).unwrap_or_default();
    }
    false
}

/// The format an image's file extension names, in the same short names as
/// [`content_format`]: `jpg` and `jfif` are `jpeg`, `tif` is `tiff`. `None`
/// for an extension that names no image format.
pub(crate) fn extension_format(extension: &str) -> Option<&'static str> {
    let format = match extension.to_ascii_lowercase().as_str() {
        "png" => "png",
        "jpg" | "jpeg" | "jfif" => "jpeg",
        "gif" => "gif",
        "bmp" => "bmp",
        "webp" => "webp",
        "hdr" => "hdr",
        "exr" => "exr",
        "psd" => "psd",
        "tif" | "tiff" => "tiff",
        "dds" => "dds",
        "tga" => "tga",
        _ => return None,
    };
    Some(format)
}

/// Whether vpin has a decoder for a format named by [`content_format`]
pub(crate) fn decodable(format: &str) -> bool {
    !matches!(format, "psd" | "tiff" | "dds")
}

/// The size of an encoded picture held outside an image, the table
/// screenshot for one, read from its header the way
/// [`ImageData::dimensions`] reads it. `None` when the header does not
/// parse or the format is not known
pub(crate) fn header_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut reader = ImageReader::new(io::Cursor::new(data));
    reader.no_limits();
    reader.with_guessed_format().ok()?.into_dimensions().ok()
}

/// What a webp conversion did with an image it was asked to convert
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Webp {
    /// The image is now a webp
    Converted,
    /// The webp would not be smaller than what is stored
    NotSmaller,
    /// The pixels are deeper than 8 bits, which webp cannot hold
    TooDeep,
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
        if self.jpeg.is_some() {
            return self
                .encoded_reader()?
                .decode()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the image has no data in the table",
        ))
    }

    /// The size of the encoded picture, read from its header without
    /// decoding the pixels, with the format chosen the way [`ImageData::decode`]
    /// chooses it
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::NotFound`] for a link or a bitmap, which have no
    /// encoded data, [`io::ErrorKind::InvalidData`] when the header does
    /// not parse.
    pub fn dimensions(&self) -> io::Result<(u32, u32)> {
        self.encoded_reader()?
            .into_dimensions()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    /// A reader over the encoded data with its format chosen: the content
    /// decides, the extension is the fallback for formats without a
    /// signature such as tga and hdr. tga has no leading signature, so the
    /// crate's sniffer cannot find it; its 2.0 footer is the content
    /// signal, and the `.tga` extension the fallback for the older
    /// footerless format. The sniffer still runs after that: it keeps the
    /// tga format when it finds no signature, and a png stored under a
    /// `.tga` name still decodes as the png it is.
    fn encoded_reader(&self) -> io::Result<ImageReader<io::Cursor<&[u8]>>> {
        let Some(jpeg) = &self.jpeg else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the image has no encoded data in the table",
            ));
        };
        let mut reader = ImageReader::new(io::Cursor::new(jpeg.data.as_slice()));
        if is_tga(&jpeg.data) || self.ext().eq_ignore_ascii_case("tga") {
            reader.set_format(ImageFormat::Tga);
        } else if let Some(format) = ImageFormat::from_extension(self.ext()) {
            reader.set_format(format);
        }
        // the default limit of 512 MB rejects the 8k float bakes of
        // recent tables
        reader.no_limits();
        reader.with_guessed_format()
    }

    /// Whether the stored data is a png, by its signature
    pub(crate) fn is_stored_png(&self) -> bool {
        const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
        self.jpeg
            .as_ref()
            .is_some_and(|jpeg| jpeg.data.starts_with(PNG_SIGNATURE))
    }

    /// Whether the stored data is a tga: by its 2.0 footer, or by a `.tga`
    /// name for the older footerless format
    pub(crate) fn is_stored_tga(&self) -> bool {
        self.jpeg
            .as_ref()
            .is_some_and(|jpeg| is_tga(&jpeg.data) || self.ext().eq_ignore_ascii_case("tga"))
    }

    /// Re-encodes a bitmap image as lossless webp, the way vpinball does
    /// when it loads one. Returns `false` when the image is not a bitmap.
    ///
    /// # Errors
    ///
    /// When the bitmap data does not decode.
    pub fn bitmap_to_webp(&mut self) -> io::Result<bool> {
        Ok(matches!(self.bitmap_webp()?, Some(Webp::Converted)))
    }

    /// [`ImageData::bitmap_to_webp`] telling why it left the image alone:
    /// `None` when the image is not a bitmap
    pub(crate) fn bitmap_webp(&mut self) -> io::Result<Option<Webp>> {
        if self.bits.is_none() {
            return Ok(None);
        }
        let decoded = self.decode()?;
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(Some(Webp::Converted))
    }

    /// Re-encodes an image that is stored as a file (not a bitmap) as
    /// lossless webp when its pixels are 8 bits deep and the webp is
    /// smaller than the stored bytes
    fn file_webp(&mut self, bytes_before: usize) -> io::Result<Webp> {
        let decoded = self.decode()?;
        if !matches!(
            decoded,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) {
            return Ok(Webp::TooDeep);
        }
        let webp = encode(&decoded, ImageFormat::WebP, 0)?;
        if webp.len() >= bytes_before {
            return Ok(Webp::NotSmaller);
        }
        self.set_data(webp, "webp", decoded.width(), decoded.height());
        Ok(Webp::Converted)
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
        Ok(matches!(self.png_webp()?, Some(Webp::Converted)))
    }

    /// [`ImageData::png_to_webp`] telling why it left the image alone:
    /// `None` when the image is not a png
    pub(crate) fn png_webp(&mut self) -> io::Result<Option<Webp>> {
        if !self.is_stored_png() {
            return Ok(None);
        }
        let bytes_before = self.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len());
        self.file_webp(bytes_before).map(Some)
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
        Ok(matches!(self.tga_webp()?, Some(Webp::Converted)))
    }

    /// [`ImageData::tga_to_webp`] telling why it left the image alone:
    /// `None` when the image is not a tga
    pub(crate) fn tga_webp(&mut self) -> io::Result<Option<Webp>> {
        if !self.is_stored_tga() {
            return Ok(None);
        }
        // decode() identifies the tga the same way, so it decodes as one
        let bytes_before = self.jpeg.as_ref().map_or(0, |jpeg| jpeg.data.len());
        self.file_webp(bytes_before).map(Some)
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
    fn content_is_named_by_its_signature() {
        assert_eq!(content_format(b"\x89PNG\r\n\x1a\n...."), Some("png"));
        assert_eq!(content_format(b"\xFF\xD8\xFF\xE0...."), Some("jpeg"));
        assert_eq!(content_format(b"GIF89a...."), Some("gif"));
        assert_eq!(content_format(b"RIFF....WEBPVP8 "), Some("webp"));
        assert_eq!(content_format(b"#?RADIANCE\n"), Some("hdr"));
        assert_eq!(content_format(b"8BPS\0\x01"), Some("psd"));
        assert_eq!(content_format(b"II*\0...."), Some("tiff"));
        assert_eq!(content_format(b"DDS |...."), Some("dds"));
        assert_eq!(content_format(b"....TRUEVISION-XFILE.\0"), Some("tga"));
        assert_eq!(content_format(b"AAAA"), None);
        assert_eq!(content_format(b""), None);
        assert!(!decodable("psd"));
        assert!(decodable("png"));
    }

    #[test]
    fn extensions_name_the_same_formats() {
        assert_eq!(extension_format("PNG"), Some("png"));
        assert_eq!(extension_format("jpg"), Some("jpeg"));
        assert_eq!(extension_format("jfif"), Some("jpeg"));
        assert_eq!(extension_format("tif"), Some("tiff"));
        assert_eq!(extension_format("tga"), Some("tga"));
        assert_eq!(extension_format("bin"), None);
    }

    #[test]
    fn dimensions_come_from_the_header() -> TestResult {
        assert_eq!(loose_png("png", 40, 20)?.dimensions()?, (40, 20));
        assert_eq!(tga_image("art", "png", 8, 4, true)?.dimensions()?, (8, 4));
        assert_eq!(
            link_image("link").dimensions().unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            bitmap_image("bmp", 8, 8).dimensions().unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        Ok(())
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

    /// A 16 bit png, which webp cannot hold
    pub(crate) fn deep_png(name: &str) -> TestResult<ImageData> {
        let deep = ::image::ImageBuffer::from_fn(8, 8, |x, y| {
            ::image::Rgb([(x * 4000) as u16, (y * 4000) as u16, 60000])
        });
        let mut data = Vec::new();
        DynamicImage::ImageRgb16(deep)
            .write_to(&mut io::Cursor::new(&mut data), ImageFormat::Png)?;
        Ok(ImageData {
            name: name.to_string(),
            path: format!("C:\\images\\{name}.png"),
            width: 8,
            height: 8,
            jpeg: Some(PinBinary {
                path: format!("C:\\images\\{name}.png"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            ..Default::default()
        })
    }

    /// A png of noise, which no lossless encoder makes smaller
    pub(crate) fn noise_png(name: &str, width: u32, height: u32) -> TestResult<ImageData> {
        let mut state: u32 = 0x9E37_79B9;
        let noise = ::image::ImageBuffer::from_fn(width, height, |_, _| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            };
            ::image::Rgb([next(), next(), next()])
        });
        let mut data = Vec::new();
        DynamicImage::ImageRgb8(noise)
            .write_to(&mut io::Cursor::new(&mut data), ImageFormat::Png)?;
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
        let mut image = deep_png("deep")?;
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
    fn a_png_stored_under_a_tga_name_still_decodes_as_a_png() -> TestResult {
        // the `.tga` extension only selects the tga decoder when the
        // sniffer finds no signature; a png signature still wins
        let mut image = loose_png("misnamed", 40, 20)?;
        image.path = "C:\\images\\misnamed.tga".to_string();
        assert_eq!(image.ext(), "tga");
        let decoded = image.decode()?;
        assert_eq!((decoded.width(), decoded.height()), (40, 20));
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

    #[test]
    fn a_webp_is_lossy_by_its_bitstream_chunk() -> TestResult {
        assert!(webp_is_lossy(b"RIFF\x10\0\0\0WEBPVP8 \x04\0\0\0abcd"));
        assert!(!webp_is_lossy(b"RIFF\x10\0\0\0WEBPVP8L\x04\0\0\0abcd"));
        // extended: the VP8X chunk first, the bitstream after it
        assert!(webp_is_lossy(
            b"RIFF\x20\0\0\0WEBPVP8X\x0a\0\0\0abcdefghijVP8 \x04\0\0\0abcd"
        ));
        assert!(!webp_is_lossy(
            b"RIFF\x20\0\0\0WEBPVP8X\x0a\0\0\0abcdefghijVP8L\x04\0\0\0abcd"
        ));
        // what the image crate writes is lossless, and a png is no webp
        let image = encoded_image("art", "webp", 8, 8)?;
        assert!(!webp_is_lossy(
            &image.jpeg.as_ref().map_or(Vec::new(), |j| j.data.clone())
        ));
        assert!(!webp_is_lossy(b"\x89PNG\r\n\x1a\n"));
        Ok(())
    }
}
