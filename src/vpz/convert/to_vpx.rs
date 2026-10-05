//! A [`Vpz`] as the `.vpx` table vpinball saves after loading it
//! (`PinTable::LoadGameFromJSONPack` followed by `PinTable::Save`)
//!
//! The JSON documents are translated back to BIFF records with the same
//! field tables as [`super::from_vpx`] and read with this crate's readers.
//! Text is stored as UTF-8, as vpinball stores it since file version 1090.
//!
//! vpinball loads a pack as a table of the current file version: fields a
//! pack does not have keep vpinball's defaults, which this crate does not
//! know, so they are left out of the records and the readers use their own.

use super::biff::invalid;
use super::{Field, Node, Value, glb};
use crate::vpx::VPX;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive::{
    MAX_VERTICES_FOR_2_BYTE_INDEX, Primitive, VertData, compress_mesh_data,
    write_animation_vertex_data,
};
use crate::vpx::image::ImageData;
use crate::vpx::material::{SaveMaterial, SavePhysicsMaterial};
use crate::vpx::pinbinary::PinBinary;
use crate::vpx::sound::{OutputTarget as VpxOutputTarget, SoundData, WaveForm};
use crate::vpx::version::Version;
use crate::vpz::{Asset, FontSidecar, ImageSidecar, OutputTarget, SoundSidecar, Vpz};
use bytes::BytesMut;
use serde_json::{Map, Value as Json};
use std::io;

/// The file version vpinball writes when it saves a pack as a table
const FILE_VERSION: u32 = 1090;

/// The table vpinball saves from a pack. `save_date` is the save time in
/// C `asctime` form, written as the table's save date like
/// [`super::from_vpx`] does; the save revision is the pack's plus one.
pub fn to_vpx(vpz: &Vpz, save_date: &str) -> io::Result<VPX> {
    let version = Version::new(FILE_VERSION);
    let empty = Map::new();
    let table = vpz.table.as_ref().map_or(&empty, |table| &table.properties);

    let mut gameitems = Vec::with_capacity(vpz.parts.len());
    for part in &vpz.parts {
        let item = part_item(part).map_err(super::context("part", &part.name))?;
        gameitems.push(item);
    }

    let collections = vpz
        .collections
        .iter()
        .map(|collection| {
            let mut properties = named(&collection.name, &collection.properties);
            properties.shift_remove("$type");
            let records = records(&properties, Node::Collection, None)?;
            crate::vpx::collection::read(&records)
        })
        .collect::<io::Result<Vec<_>>>()?;

    // the table records, with the material and render probe documents back
    // in their lists
    let mut table_properties = table.clone();
    table_properties.insert(
        "materials".into(),
        Json::Array(
            vpz.materials
                .iter()
                .map(|material| Json::Object(named(&material.name, &material.properties)))
                .collect(),
        ),
    );
    table_properties.insert(
        "renderprobes".into(),
        Json::Array(
            vpz.render_probes
                .iter()
                .map(|probe| Json::Object(named(&probe.name, &probe.properties)))
                .collect(),
        ),
    );
    let records = records(
        &table_properties,
        Node::Table,
        Some(vpz.script.as_deref().unwrap_or("")),
    )
    .map_err(super::context("table", "table.json"))?;
    let mut gamedata = crate::vpx::gamedata::read_all_gamedata_records(&records, &version)?;
    // vpinball also writes the materials in the pre 10.8 formats
    let materials = gamedata.materials.as_deref().unwrap_or(&[]);
    let materials_size = materials.len() as u32;
    let materials_old = materials.iter().map(SaveMaterial::from).collect();
    let materials_physics_old =
        (!materials.is_empty()).then(|| materials.iter().map(SavePhysicsMaterial::from).collect());
    gamedata.materials_size = materials_size;
    gamedata.materials_old = materials_old;
    gamedata.materials_physics_old = materials_physics_old;

    let save_rev = table.get("save_rev").and_then(Json::as_u64).unwrap_or(0);
    let (mut info, custominfotags) = crate::vpz::info::table_info(
        vpz.table
            .as_ref()
            .unwrap_or(&crate::vpz::Document::default()),
    );
    info.table_save_date = Some(save_date.to_string());
    info.table_save_rev = Some((save_rev + 1).to_string());

    // the screenshot image is stored as the table's screenshot, linked
    let screenshot = table.get("screenshot").and_then(Json::as_str).unwrap_or("");
    let mut images = Vec::with_capacity(vpz.images.len());
    for image in &vpz.images {
        let mut data = image_data(image).map_err(super::context("image", &image.name))?;
        if !screenshot.is_empty() && image.name.eq_ignore_ascii_case(screenshot) {
            info.screenshot = data.jpeg.take().map(|jpeg| jpeg.data);
            data.link = Some(1);
        }
        images.push(data);
    }
    let sounds = vpz
        .sounds
        .iter()
        .map(|sound| sound_data(sound).map_err(super::context("sound", &sound.name)))
        .collect::<io::Result<Vec<_>>>()?;
    let fonts: Vec<PinBinary> = vpz.fonts.iter().map(font_data).collect();

    gamedata.gameitems_size = gameitems.len() as u32;
    gamedata.collections_size = collections.len() as u32;
    gamedata.images_size = images.len() as u32;
    gamedata.sounds_size = sounds.len() as u32;
    gamedata.fonts_size = fonts.len() as u32;

    Ok(VPX {
        custominfotags,
        info,
        version,
        gamedata,
        gameitems,
        images,
        sounds,
        fonts,
        collections,
    })
}

/// The properties of a document with its `name` first
fn named(name: &str, properties: &Map<String, Json>) -> Map<String, Json> {
    let mut named = Map::with_capacity(properties.len() + 1);
    named.insert("name".into(), Json::from(name));
    for (key, value) in properties {
        if key != "name" {
            named.insert(key.clone(), value.clone());
        }
    }
    named
}

/// The vpinball item type of a part `$type` (`GetPartTypeFromName`)
fn item_type(part_type: &crate::vpz::PartType) -> Option<u32> {
    (0..=24).find(|&item_type| super::part_type(item_type).as_ref() == Some(part_type))
}

fn part_item(part: &crate::vpz::Part) -> io::Result<GameItemEnum> {
    let item_type = item_type(&part.part_type)
        .ok_or_else(|| invalid(format!("unsupported part type {}", part.part_type)))?;
    let properties = named(&part.name, &part.properties);
    let mut bytes = item_type.to_le_bytes().to_vec();
    bytes.extend(records(&properties, Node::Part(item_type), None)?);
    let mut item = crate::vpx::gameitem::read(&bytes)?;
    if let (GameItemEnum::Primitive(primitive), Some(mesh)) = (&mut item, &part.mesh) {
        set_mesh(primitive, mesh)?;
    }
    Ok(item)
}

/// The mesh of a primitive from its glTF binary, compressed as vpinball
/// saves it
fn set_mesh(primitive: &mut Primitive, glb: &[u8]) -> io::Result<()> {
    let mesh = glb::read_glb(glb)?;
    let vertex_count = mesh.vertices.len();
    let mut vertices = Vec::with_capacity(vertex_count * 32);
    for vertex in &mesh.vertices {
        vertices.extend_from_slice(&vertex.to_vpx_bytes());
    }
    let wide = vertex_count > MAX_VERTICES_FOR_2_BYTE_INDEX;
    let mut indices = Vec::with_capacity(mesh.indices.len() * if wide { 4 } else { 2 });
    for &index in &mesh.indices {
        if wide {
            indices.extend_from_slice(&index.to_le_bytes());
        } else {
            indices.extend_from_slice(&(index as u16).to_le_bytes());
        }
    }
    let compressed_vertices = compress_mesh_data(&vertices)?;
    let compressed_indices = compress_mesh_data(&indices)?;
    primitive.num_vertices = Some(vertex_count as u32);
    primitive.num_indices = Some(mesh.indices.len() as u32);
    primitive.compressed_vertices_len = Some(compressed_vertices.len() as u32);
    primitive.compressed_vertices_data = Some(compressed_vertices);
    primitive.compressed_indices_len = Some(compressed_indices.len() as u32);
    primitive.compressed_indices_data = Some(compressed_indices);
    primitive.vertices_data = None;
    primitive.indices_data = None;
    if mesh.frames.is_empty() {
        primitive.compressed_animation_vertices_len = None;
        primitive.compressed_animation_vertices_data = None;
    } else {
        let mut lengths = Vec::with_capacity(mesh.frames.len());
        let mut frames = Vec::with_capacity(mesh.frames.len());
        for frame in &mesh.frames {
            let mut buff = BytesMut::with_capacity(frame.len() * VertData::SERIALIZED_SIZE);
            for vertex in frame {
                write_animation_vertex_data(&mut buff, vertex);
            }
            let compressed = compress_mesh_data(&buff)?;
            lengths.push(compressed.len() as u32);
            frames.push(compressed);
        }
        primitive.compressed_animation_vertices_len = Some(lengths);
        primitive.compressed_animation_vertices_data = Some(frames);
    }
    Ok(())
}

/// A BIFF record stream writer
#[derive(Default)]
struct Records(Vec<u8>);

impl Records {
    fn record(&mut self, tag: &str, payload: &[u8]) {
        self.0
            .extend_from_slice(&((4 + payload.len()) as u32).to_le_bytes());
        self.tag(tag);
        self.0.extend_from_slice(payload);
    }

    /// A record of only its tag, its data following outside of it
    fn tag_only(&mut self, tag: &str, data: &[u8]) {
        self.0.extend_from_slice(&4u32.to_le_bytes());
        self.tag(tag);
        self.0.extend_from_slice(data);
    }

    fn tag(&mut self, tag: &str) {
        let mut bytes = [0u8; 4];
        for (byte, tag_byte) in bytes.iter_mut().zip(tag.bytes()) {
            *byte = tag_byte;
        }
        self.0.extend_from_slice(&bytes);
    }
}

/// The BIFF records of a document, ending with `ENDB`. Properties the
/// field table does not map, or maps to a field vpinball only reads, are
/// left out; `script` is the text of a `CODE` field.
fn records(
    properties: &Map<String, Json>,
    node: Node,
    script: Option<&str>,
) -> io::Result<Vec<u8>> {
    let mut records = Records::default();
    write_properties(&mut records, properties, node, node.fields(), "", script)?;
    records.record("ENDB", &[]);
    Ok(records.0)
}

fn write_properties(
    records: &mut Records,
    properties: &Map<String, Json>,
    node: Node,
    fields: &[Field],
    prefix: &str,
    script: Option<&str>,
) -> io::Result<()> {
    for (key, value) in properties {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        // a name mapped twice resolves to its last field, as in vpinball
        let field = fields
            .iter()
            .rev()
            .find(|field| field.name == path && field.value.is_some());
        let Some(field) = field else {
            // a grouping object of dotted fields (`desktop_view.fov`)
            let dotted = format!("{path}.");
            if let Json::Object(group) = value
                && fields.iter().any(|field| field.name.starts_with(&dotted))
            {
                write_properties(records, group, node, fields, &path, script)?;
            }
            continue;
        };
        let value_type = field.value.unwrap_or(Value::Raw);
        match value_type {
            Value::Objects(sub_node) => {
                for element in value.as_array().into_iter().flatten() {
                    let Json::Object(element) = element else {
                        return Err(invalid(format!("{path} holds a non-object entry")));
                    };
                    let nested = self::records(element, sub_node, None)?;
                    records.record(field.tag, &nested);
                }
            }
            Value::Script => {
                let script = script.unwrap_or("").as_bytes();
                let mut data = (script.len() as u32).to_le_bytes().to_vec();
                data.extend_from_slice(script);
                records.tag_only(field.tag, &data);
            }
            Value::Font => records.tag_only(field.tag, &font(value, &path)?),
            _ if node.is_repeatable(field.tag) => {
                for element in value.as_array().into_iter().flatten() {
                    records.record(field.tag, &encode(value_type, element, &path)?);
                }
            }
            _ => records.record(field.tag, &encode(value_type, value, &path)?),
        }
    }
    Ok(())
}

fn number(value: &Json, path: &str) -> io::Result<f64> {
    match value {
        Json::Number(number) => number
            .as_f64()
            .ok_or_else(|| invalid(format!("{path}: {number} is not a number"))),
        Json::Bool(flag) => Ok(f64::from(u8::from(*flag))),
        // a float that is not finite, which JSON cannot hold
        Json::Null => Ok(f64::NAN),
        other => Err(invalid(format!("{path} should be a number, got {other}"))),
    }
}

/// Floats of a vector object; a missing component is 0, as vpinball reads
/// it, a `null` one not finite
fn floats(value: &Json, names: &[&str], path: &str) -> io::Result<Vec<u8>> {
    let mut data = Vec::with_capacity(names.len() * 4);
    for name in names {
        let component = match value.get(name) {
            None => 0.0,
            Some(component) => number(component, &format!("{path}.{name}"))?,
        };
        data.extend_from_slice(&(component as f32).to_le_bytes());
    }
    Ok(data)
}

fn sized(bytes: &[u8]) -> Vec<u8> {
    let mut data = (bytes.len() as u32).to_le_bytes().to_vec();
    data.extend_from_slice(bytes);
    data
}

fn encode(value_type: Value, value: &Json, path: &str) -> io::Result<Vec<u8>> {
    Ok(match value_type {
        Value::Bool => {
            let flag = match value {
                Json::Bool(flag) => *flag,
                other => number(other, path)? != 0.0,
            };
            i32::from(flag).to_le_bytes().to_vec()
        }
        Value::Int => (number(value, path)? as i32).to_le_bytes().to_vec(),
        Value::UInt => (number(value, path)? as u32).to_le_bytes().to_vec(),
        Value::Float => (number(value, path)? as f32).to_le_bytes().to_vec(),
        Value::String => sized(text(value, path)?.as_bytes()),
        Value::WideString => {
            let units: Vec<u8> = text(value, path)?
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            sized(&units)
        }
        Value::Vector2 => floats(value, &["x", "y"], path)?,
        Value::Vector3 => floats(value, &["x", "y", "z"], path)?,
        Value::Vector4 => floats(value, &["x", "y", "z", "w"], path)?,
        Value::Raw => match value {
            // a vector serialized as an object (render probe reflection plane)
            Json::Object(_) => floats(value, &["x", "y", "z", "w"], path)?,
            Json::Array(words) => {
                let mut data = Vec::with_capacity(words.len() * 4);
                for word in words {
                    data.extend_from_slice(&(number(word, path)? as u32).to_le_bytes());
                }
                data
            }
            other => return Err(invalid(format!("{path} should be raw data, got {other}"))),
        },
        Value::Font | Value::Script | Value::Objects(_) => {
            return Err(invalid(format!("{path} is not a plain record")));
        }
    })
}

fn text<'a>(value: &'a Json, path: &str) -> io::Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| invalid(format!("{path} should be a string, got {value}")))
}

/// A font descriptor as vpinball writes it after its record
fn font(value: &Json, path: &str) -> io::Result<Vec<u8>> {
    let name = value.get("name").and_then(Json::as_str).unwrap_or("");
    let flag = |key: &str| value.get(key).and_then(Json::as_bool).unwrap_or(false);
    let number = |key: &str| value.get(key).and_then(Json::as_u64).unwrap_or(0);
    let name_bytes = name.as_bytes();
    let name_len = u8::try_from(name_bytes.len())
        .map_err(|_| invalid(format!("{path}: font name {name:?} is too long")))?;
    let attributes = (u8::from(flag("italic")) << 1)
        | (u8::from(flag("underline")) << 2)
        | (u8::from(flag("strikethrough")) << 3);
    let mut data = vec![1];
    data.extend_from_slice(&(number("charset") as u16).to_le_bytes());
    data.push(attributes);
    data.extend_from_slice(&(number("weight") as u16).to_le_bytes());
    data.extend_from_slice(&(number("size") as u32).to_le_bytes());
    data.push(name_len);
    data.extend_from_slice(name_bytes);
    Ok(data)
}

/// Text as this crate holds the bytes of a BIFF string: one character per
/// byte. vpinball writes text as UTF-8, so non-ASCII text shows as the
/// characters of its UTF-8 bytes and is written back as those bytes.
fn as_stored(text: &str) -> String {
    text.bytes().map(char::from).collect()
}

fn image_data(image: &Asset<ImageSidecar>) -> io::Result<ImageData> {
    let sidecar = &image.sidecar;
    let path = as_stored(sidecar.import_path.as_deref().unwrap_or(""));
    let md5_hash = match &sidecar.md5 {
        Some(hex) => {
            let bytes = hex::decode(hex).map_err(|e| invalid(format!("md5 {hex:?}: {e}")))?;
            Some(
                <[u8; 16]>::try_from(bytes.as_slice())
                    .map_err(|_| invalid(format!("md5 {hex:?} is not 16 bytes")))?,
            )
        }
        None => None,
    };
    Ok(ImageData {
        name: as_stored(&image.name),
        internal_name: None,
        path: path.clone(),
        width: sidecar.width.unwrap_or(0),
        height: sidecar.height.unwrap_or(0),
        link: None,
        // vpinball reads the 0..255 value, stores it divided by 255 and
        // writes it multiplied by 255 again
        alpha_test_value: (sidecar.alpha_test.unwrap_or(-255.0) as f32 * (1.0 / 255.0) as f32)
            * 255.0,
        is_opaque: sidecar.opaque,
        is_signed: None,
        jpeg: Some(PinBinary {
            name: as_stored(&image.name),
            internal_name: None,
            path,
            data: image.data.clone(),
        }),
        bits: None,
        md5_hash,
    })
}

fn sound_data(sound: &Asset<SoundSidecar>) -> io::Result<SoundData> {
    let sidecar = &sound.sidecar;
    let path = sidecar.import_path.clone().unwrap_or_default();
    let mut data = SoundData {
        name: as_stored(&sound.name),
        path: as_stored(&path),
        wave_form: WaveForm::default(),
        data: sound.data.clone(),
        internal_name: String::new(),
        fade: sidecar.rear_front_offset.unwrap_or(0) as u32,
        volume: sidecar.volume_offset.unwrap_or(0) as u32,
        balance: sidecar.left_right_offset.unwrap_or(0) as u32,
        output_target: match sidecar.output_target {
            Some(OutputTarget::Backglass) => VpxOutputTarget::Backglass,
            _ => VpxOutputTarget::Table,
        },
    };
    // a WAV is stored as its format block and samples, as vpinball saves it
    if crate::vpx::sound::is_wav(&path) {
        (data.wave_form, data.data) = crate::vpx::sound::read_vpinball_wav(&sound.data)?;
    }
    Ok(data)
}

fn font_data(font: &Asset<FontSidecar>) -> PinBinary {
    PinBinary {
        name: as_stored(&font.name),
        internal_name: None,
        path: as_stored(font.sidecar.import_path.as_deref().unwrap_or("")),
        data: font.data.clone(),
    }
}
