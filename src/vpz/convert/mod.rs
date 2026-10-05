//! A [`VPX`] as the VPZ pack vpinball saves from it (`PinTable::SaveToJSON`)
//!
//! The records of every object are written back with this crate's BIFF
//! writers, which reproduce the file, and translated with vpinball's
//! field maps: the JSON names, the value type of each field as vpinball's
//! save code writes it, the field order. Fields vpinball does not map, or
//! only reads from older files, are left out as vpinball leaves them out.
//!
//! vpinball saves the table as it holds it after loading, which for a file
//! of an older version than the vpinball doing the save includes the
//! defaults of newer fields and the conversion of legacy ones. The packs
//! made here hold the fields of the file.

mod biff;
mod fields;
mod glb;
mod to_vpx;

use super::{
    Asset, Document, FontSidecar, ImageSidecar, Manifest, NamedDocument, OutputTarget, Part,
    PartType, SoundSidecar, Vpz,
};
use crate::vpx::VPX;
use crate::vpx::biff::{BiffWrite, BiffWriter};
use crate::vpx::gameitem::{self, GameItemEnum};
use biff::{Bytes, Extras, float, invalid, sort_fields, utf8_or_cp1252};
use serde_json::{Map, Value as Json};
use std::io;
pub use to_vpx::to_vpx;

/// One field of vpinball's JSON field map of an object
pub(super) struct Field {
    /// The BIFF record tag
    tag: &'static str,
    /// The JSON name, dotted for a field of a grouping object
    name: &'static str,
    /// The value type vpinball writes or reads the field with, `None` for a
    /// field vpinball neither writes nor uses
    value: Option<Value>,
}

impl Field {
    const fn new(tag: &'static str, name: &'static str, value: Option<Value>) -> Self {
        Self { tag, name, value }
    }

    /// A field vpinball only reads from older files and converts on load.
    /// It is carried over when a table has it, so vpinball converts it when
    /// it loads the pack.
    const fn legacy(tag: &'static str, name: &'static str, value: Value) -> Self {
        Self {
            tag,
            name,
            value: Some(value),
        }
    }
}

/// The `IObjectWriter` call a field is written with
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Value {
    Bool,
    Int,
    UInt,
    Float,
    String,
    WideString,
    Vector2,
    Vector3,
    Vector4,
    Font,
    Raw,
    Script,
    /// Sub objects, collected in an array
    Objects(Node),
}

/// The objects with a field map
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Node {
    /// A part, by its vpinball item type
    Part(u32),
    Table,
    Collection,
    Material,
    RenderProbe,
    DragPoint,
}

impl Node {
    fn fields(self) -> &'static [Field] {
        match self {
            Node::Part(item_type) => fields::part_fields(item_type).unwrap_or(&[]),
            Node::Table => fields::TABLE,
            Node::Collection => fields::COLLECTION,
            Node::Material => fields::MATERIAL,
            Node::RenderProbe => fields::RENDER_PROBE,
            Node::DragPoint => fields::DRAG_POINT,
        }
    }

    fn is_repeatable(self, tag: &str) -> bool {
        self == Node::Collection && fields::COLLECTION_REPEATABLE.contains(&tag)
    }
}

/// The `$type` vpinball gives a part of an item type (`GetPartTypeName`)
fn part_type(item_type: u32) -> Option<PartType> {
    Some(match item_type {
        0 => PartType::Surface,
        1 => PartType::Flipper,
        2 => PartType::Timer,
        3 => PartType::Plunger,
        4 => PartType::Textbox,
        5 => PartType::Bumper,
        6 => PartType::Trigger,
        7 => PartType::Light,
        8 => PartType::Kicker,
        9 => PartType::Decal,
        10 => PartType::Gate,
        11 => PartType::Spinner,
        12 => PartType::Ramp,
        17 => PartType::DispReel,
        18 => PartType::LightSeq,
        19 => PartType::Primitive,
        20 => PartType::Flasher,
        21 => PartType::Rubber,
        22 => PartType::HitTarget,
        23 => PartType::Ball,
        24 => PartType::PartGroup,
        _ => return None,
    })
}

/// The JSON document of an object, its fields in vpinball's order
fn object_document(
    records: &[u8],
    node: Node,
    extras: &mut Extras,
) -> io::Result<Map<String, Json>> {
    let fields = node.fields();
    let mut document = biff::document(records, node, fields, extras)?;
    sort_fields(&mut document, node, fields, "");
    Ok(document)
}

/// An error naming the object it happened in
fn context(what: &str, name: &str) -> impl FnOnce(io::Error) -> io::Error {
    let what = format!("{what} {name:?}");
    move |e| io::Error::new(e.kind(), format!("{what}: {e}"))
}

fn take_name(document: &mut Map<String, Json>, what: &str) -> io::Result<String> {
    match document.shift_remove("name") {
        Some(Json::String(name)) => Ok(name),
        None => Ok(String::new()),
        Some(other) => Err(invalid(format!("{what} has a non-string name {other}"))),
    }
}

/// The pack vpinball saves from a table. `save_date` is the save time in
/// C `asctime` form, as vpinball writes it in the manifest and
/// `table.json`, for example `Sun Oct  4 21:13:20 2026`.
pub fn from_vpx(vpx: &VPX, save_date: &str) -> io::Result<Vpz> {
    let parts = parts(vpx)?;
    let collections = vpx
        .collections
        .iter()
        .map(|collection| {
            collection_document(collection).map_err(context("collection", &collection.name))
        })
        .collect::<io::Result<Vec<_>>>()?;

    let records = crate::vpx::gamedata::write_all_gamedata_records(&vpx.gamedata, &vpx.version);
    let mut extras = Extras::default();
    let mut table = object_document(&records, Node::Table, &mut extras)
        .map_err(context("table", "GameData"))?;
    let materials = sub_documents(&mut table, "materials")?;
    let render_probes = sub_documents(&mut table, "renderprobes")?;

    let info = &vpx.info;
    let text = |value: &Option<String>| Json::from(value.clone().unwrap_or_default());
    table.insert("table_name".into(), text(&info.table_name));
    table.insert("author".into(), text(&info.author_name));
    table.insert("table_version".into(), text(&info.table_version));
    table.insert("release_date".into(), text(&info.release_date));
    table.insert("author_email".into(), text(&info.author_email));
    table.insert("web_site".into(), text(&info.author_website));
    table.insert("blurb".into(), text(&info.table_blurb));
    table.insert("description".into(), text(&info.table_description));
    table.insert("rules".into(), text(&info.table_rules));
    table.insert("date_saved".into(), Json::from(save_date));
    let save_rev = info
        .table_save_rev
        .as_deref()
        .and_then(|rev| rev.trim().parse::<u32>().ok())
        .unwrap_or(0);
    table.insert("save_rev".into(), Json::from(save_rev + 1));
    table.insert("parts".into(), Json::Array(Vec::new()));
    table.insert("collections".into(), Json::Array(Vec::new()));
    let mut custom_tags = Map::new();
    for tag in &vpx.custominfotags {
        let value = info.properties.get(tag).cloned().unwrap_or_default();
        custom_tags.insert(tag.clone(), Json::from(value));
    }
    table.insert("custom_tags".into(), Json::Object(custom_tags));

    let manifest = Manifest {
        name: Some(info.table_name.clone().unwrap_or_default()),
        author: Some(info.author_name.clone().unwrap_or_default()),
        version: Some(info.table_version.clone().unwrap_or_default()),
        description: Some(info.table_blurb.clone().unwrap_or_default()),
        save_date: Some(save_date.to_string()),
        ..Manifest::default()
    };

    Ok(Vpz {
        manifest,
        table: Some(Document { properties: table }),
        script: extras.script,
        parts,
        collections,
        materials,
        render_probes,
        images: images(vpx)?,
        sounds: sounds(vpx)?,
        fonts: fonts(vpx)?,
        other_files: Default::default(),
    })
}

fn collection_document(
    collection: &crate::vpx::collection::Collection,
) -> io::Result<NamedDocument> {
    let records = crate::vpx::collection::write(collection);
    let mut properties = object_document(&records, Node::Collection, &mut Extras::default())?;
    let name = take_name(&mut properties, "a collection")?;
    Ok(NamedDocument { name, properties })
}

/// The parts in z-order, part groups first like vpinball saves them
fn parts(vpx: &VPX) -> io::Result<Vec<Part>> {
    let mut items: Vec<&GameItemEnum> = vpx.gameitems.iter().collect();
    items.sort_by_key(|item| !matches!(item, GameItemEnum::PartGroup(_)));
    items
        .into_iter()
        .map(|item| part(item).map_err(context("part", item.name())))
        .collect()
}

fn part(item: &GameItemEnum) -> io::Result<Part> {
    let bytes = gameitem::write(item);
    let mut reader = Bytes::new(&bytes);
    let item_type = reader.u32()?;
    let part_type = part_type(item_type)
        .ok_or_else(|| invalid(format!("item type {item_type} is not a part")))?;
    let mut properties =
        object_document(&bytes[4..], Node::Part(item_type), &mut Extras::default())?;
    let name = take_name(&mut properties, "a part")?;
    let mesh = match item {
        GameItemEnum::Primitive(primitive) if primitive.use_3d_mesh => {
            glb::primitive_glb(primitive)?
        }
        _ => None,
    };
    Ok(Part {
        name,
        part_type,
        properties,
        mesh,
    })
}

/// Moves the sub documents of a table list to their own documents, the
/// list keeping their names
fn sub_documents(table: &mut Map<String, Json>, key: &str) -> io::Result<Vec<NamedDocument>> {
    let Some(Json::Array(documents)) = table.get_mut(key) else {
        return Ok(Vec::new());
    };
    let mut named = Vec::with_capacity(documents.len());
    let mut names = Vec::with_capacity(documents.len());
    for document in documents.drain(..) {
        let Json::Object(mut properties) = document else {
            return Err(invalid(format!("{key} holds a non-object entry")));
        };
        let name = take_name(&mut properties, key)?;
        names.push(Json::from(name.clone()));
        named.push(NamedDocument { name, properties });
    }
    *documents = names;
    Ok(named)
}

/// The records of a BIFF stream as `(tag, data)`. A record of only its tag
/// is followed by its sub object, up to and including the sub object's
/// `ENDB`, which becomes its data.
fn records(data: &[u8]) -> io::Result<Vec<(String, &[u8])>> {
    let mut reader = Bytes::new(data);
    let mut records = Vec::new();
    while reader.remaining() > 0 {
        let (tag, size) = record_header(&mut reader)?;
        if tag == "ENDB" {
            break;
        }
        if size == 4 && SUB_OBJECT_TAGS.contains(&tag.as_str()) {
            let start = data.len() - reader.remaining();
            loop {
                let (sub_tag, sub_size) = record_header(&mut reader)?;
                reader.take(sub_size - 4)?;
                if sub_tag == "ENDB" {
                    break;
                }
            }
            let end = data.len() - reader.remaining();
            records.push((tag, &data[start..end]));
            continue;
        }
        let data = reader.take(size - 4)?;
        records.push((tag, data));
    }
    Ok(records)
}

/// The tags of sub objects written as a record of only their tag
const SUB_OBJECT_TAGS: [&str; 1] = ["JPEG"];

fn record_header(reader: &mut Bytes) -> io::Result<(String, usize)> {
    let size = reader.u32()? as usize;
    let tag = String::from_utf8_lossy(reader.take(4)?)
        .trim_end_matches('\0')
        .to_string();
    if size < 4 {
        return Err(invalid(format!("record {tag:?} has size {size}")));
    }
    Ok((tag, size))
}

fn path_extension(path: &str) -> String {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match file_name.rfind('.') {
        Some(dot) if dot > 0 => file_name[dot..].to_lowercase(),
        _ => String::new(),
    }
}

/// The asset extension vpinball writes: the import path's, lower case,
/// else the default
fn asset_extension(path: &str, default: &str) -> String {
    let extension = path_extension(path);
    let extension = if extension.is_empty() {
        default.to_string()
    } else {
        extension
    };
    extension.trim_start_matches('.').to_string()
}

fn images(vpx: &VPX) -> io::Result<Vec<Asset<ImageSidecar>>> {
    vpx.images
        .iter()
        .map(|image| image_asset(vpx, image).map_err(context("image", &image.name)))
        .collect()
}

fn image_asset(vpx: &VPX, image: &crate::vpx::image::ImageData) -> io::Result<Asset<ImageSidecar>> {
    // a legacy bitmap is not made of records, it is handled below
    let mut records_image = image.clone();
    records_image.bits = None;
    let mut writer = BiffWriter::new();
    records_image.biff_write(&mut writer);
    let data = writer.into_data();
    let mut name = String::new();
    let mut sidecar = ImageSidecar::default();
    let mut bytes = Vec::new();
    let mut image_path = String::new();
    for (tag, record) in records(&data)? {
        let mut reader = Bytes::new(record);
        match tag.as_str() {
            "NAME" => name = utf8_or_cp1252(reader.sized()?),
            "PATH" if image.bits.is_some() => image_path = utf8_or_cp1252(reader.sized()?),
            "WDTH" => sidecar.width = Some(reader.u32()?),
            "HGHT" => sidecar.height = Some(reader.u32()?),
            "ALTV" => {
                // read as a 0..1 threshold and written back on the 0..255 scale
                let value = reader.f32()? * (1.0 / 255.0) as f32 * 255.0;
                sidecar.alpha_test = float(value).as_f64();
            }
            "MD5H" => sidecar.md5 = Some(record.iter().map(|b| format!("{b:02x}")).collect()),
            "OPAQ" => sidecar.opaque = Some(reader.i32()? != 0),
            "JPEG" => {
                for (tag, record) in records(record)? {
                    let mut reader = Bytes::new(record);
                    match tag.as_str() {
                        "PATH" => image_path = utf8_or_cp1252(reader.sized()?),
                        "DATA" => bytes = record.to_vec(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // a linked image is the table screenshot, without a path
    if image.link.is_some() {
        bytes = vpx.info.screenshot.clone().unwrap_or_default();
    }
    if let Some(bits) = &image.bits {
        bytes = bitmap_webp(&bits.lzw_compressed_data, image.width, image.height)?;
        // the path gets the webp extension
        let extension = path_extension(&image_path);
        if !extension.is_empty() {
            image_path.truncate(image_path.len() - (extension.len() - 1));
            image_path.push_str("webp");
        }
    }
    sidecar.import_path = Some(image_path.clone());
    Ok(Asset {
        name,
        extension: asset_extension(&image_path, ".png"),
        data: bytes,
        sidecar,
    })
}

/// A legacy bitmap as vpinball converts it on load: a lossless webp,
/// without alpha channel when every alpha value is 0 or 255. vpinball
/// encodes with FreeImage, so the bytes differ, the pixels do not.
fn bitmap_webp(lzw_compressed_data: &[u8], width: u32, height: u32) -> io::Result<Vec<u8>> {
    let decoded =
        crate::vpx::image::vpx_image_to_dynamic_image(lzw_compressed_data, width, height)?;
    let decoded = match decoded {
        image::DynamicImage::ImageRgba8(rgba)
            if rgba.pixels().all(|pixel| pixel[3] == 0 || pixel[3] == 255) =>
        {
            image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(rgba).to_rgb8())
        }
        decoded => decoded,
    };
    crate::vpx::images::encode(&decoded, image::ImageFormat::WebP, 0)
}

fn sounds(vpx: &VPX) -> io::Result<Vec<Asset<SoundSidecar>>> {
    vpx.sounds
        .iter()
        .map(|sound| sound_asset(vpx, sound).map_err(context("sound", &sound.name)))
        .collect()
}

/// The file version since which sounds store their output target, volume,
/// balance and fade
const NEW_SOUND_FORMAT_VERSION: u32 = 1031;

fn sound_asset(vpx: &VPX, sound: &crate::vpx::sound::SoundData) -> io::Result<Asset<SoundSidecar>> {
    let mut writer = BiffWriter::new();
    crate::vpx::sound::write(&vpx.version, sound, &mut writer);
    let data = writer.into_data();
    let mut reader = Bytes::new(&data);
    let name = utf8_or_cp1252(reader.sized()?);
    let path = utf8_or_cp1252(reader.sized()?);
    let _internal_name = reader.sized()?;
    // vpinball only takes a `.wav` extension as WAV; this crate also takes
    // a path without extension, as old files store those sounds as WAV
    let is_wav = crate::vpx::sound::is_wav(&sound.path);
    let bytes = if is_wav {
        // the format block, then the samples
        reader.take(18)?;
        reader.sized()?;
        crate::vpx::sound::vpinball_wav_file(&sound.wave_form, &sound.data)
    } else {
        reader.sized()?.to_vec()
    };
    let mut sidecar = SoundSidecar {
        import_path: Some(path.clone()),
        output_target: Some(OutputTarget::Playfield),
        volume_offset: Some(100),
        left_right_offset: Some(100),
        rear_front_offset: Some(100),
        ..SoundSidecar::default()
    };
    if vpx.version.u32() >= NEW_SOUND_FORMAT_VERSION {
        let output_target = reader.u8()?;
        let _volume = reader.i32()?;
        sidecar.left_right_offset = Some(reader.i32()?);
        sidecar.rear_front_offset = Some(reader.i32()?);
        sidecar.volume_offset = Some(reader.i32()?);
        if output_target == 1 {
            sidecar.output_target = Some(OutputTarget::Backglass);
        }
    } else {
        let to_backglass = reader.u8()? != 0;
        let legacy_backglass = name.to_lowercase().contains("bgout_")
            || path.eq_ignore_ascii_case("* Backglass Output *");
        if to_backglass || legacy_backglass {
            sidecar.output_target = Some(OutputTarget::Backglass);
        }
    }
    Ok(Asset {
        name,
        extension: asset_extension(&path, ".wav"),
        data: bytes,
        sidecar,
    })
}

fn fonts(vpx: &VPX) -> io::Result<Vec<Asset<FontSidecar>>> {
    vpx.fonts
        .iter()
        .map(|font| font_asset(font).map_err(context("font", &font.name)))
        .collect()
}

fn font_asset(font: &crate::vpx::pinbinary::PinBinary) -> io::Result<Asset<FontSidecar>> {
    let data = crate::vpx::pinbinary::write(font);
    let mut name = String::new();
    let mut path = String::new();
    let mut bytes = Vec::new();
    for (tag, record) in records(&data)? {
        let mut reader = Bytes::new(record);
        match tag.as_str() {
            "NAME" => name = utf8_or_cp1252(reader.sized()?),
            "PATH" => path = utf8_or_cp1252(reader.sized()?),
            "DATA" => bytes = record.to_vec(),
            _ => {}
        }
    }
    Ok(Asset {
        name,
        extension: asset_extension(&path, ".ttf"),
        data: bytes,
        sidecar: FontSidecar {
            import_path: Some(path),
            ..FontSidecar::default()
        },
    })
}
