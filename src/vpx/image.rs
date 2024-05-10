use serde::{Deserialize, Serialize};
use std::fmt;

use super::biff::{self, BiffRead, BiffReader, BiffWrite, BiffWriter};

#[derive(PartialEq)]
pub struct ImageDataJpeg {
    pub path: String,
    pub name: String,
    // /**
    //  * Lowercased name?
    //  * No longer in use
    //  */
    pub internal_name: Option<String>,
    // alpha_test_value: f32,
    pub data: Vec<u8>,
}

impl fmt::Debug for ImageDataJpeg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("ImageDataJpeg")
            .field("path", &self.path)
            .field("name", &self.name)
            // .field("alpha_test_value", &self.alpha_test_value)
            .field("data", &self.data.len())
            .finish()
    }
}

/**
 * A bitmap blob, typically used by textures.
 */
#[derive(PartialEq)]
pub struct ImageDataBits {
    /// Lzw compressed raw BMP 32-bit sBGRA bitmap data
    /// However we expect the alpha channel to always be 255
    pub lzw_compressed_data: Vec<u8>,
}

impl fmt::Debug for ImageDataBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("ImageDataBits")
            .field("data", &self.lzw_compressed_data.len())
            .finish()
    }
}

#[derive(PartialEq, Debug)]
pub struct ImageData {
    pub name: String, // NAME
    // /**
    //  * Lowercased name?
    //  * INME
    //  * No longer in use
    //  */
    pub internal_name: Option<String>,
    pub path: String, // PATH
    pub width: u32,   // WDTH
    pub height: u32,  // HGHT
    // TODO seems to be 1 for some kind of link type img, related to screenshots.
    // we only see this where a screenshot is set on the table info.
    // https://github.com/vpinball/vpinball/blob/1a70aa35eb57ec7b5fbbb9727f6735e8ef3183e0/Texture.cpp#L588
    pub link: Option<u32>, // LINK
    /// ALTV
    /// Alpha test value, used for transparency
    /// Used to default to 1.0, now defaults to -1.0 since 10.8
    pub alpha_test_value: f32, // ALTV
    pub is_opaque: Option<bool>, // OPAQ (added in 10.8)
    pub is_signed: Option<bool>, // SIGN (added in 10.8)
    // TODO we can probably only have one of these so we can make an enum
    pub jpeg: Option<ImageDataJpeg>,
    pub bits: Option<ImageDataBits>,
}

impl ImageData {
    const ALPHA_TEST_VALUE_DEFAULT: f32 = -1.0;

    pub fn is_link(&self) -> bool {
        self.link == Some(1)
    }

    pub(crate) fn ext(&self) -> String {
        // TODO we might want to also check the jpeg fsPath
        match self.path.split('.').last() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub(crate) struct ImageDataJson {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    internal_name: Option<String>,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_test_value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_opaque: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_signed: Option<bool>,

    // these are just for full compatibility with the original file
    #[serde(skip_serializing_if = "Option::is_none")]
    jpeg_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jpeg_internal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jpeg_path: Option<String>,

    // in case we have a duplicate name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name_dedup: Option<String>,
}

impl ImageDataJson {
    pub fn from_image_data(image_data: &ImageData) -> Self {
        let (jpeg_name, jpeg_path, jpeg_internal_name) = if let Some(jpeg) = &image_data.jpeg {
            let jpeg_name = if jpeg.name == image_data.name {
                None
            } else {
                Some(jpeg.name.clone())
            };
            let jpeg_path = if jpeg.path == image_data.path {
                None
            } else {
                Some(jpeg.path.clone())
            };
            let jpeg_internal_name = jpeg.internal_name.clone();
            (jpeg_name, jpeg_path, jpeg_internal_name)
        } else {
            (None, None, None)
        };

        // TODO we might want to generate a warning if the alpha_test_value is the old default of 1.0
        //   which caused overhead in the shader
        let alpha_test_value = if image_data.alpha_test_value == ImageData::ALPHA_TEST_VALUE_DEFAULT
        {
            None
        } else {
            Some(image_data.alpha_test_value)
        };

        ImageDataJson {
            name: image_data.name.clone(),
            internal_name: image_data.internal_name.clone(),
            path: image_data.path.clone(),
            width: None,  // will be set later if needed
            height: None, // will be set later if needed
            link: image_data.link,
            alpha_test_value,
            is_opaque: image_data.is_opaque,
            is_signed: image_data.is_signed,
            jpeg_name,
            jpeg_internal_name,
            jpeg_path,
            name_dedup: None,
        }
    }

    pub fn to_image_data(&self, width: u32, height: u32, bits: Option<ImageDataBits>) -> ImageData {
        let mut jpeg = None;
        if !self.is_bmp() && !self.is_link() {
            let name = match &self.jpeg_name {
                Some(name) => name.clone(),
                None => self.name.clone(),
            };
            let path = match &self.jpeg_path {
                Some(path) => path.clone(),
                None => self.path.clone(),
            };
            let internal_name = self.jpeg_internal_name.clone();

            jpeg = Some(ImageDataJpeg {
                path,
                name,
                internal_name,
                data: vec![], // populated later
            });
        }

        let alpha_test_value = self
            .alpha_test_value
            .unwrap_or(ImageData::ALPHA_TEST_VALUE_DEFAULT);
        ImageData {
            name: self.name.clone(),
            internal_name: self.internal_name.clone(),
            path: self.path.clone(),
            width,
            height,
            link: self.link,
            alpha_test_value,
            is_opaque: self.is_opaque,
            is_signed: self.is_signed,
            jpeg,
            bits,
        }
    }

    pub fn is_link(&self) -> bool {
        self.link == Some(1)
    }

    pub(crate) fn ext(&self) -> String {
        // TODO we might want to also check the jpeg fsPath
        match self.path.split('.').last() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }

    pub(crate) fn is_bmp(&self) -> bool {
        self.ext().to_ascii_lowercase() == "bmp"
    }
}

impl BiffWrite for ImageData {
    fn biff_write(&self, writer: &mut BiffWriter) {
        write(self, writer);
    }
}

impl BiffRead for ImageData {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        read(reader)
    }
}

impl Default for ImageData {
    fn default() -> Self {
        ImageData {
            name: "".to_string(),
            internal_name: None,
            path: "".to_string(),
            width: 0,
            height: 0,
            link: None,
            alpha_test_value: 0.0,
            is_opaque: None,
            is_signed: None,
            jpeg: None,
            bits: None,
        }
    }
}

fn read(reader: &mut BiffReader) -> ImageData {
    let mut image_data = ImageData::default();
    loop {
        reader.next(biff::WARN);
        if reader.is_eof() {
            break;
        }
        let tag = reader.tag();
        let tag_str = tag.as_str();
        match tag_str {
            "NAME" => {
                image_data.name = reader.get_string();
            }
            "PATH" => {
                image_data.path = reader.get_string();
            }
            "INME" => {
                image_data.internal_name = Some(reader.get_string());
            }
            "WDTH" => {
                image_data.width = reader.get_u32();
            }
            "HGHT" => {
                image_data.height = reader.get_u32();
            }
            "ALTV" => {
                image_data.alpha_test_value = reader.get_f32();
            }
            "OPAQ" => {
                image_data.is_opaque = Some(reader.get_bool());
            }
            "SIGN" => {
                image_data.is_signed = Some(reader.get_bool());
            }
            "BITS" => {
                // these have zero as length
                // read all the data until the next expected tag
                let data = reader.data_until("ALTV".as_bytes());
                //let reader = std::io::Cursor::new(data);

                // uncompressed = zlib.decompress(image_data.data[image_data.pos:]) #, wbits=9)
                // reader.skip_end_tag(len.try_into().unwrap());
                image_data.bits = Some(ImageDataBits {
                    lzw_compressed_data: data,
                });
            }
            "JPEG" => {
                // these have zero as length
                // Strangely, raw data are pushed outside the JPEG tag (breaking the BIFF structure of the file)
                let mut sub_reader = reader.child_reader();
                let jpeg_data = read_jpeg(&mut sub_reader);
                image_data.jpeg = Some(jpeg_data);
                let pos = sub_reader.pos();
                reader.skip_end_tag(pos);
            }
            "LINK" => {
                // TODO seems to be 1 for some kind of link type img, related to screenshots.
                // we only see this where a screenshot is set on the table info.
                // https://github.com/vpinball/vpinball/blob/1a70aa35eb57ec7b5fbbb9727f6735e8ef3183e0/Texture.cpp#L588
                image_data.link = Some(reader.get_u32());
            }
            _ => {
                println!("Skipping image tag: {}", tag);
                reader.skip_tag();
            }
        }
    }
    image_data
}

fn write(data: &ImageData, writer: &mut BiffWriter) {
    writer.write_tagged_string("NAME", &data.name);
    if let Some(inme) = &data.internal_name {
        writer.write_tagged_string("INME", inme);
    }
    writer.write_tagged_string("PATH", &data.path);
    writer.write_tagged_u32("WDTH", data.width);
    writer.write_tagged_u32("HGHT", data.height);
    if let Some(link) = data.link {
        writer.write_tagged_u32("LINK", link);
    }
    if let Some(bits) = &data.bits {
        writer.write_tagged_data_without_size("BITS", &bits.lzw_compressed_data);
    }
    if let Some(jpeg) = &data.jpeg {
        let bits = write_jpg(jpeg);
        writer.write_tagged_data_without_size("JPEG", &bits);
    }
    writer.write_tagged_f32("ALTV", data.alpha_test_value);
    if let Some(is_opaque) = data.is_opaque {
        writer.write_tagged_bool("OPAQ", is_opaque);
    }
    if let Some(is_signed) = data.is_signed {
        writer.write_tagged_bool("SIGN", is_signed);
    }
    writer.close(true);
}

fn read_jpeg(reader: &mut BiffReader) -> ImageDataJpeg {
    // I do wonder why all the tags are duplicated here
    let mut size_opt: Option<u32> = None;
    let mut path: String = "".to_string();
    let mut name: String = "".to_string();
    let mut data: Vec<u8> = vec![];
    // let mut alpha_test_value: f32 = 0.0;
    let mut internal_name: Option<String> = None;
    loop {
        reader.next(biff::WARN);
        if reader.is_eof() {
            break;
        }
        let tag = reader.tag();
        let tag_str = tag.as_str();
        match tag_str {
            "SIZE" => {
                size_opt = Some(reader.get_u32());
            }
            "DATA" => match size_opt {
                Some(size) => data = reader.get_data(size.try_into().unwrap()).to_vec(),
                None => {
                    panic!("DATA tag without SIZE tag");
                }
            },
            "NAME" => name = reader.get_string(),
            "PATH" => path = reader.get_string(),
            // "ALTV" => alpha_test_value = reader.get_f32(), // TODO why are these duplicated?
            "INME" => internal_name = Some(reader.get_string()),
            _ => {
                // skip this record
                println!("skipping tag inside JPEG {}", tag);
                reader.skip_tag();
            }
        }
    }
    let data = data.to_vec();
    ImageDataJpeg {
        path,
        name,
        internal_name,
        // alpha_test_value,
        data,
    }
}

fn write_jpg(img: &ImageDataJpeg) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    writer.write_tagged_string("NAME", &img.name);
    if let Some(inme) = &img.internal_name {
        writer.write_tagged_string("INME", inme);
    }
    writer.write_tagged_string("PATH", &img.path);
    writer.write_tagged_u32("SIZE", img.data.len().try_into().unwrap());
    writer.write_tagged_data("DATA", &img.data);
    // writer.write_tagged_f32("ALTV", img.alpha_test_value);
    writer.close(true);
    writer.get_data().to_vec()
}

#[cfg(test)]
mod test {

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read_jpeg() {
        let img = ImageDataJpeg {
            path: "path_value".to_string(),
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            // alpha_test_value: 1.0,
            data: vec![1, 2, 3],
        };

        let bytes = write_jpg(&img);

        let read = read_jpeg(&mut BiffReader::new(&bytes));

        assert_eq!(read, img);
    }

    #[test]
    fn test_write_jpeg_should_have_tag_size_zero() {
        let image: ImageData = ImageData {
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            path: "path_value".to_string(),
            width: 1,
            height: 2,
            link: None,
            alpha_test_value: 1.0,
            is_opaque: Some(true),
            is_signed: Some(false),
            jpeg: Some(ImageDataJpeg {
                path: "path_value".to_string(),
                name: "name_value".to_string(),
                internal_name: Some("inme_value".to_string()),
                // alpha_test_value: 1.0,
                data: vec![1, 2, 3],
            }),
            bits: None,
        };

        let mut writer = BiffWriter::new();
        ImageData::biff_write(&image, &mut writer);
        let data = writer.get_data();
        let mut reader = BiffReader::new(data);
        reader.next(false); // NAME
        reader.next(false); // INME
        reader.next(false); // PATH
        reader.next(false); // WDTH
        reader.next(false); // HGHT
        reader.next(false); // LINK
        assert_eq!(reader.tag().as_str(), "JPEG");
        assert_eq!(reader.remaining_in_record(), 0);
    }

    #[test]
    fn test_write_read() {
        let image: ImageData = ImageData {
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            path: "path_value".to_string(),
            width: 1,
            height: 2,
            link: None,
            alpha_test_value: 1.0,
            is_opaque: Some(true),
            is_signed: Some(false),
            jpeg: Some(ImageDataJpeg {
                path: "path_value".to_string(),
                name: "name_value".to_string(),
                internal_name: Some("inme_value".to_string()),
                // alpha_test_value: 1.0,
                data: vec![1, 2, 3],
            }),
            bits: None,
        };
        let mut writer = BiffWriter::new();
        ImageData::biff_write(&image, &mut writer);
        let image_read = read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(image, image_read);
    }

    #[test]
    fn test_write_read_json() {
        let image: ImageData = ImageData {
            name: "name_value".to_string(),
            internal_name: Some("inme_value".to_string()),
            path: "path_value".to_string(),
            width: 1,
            height: 2,
            link: None,
            alpha_test_value: 1.0,
            is_opaque: Some(true),
            is_signed: Some(false),
            jpeg: Some(ImageDataJpeg {
                path: "path_value".to_string(),
                name: "name_value".to_string(),
                internal_name: Some("inme_value".to_string()),
                // alpha_test_value: 1.0,
                data: vec![1, 2, 3],
            }),
            bits: None,
        };
        let image_json = ImageDataJson::from_image_data(&image);
        let mut image_read = image_json.to_image_data(1, 2, None);
        // these are populated later whe reading the actual images from the file
        if let Some(jpeg) = &mut image_read.jpeg {
            jpeg.data = vec![1, 2, 3];
        }
        image_read.width = 1;
        image_read.height = 2;
        assert_eq!(image, image_read);
    }
}
