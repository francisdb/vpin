use super::biff::{self, BiffReader, BiffWriter};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt;

// TODO comment here a vpx file that contains font data

#[derive(PartialEq)]
pub struct FontData {
    pub name: String,
    pub path: String, // patho of original file for easy re-importing
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontDataJson {
    name: String,
    path: String,
}

impl FontDataJson {
    pub fn from_font_data(font_data: &FontData) -> Self {
        Self {
            name: font_data.name.clone(),
            path: font_data.path.clone(),
        }
    }
    pub fn to_font_data(&self) -> FontData {
        FontData {
            name: self.name.clone(),
            path: self.path.clone(),
            data: vec![],
        }
    }
}

impl fmt::Debug for FontData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("FontData")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("data", &format!("<{} bytes>", self.data.len()))
            .finish()
    }
}

impl FontData {
    pub(crate) fn ext(&self) -> String {
        // TODO we might want to also check the jpeg fsPath
        match self.path.split('.').next_back() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }
}

pub fn read(input: &[u8]) -> FontData {
    let mut reader = BiffReader::new(input);
    let mut name: String = "".to_string();
    let mut path: String = "".to_string();
    let mut size_opt: Option<u32> = None;
    let mut data: Vec<u8> = vec![];
    loop {
        reader.next(biff::WARN);
        if reader.is_eof() {
            break;
        }
        let tag = reader.tag();
        let tag_str = tag.as_str();
        match tag_str {
            "NAME" => {
                name = reader.get_string();
            }
            "PATH" => {
                path = reader.get_string();
            }
            "SIZE" => {
                size_opt = Some((reader.get_u32()).to_owned());
            }
            "DATA" => match size_opt {
                Some(size) => {
                    let d = reader.get_data(size.try_into().unwrap());
                    d.clone_into(&mut data);
                }
                None => {
                    panic!("DATA tag without SIZE tag");
                }
            },
            _ => {
                warn!("Skipping font tag: {tag_str}");
                reader.skip_tag();
            }
        }
    }
    FontData { name, path, data }
}

pub fn write(font_data: &FontData) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    writer.write_tagged_string("NAME", &font_data.name);
    writer.write_tagged_string("PATH", &font_data.path);
    writer.write_tagged_u32("SIZE", font_data.data.len().try_into().unwrap());
    writer.write_tagged_data("DATA", &font_data.data);
    writer.close(true);
    writer.get_data().to_owned()
}

#[test]
fn read_write() {
    use pretty_assertions::assert_eq;

    let font = FontData {
        name: "test_name".to_string(),
        path: "/tmp/test".to_string(),
        data: vec![1, 2, 3, 4],
    };
    let bytes = write(&font);
    let font_read = read(&bytes);

    assert_eq!(font, font_read);
}
