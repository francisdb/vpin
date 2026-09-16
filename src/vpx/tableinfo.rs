use cfb::CompoundFile;
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{MAIN_SEPARATOR_STR, Path};

use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::json::infallible_to_value;
use crate::vpx::utf16::{decode_utf16le, encode_utf16le};
use serde::{Deserialize, Serialize};

// >    "/TableInfo/AuthorName",
// >    "/TableInfo/Screenshot",
// >    "/TableInfo/TableBlurb",
// >    "/TableInfo/TableRules",
// >    "/TableInfo/AuthorEmail",
// >    "/TableInfo/ReleaseDate",

/// The table metadata from the `TableInfo` storage, mirroring the info
/// fields of vpinball's `PinTable`.
///
/// vpinball writes each property as its own stream in the `TableInfo`
/// storage, as UTF-16LE text without a terminator, and does not write a
/// stream for an empty value (`PinTable::SaveInfo` and `PinTable::LoadInfo`
/// in `src/parts/pintable.cpp`). Every field is `None` when its stream is
/// absent, which for a vpinball-written file means the value was empty.
/// All of them are free text entered in the table info dialog unless
/// noted otherwise.
#[derive(PartialEq, Debug)]
pub struct TableInfo {
    /// Name of the table (`PinTable::m_tableName`).
    ///
    /// Stream `TableInfo/TableName`
    pub table_name: Option<String>,
    /// Author of the table (`PinTable::m_author`).
    ///
    /// Stream `TableInfo/AuthorName`
    pub author_name: Option<String>,
    /// The table screenshot as the bytes of its original image file.
    ///
    /// vpinball writes the file of the image the author picked as
    /// screenshot (`PinTable::m_screenShot`, `Texture::GetFileRaw`), so the
    /// format is whatever that image was imported in. On load the image
    /// with [`ImageData::link`](crate::vpx::image::ImageData::link) set
    /// takes its pixels from here.
    ///
    /// Stream `TableInfo/Screenshot`
    pub screenshot: Option<Vec<u8>>,
    /// Short description of the table (`PinTable::m_blurb`).
    ///
    /// Stream `TableInfo/TableBlurb`
    pub table_blurb: Option<String>,
    /// Rules of the game (`PinTable::m_rules`).
    ///
    /// Stream `TableInfo/TableRules`
    pub table_rules: Option<String>,
    /// Email address of the author (`PinTable::m_authorEMail`).
    ///
    /// Stream `TableInfo/AuthorEmail`
    pub author_email: Option<String>,
    /// Release date of the table (`PinTable::m_releaseDate`), in whatever
    /// form the author typed it.
    ///
    /// Stream `TableInfo/ReleaseDate`
    pub release_date: Option<String>,
    /// Number of times vpinball has saved the table, as a decimal string
    /// (`PinTable::m_numTimesSaved`). vpinball increments it on every save
    /// and leaves it out of the table hash.
    ///
    /// Stream `TableInfo/TableSaveRev`
    pub table_save_rev: Option<String>,
    /// Version of the table as given by the author (`PinTable::m_version`).
    /// vpinball also records it in its settings as the last played version
    /// of the table.
    ///
    /// Stream `TableInfo/TableVersion`
    pub table_version: Option<String>,
    /// Website of the author (`PinTable::m_webSite`).
    ///
    /// Stream `TableInfo/AuthorWebSite`
    pub author_website: Option<String>,
    /// Local date and time of the last save by vpinball
    /// (`PinTable::m_dateSaved`), in C `asctime` form such as
    /// `Wed Sep 16 14:03:52 2026`. Written on every save and left out of
    /// the table hash.
    ///
    /// Stream `TableInfo/TableSaveDate`
    pub table_save_date: Option<String>,
    /// Long description of the table (`PinTable::m_description`).
    ///
    /// Stream `TableInfo/TableDescription`
    pub table_description: Option<String>,
    /// The custom info properties, keyed by name. The names and their
    /// order are stored separately in the `GameStg/CustomInfoTags` stream,
    /// see [`CustomInfoTags`];
    /// this library also puts any unknown stream of the storage here.
    ///
    /// Streams `TableInfo/<name>`
    pub properties: HashMap<String, String>,
}

impl TableInfo {
    pub(crate) fn new() -> TableInfo {
        // current data as ISO string
        //let now: String = chrono::Local::now().to_rfc3339();
        TableInfo {
            table_name: None,
            author_name: None,
            screenshot: None,
            table_blurb: None,
            table_rules: None,
            author_email: None,
            release_date: None,
            table_save_rev: None, // added in ?
            table_version: None,
            author_website: None,
            table_save_date: None, // added in ?
            table_description: None,
            properties: HashMap::new(),
        }
    }
}

impl Default for TableInfo {
    fn default() -> Self {
        TableInfo::new()
    }
}

pub(crate) fn write_tableinfo<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    table_info: &TableInfo,
) -> std::io::Result<()> {
    let table_info_path = Path::new(MAIN_SEPARATOR_STR).join("TableInfo");
    comp.create_storage(&table_info_path)?;

    table_info
        .table_name
        .as_ref()
        .map(|table_name| {
            write_stream_string(
                comp,
                table_info_path.join("TableName").as_path(),
                table_name,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .author_name
        .as_ref()
        .map(|author_name| {
            write_stream_string(
                comp,
                table_info_path.join("AuthorName").as_path(),
                author_name,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .screenshot
        .as_ref()
        .map(|screenshot| {
            write_stream_binary(
                comp,
                table_info_path.join("Screenshot").as_path(),
                screenshot,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_blurb
        .as_ref()
        .map(|table_blurb| {
            write_stream_string(
                comp,
                table_info_path.join("TableBlurb").as_path(),
                table_blurb,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_rules
        .as_ref()
        .map(|table_rules| {
            write_stream_string(
                comp,
                table_info_path.join("TableRules").as_path(),
                table_rules,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .author_email
        .as_ref()
        .map(|author_email| {
            write_stream_string(
                comp,
                table_info_path.join("AuthorEmail").as_path(),
                author_email,
            )
        })
        .unwrap_or(Ok(()))?;

    table_info
        .release_date
        .as_ref()
        .map(|release_date| {
            write_stream_string(
                comp,
                table_info_path.join("ReleaseDate").as_path(),
                release_date,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_save_rev
        .as_ref()
        .map(|table_save_rev| {
            write_stream_string(
                comp,
                table_info_path.join("TableSaveRev").as_path(),
                table_save_rev,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_version
        .as_ref()
        .map(|table_version| {
            write_stream_string(
                comp,
                table_info_path.join("TableVersion").as_path(),
                table_version,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .author_website
        .as_ref()
        .map(|author_website| {
            write_stream_string(
                comp,
                table_info_path.join("AuthorWebSite").as_path(),
                author_website,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_save_date
        .as_ref()
        .map(|table_save_date| {
            write_stream_string(
                comp,
                table_info_path.join("TableSaveDate").as_path(),
                table_save_date,
            )
        })
        .unwrap_or(Ok(()))?;
    table_info
        .table_description
        .as_ref()
        .map(|table_description| {
            write_stream_string(
                comp,
                table_info_path.join("TableDescription").as_path(),
                table_description,
            )
        })
        .unwrap_or(Ok(()))?;

    // write properties
    for (key, value) in &table_info.properties {
        write_stream_string(comp, table_info_path.join(key).as_path(), value)?;
    }

    Ok(())
}

pub(crate) fn read_tableinfo<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
) -> std::io::Result<TableInfo> {
    // create path to table info using path separator
    let table_info_path = Path::new(MAIN_SEPARATOR_STR).join("TableInfo");
    let mut table_info = TableInfo::new();

    let entries = comp.read_storage(table_info_path)?;
    // read all the entries in the entrues
    let paths: Vec<_> = entries
        .filter(|entry| entry.is_stream())
        .map(|entry| entry.path().to_owned())
        .collect();

    // "/TableInfo/TableName"
    // "/TableInfo/TableDescription"

    let result: Result<Vec<_>, _> = paths
        .iter()
        .map(|path| {
            let file_name = path
                .file_name()
                .map(|s| s.to_str().unwrap_or("[not unicode]"))
                .unwrap_or("..");
            match file_name {
                "TableName" => {
                    read_stream_string(comp, path).map(|s| table_info.table_name = Some(s))
                }
                "AuthorName" => {
                    read_stream_string(comp, path).map(|s| table_info.author_name = Some(s))
                }
                "Screenshot" => {
                    // seems to be a full image file, eg if there is no jpeg data in the image this is a full png
                    // but how do we know the extension?
                    read_stream_binary(comp, path).map(|v| table_info.screenshot = Some(v))
                }
                "TableBlurb" => {
                    read_stream_string(comp, path).map(|s| table_info.table_blurb = Some(s))
                }
                "TableRules" => {
                    read_stream_string(comp, path).map(|s| table_info.table_rules = Some(s))
                }
                "AuthorEmail" => {
                    read_stream_string(comp, path).map(|s| table_info.author_email = Some(s))
                }
                "ReleaseDate" => {
                    read_stream_string(comp, path).map(|s| table_info.release_date = Some(s))
                }
                "TableSaveRev" => {
                    read_stream_string(comp, path).map(|s| table_info.table_save_rev = Some(s))
                }
                "TableVersion" => {
                    read_stream_string(comp, path).map(|s| table_info.table_version = Some(s))
                }
                "AuthorWebSite" => {
                    read_stream_string(comp, path).map(|s| table_info.author_website = Some(s))
                }
                "TableSaveDate" => {
                    read_stream_string(comp, path).map(|s| table_info.table_save_date = Some(s))
                }
                "TableDescription" => {
                    read_stream_string(comp, path).map(|s| table_info.table_description = Some(s))
                }
                other => {
                    let str = read_stream_string(comp, path)?;
                    table_info.properties.insert(other.to_string(), str);
                    Ok(())
                }
            }
        })
        .collect();

    result.map(|_| table_info)
}

fn read_stream_string<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    path: &Path,
) -> Result<String, std::io::Error> {
    let mut stream = comp.open_stream(path)?;
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;

    match decode_utf16le(&buffer) {
        Ok(str) => Ok(str),
        Err(e) => Err(std::io::Error::other(
            "Error reading stream as utf16le for path: ".to_owned()
                + path.to_str().unwrap_or("[not unicode]")
                + " "
                + &e.to_string(),
        )),
    }
}

fn write_stream_string<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    path: &Path,
    str: &str,
) -> std::io::Result<()> {
    let mut stream = comp.create_stream(path)?;
    let wide = encode_utf16le(str);
    stream.write_all(&wide)
}

fn read_stream_binary<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    path: &Path,
) -> std::io::Result<Vec<u8>> {
    let mut stream = comp.open_stream(path)?;
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn write_stream_binary<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    path: &Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    let mut stream = comp.create_stream(path)?;
    stream.write_all(bytes)
}

/// The shape of `info.json` in an extracted table directory. The
/// screenshot is not part of it, it is written to its own file, and the
/// custom tag names go into `properties_order` because a JSON object does
/// not keep the order vpinball stores them in.
#[derive(Serialize, Deserialize)]
struct TableInfoJson {
    table_name: Option<String>,
    author_name: Option<String>,
    table_blurb: Option<String>,
    table_rules: Option<String>,
    author_email: Option<String>,
    release_date: Option<String>,
    table_save_rev: Option<String>,
    table_version: Option<String>,
    author_website: Option<String>,
    table_save_date: Option<String>,
    table_description: Option<String>,
    properties: HashMap<String, String>,
    properties_order: Vec<String>,
}

/// Converts a [`TableInfo`] and its [`CustomInfoTags`] to the JSON of
/// `info.json` in an extracted table directory.
///
/// The custom tag names go into a `properties_order` array because a
/// JSON object does not keep the order vpinball stores them in. The
/// screenshot is not part of the JSON; it is written to its own file.
pub(crate) fn info_to_json(
    table_info: &TableInfo,
    custom_info_tags: &CustomInfoTags,
) -> serde_json::Value {
    let info_json = TableInfoJson {
        table_name: table_info.table_name.clone(),
        author_name: table_info.author_name.clone(),
        table_blurb: table_info.table_blurb.clone(),
        table_rules: table_info.table_rules.clone(),
        author_email: table_info.author_email.clone(),
        release_date: table_info.release_date.clone(),
        table_save_rev: table_info.table_save_rev.clone(),
        table_version: table_info.table_version.clone(),
        author_website: table_info.author_website.clone(),
        table_save_date: table_info.table_save_date.clone(),
        table_description: table_info.table_description.clone(),
        properties: table_info.properties.clone(),
        properties_order: custom_info_tags.clone(),
    };
    infallible_to_value(info_json)
}

/// Converts the JSON of `info.json` in an extracted table directory back
/// to a [`TableInfo`] and its [`CustomInfoTags`]; the inverse of
/// [`info_to_json`]. The screenshot comes from its own file and is passed
/// in as `screenshot`.
///
/// # Errors
///
/// Fails when the JSON does not have the shape [`info_to_json`] writes.
pub(crate) fn json_to_info(
    json: serde_json::Value,
    screenshot: Option<Vec<u8>>,
) -> Result<(TableInfo, CustomInfoTags), serde_json::Error> {
    let info_json: TableInfoJson = serde_json::from_value(json)?;
    let table_info = TableInfo {
        table_name: info_json.table_name,
        author_name: info_json.author_name,
        screenshot,
        table_blurb: info_json.table_blurb,
        table_rules: info_json.table_rules,
        author_email: info_json.author_email,
        release_date: info_json.release_date,
        table_save_rev: info_json.table_save_rev,
        table_version: info_json.table_version,
        author_website: info_json.author_website,
        table_save_date: info_json.table_save_date,
        table_description: info_json.table_description,
        properties: info_json.properties,
    };
    let custom_info_tags = info_json.properties_order;
    Ok((table_info, custom_info_tags))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_write_read() {
        let buff = Cursor::new(vec![0; 15]);
        let mut comp = CompoundFile::create(buff).unwrap();
        let table_info = TableInfo {
            table_name: Some("test_table_name".to_string()),
            author_name: Some("test_author_name".to_string()),
            screenshot: Some(vec![1, 2, 3]),
            table_blurb: Some("test_table_blurb".to_string()),
            table_rules: Some("test_table_rules".to_string()),
            author_email: Some("test_author_email".to_string()),
            release_date: None,
            table_save_rev: Some("test_table_save_rev".to_string()),
            table_version: Some("test_table_version".to_string()),
            author_website: Some("test_author_website".to_string()),
            table_save_date: Some("test_table_save_date".to_string()),
            table_description: Some("test_table_description".to_string()),
            properties: HashMap::from([
                ("prop1".to_string(), "value1".to_string()),
                ("prop2".to_string(), "value2".to_string()),
            ]),
        };
        write_tableinfo(&mut comp, &table_info).unwrap();
        let table_info_read = read_tableinfo(&mut comp).unwrap();

        assert_eq!(table_info_read, table_info);
    }

    // #[test]
    // fn test_bad_add() {
    //     // This assert would fire and test will fail.
    //     // Please note, that private functions can be tested too!
    //     assert_eq!(bad_add(1, 2), 3);
    // }

    #[test]
    fn info_round_trips_through_the_json() {
        let table_info = TableInfo {
            table_name: Some("Table".to_string()),
            author_name: None,
            properties: HashMap::from([("Custom".to_string(), "value".to_string())]),
            ..TableInfo::default()
        };
        let custom_info_tags = vec!["Custom".to_string()];
        let json = info_to_json(&table_info, &custom_info_tags);
        // an absent stream is written as null so the author sees it
        assert!(json["author_name"].is_null());
        let (table_info2, custom_info_tags2) = json_to_info(json, None).unwrap();
        assert_eq!(table_info, table_info2);
        assert_eq!(custom_info_tags, custom_info_tags2);
    }
}
