//! BIFF records to the JSON documents of vpinball's VPZ writer
//! (`JSONObjectWriter`), driven by the field tables

use super::{Field, Node, Value};
use serde_json::{Map, Value as Json};
use std::io;

pub(super) fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Text as vpinball reads it from a file: UTF-8 is kept, anything else is
/// taken as Windows-1252, the five unassigned bytes mapping to U+0081...
/// like Windows does (`string_from_utf8_or_cp1252`)
pub(super) fn utf8_or_cp1252(bytes: &[u8]) -> String {
    const CP1252_80_9F: [u16; 32] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022,
        0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
    ];
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9F => char::from_u32(u32::from(CP1252_80_9F[usize::from(b - 0x80)]))
                    .unwrap_or(char::REPLACEMENT_CHARACTER),
                _ => char::from(b),
            })
            .collect(),
    }
}

/// A little endian reader over a byte slice
pub(super) struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(super) fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| {
                invalid(format!(
                    "{count} bytes wanted at offset {}, {} left",
                    self.pos,
                    self.remaining()
                ))
            })?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    pub(super) fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap_or_default(),
        ))
    }

    pub(super) fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    pub(super) fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    pub(super) fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    /// A string with a 32-bit length prefix, as raw bytes
    pub(super) fn sized(&mut self) -> io::Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }
}

/// What the records of an object hold besides its JSON fields
#[derive(Default)]
pub(super) struct Extras {
    /// The `CODE` record: the script, as vpinball decodes it
    pub(super) script: Option<String>,
}

/// The JSON document of the BIFF records of one object, in record order,
/// before [`sort_fields`]. Records the field table does not map, or maps
/// without a type (fields vpinball only reads), are left out like
/// `JSONObjectWriter` leaves out unmapped fields.
pub(super) fn document(
    records: &[u8],
    node: Node,
    fields: &[Field],
    extras: &mut Extras,
) -> io::Result<Map<String, Json>> {
    let mut reader = Bytes::new(records);
    let mut document = Map::new();
    read_records(&mut reader, node, fields, &mut document, extras)?;
    Ok(document)
}

fn read_records(
    reader: &mut Bytes,
    node: Node,
    fields: &[Field],
    document: &mut Map<String, Json>,
    extras: &mut Extras,
) -> io::Result<()> {
    while reader.remaining() > 0 {
        let size = reader.u32()? as usize;
        let tag_bytes = reader.take(4)?;
        let tag = String::from_utf8_lossy(tag_bytes);
        let tag = tag.trim_end_matches('\0');
        if size < 4 {
            return Err(invalid(format!("record {tag:?} has size {size}")));
        }
        if tag == "ENDB" {
            return Ok(());
        }
        let field = fields.iter().find(|field| field.tag == tag);
        let value_type = field.and_then(|field| field.value);
        match tag {
            // the script follows the tag outside of its record
            "CODE" => {
                let script = reader.sized()?;
                if let (Some(field), Some(Value::Script)) = (field, value_type) {
                    extras.script = Some(utf8_or_cp1252(script));
                    assign(
                        document,
                        field.name,
                        Json::from(super::super::read::SCRIPT),
                        false,
                    );
                }
                continue;
            }
            // so does a font descriptor
            "FONT" if size == 4 => {
                let font = read_font(reader)?;
                if let (Some(field), Some(Value::Font)) = (field, value_type) {
                    assign(document, field.name, font, false);
                }
                continue;
            }
            _ => {}
        }
        if let Some(Value::Objects(sub_node)) = value_type {
            // a sub object either fills its record or follows a record of
            // only its tag, up to its own ENDB
            let mut sub = Map::new();
            let sub_fields = sub_node.fields();
            if size == 4 {
                read_records(reader, sub_node, sub_fields, &mut sub, extras)?;
            } else {
                let mut inner = Bytes::new(reader.take(size - 4)?);
                read_records(&mut inner, sub_node, sub_fields, &mut sub, extras)?;
            }
            if let Some(field) = field {
                assign(document, field.name, Json::Object(sub), true);
            }
            continue;
        }
        let data = reader.take(size - 4)?;
        let (Some(field), Some(value_type)) = (field, value_type) else {
            continue;
        };
        let value = decode(node, field.tag, value_type, data)?;
        assign(document, field.name, value, node.is_repeatable(field.tag));
    }
    Ok(())
}

fn read_font(reader: &mut Bytes) -> io::Result<Json> {
    let _version = reader.u8()?;
    let charset = reader.u16()?;
    let attributes = reader.u8()?;
    let weight = reader.u16()?;
    let size = reader.u32()?;
    let name_len = usize::from(reader.u8()?);
    let name = utf8_or_cp1252(reader.take(name_len)?);
    let mut font = Map::new();
    font.insert("name".into(), Json::from(name));
    font.insert("size".into(), Json::from(size));
    font.insert("weight".into(), Json::from(weight));
    font.insert("charset".into(), Json::from(charset));
    font.insert("italic".into(), Json::from(attributes & 0x02 != 0));
    font.insert("underline".into(), Json::from(attributes & 0x04 != 0));
    font.insert("strikethrough".into(), Json::from(attributes & 0x08 != 0));
    Ok(Json::Object(font))
}

fn floats(data: &[u8], names: &[&str]) -> io::Result<Json> {
    let mut reader = Bytes::new(data);
    let mut object = Map::new();
    for name in names {
        object.insert(name.to_string(), float(reader.f32()?));
    }
    Ok(Json::Object(object))
}

/// A float as nlohmann holds it: widened to double, `null` when not finite
pub(super) fn float(value: f32) -> Json {
    serde_json::Number::from_f64(f64::from(value)).map_or(Json::Null, Json::Number)
}

fn decode(node: Node, tag: &str, value_type: Value, data: &[u8]) -> io::Result<Json> {
    let mut reader = Bytes::new(data);
    Ok(match value_type {
        Value::Bool => Json::from(reader.i32()? != 0),
        Value::Int => Json::from(reader.i32()?),
        Value::UInt => Json::from(reader.u32()?),
        Value::Float => float(reader.f32()?),
        Value::String => Json::from(utf8_or_cp1252(reader.sized()?)),
        Value::WideString => {
            let bytes = reader.sized()?;
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                .collect();
            Json::from(String::from_utf16_lossy(&units))
        }
        Value::Vector2 => floats(data, &["x", "y"])?,
        Value::Vector3 => floats(data, &["x", "y", "z"])?,
        Value::Vector4 => floats(data, &["x", "y", "z", "w"])?,
        Value::Raw if node == Node::RenderProbe && tag == "RPLA" && data.len() == 16 => {
            floats(data, &["x", "y", "z", "w"])?
        }
        // raw blocks become little endian 32-bit integers, or bytes
        Value::Raw if data.len().is_multiple_of(4) => Json::Array(
            data.as_chunks::<4>()
                .0
                .iter()
                .map(|word| Json::from(u32::from_le_bytes([word[0], word[1], word[2], word[3]])))
                .collect(),
        ),
        Value::Raw => Json::Array(data.iter().map(|&b| Json::from(b)).collect()),
        Value::Font | Value::Script | Value::Objects(_) => {
            return Err(invalid(format!("{tag} is not a plain record")));
        }
    })
}

/// Puts a value at a field name, a dotted name nesting it in grouping
/// objects (`desktop_view.fov`); a repeated field collects its values in
/// an array (`AssignField`, `FieldSlot`)
fn assign(document: &mut Map<String, Json>, name: &str, value: Json, repeatable: bool) {
    let mut slot = document;
    let mut parts = name.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if repeatable {
                let entry = slot
                    .entry(part.to_string())
                    .or_insert_with(|| Json::Array(Vec::new()));
                if !entry.is_array() {
                    *entry = Json::Array(Vec::new());
                }
                if let Json::Array(values) = entry {
                    values.push(value);
                }
            } else {
                slot.insert(part.to_string(), value);
            }
            return;
        }
        let entry = slot
            .entry(part.to_string())
            .or_insert_with(|| Json::Object(Map::new()));
        if !entry.is_object() {
            *entry = Json::Object(Map::new());
        }
        let Json::Object(next) = entry else {
            return;
        };
        slot = next;
    }
}

/// Orders the fields of a document, and of its sub documents, like
/// vpinball's `SortNodeFields`: by the declaration order of the field
/// table, a grouping object by its first field, fields the table does not
/// know after them in their order, `$type` first
pub(super) fn sort_fields(
    document: &mut Map<String, Json>,
    node: Node,
    fields: &[Field],
    prefix: &str,
) {
    let path_of = |key: &str| {
        if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        }
    };
    for (key, value) in document.iter_mut() {
        let path = path_of(key);
        // a name mapped twice resolves to its last field, as in vpinball's
        // name to field id map (`dragpoints`: DPNT, not the legacy PNTS)
        let field = fields.iter().rev().find(|field| field.name == path);
        let sub_node = field.and_then(|field| match field.value {
            Some(Value::Objects(sub_node)) => Some(sub_node),
            _ => None,
        });
        let (sub_node, sub_fields, sub_prefix) = match sub_node {
            Some(sub_node) => (sub_node, sub_node.fields(), String::new()),
            None => (node, fields, path),
        };
        match value {
            Json::Object(object) => sort_fields(object, sub_node, sub_fields, &sub_prefix),
            Json::Array(values) => {
                for value in values {
                    if let Json::Object(object) = value {
                        sort_fields(object, sub_node, sub_fields, &sub_prefix);
                    }
                }
            }
            _ => {}
        }
    }
    let rank = |key: &str| {
        let path = path_of(key);
        let dotted = format!("{path}.");
        fields
            .iter()
            .position(|field| field.name == path || field.name.starts_with(&dotted))
            .unwrap_or(fields.len())
    };
    let mut entries: Vec<(String, Json)> = std::mem::take(document).into_iter().collect();
    entries.sort_by_key(|(key, _)| if key == "$type" { 0 } else { rank(key) + 1 });
    document.extend(entries);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn text_is_utf8_or_windows_1252() {
        assert_eq!(utf8_or_cp1252("Mélo €".as_bytes()), "Mélo €");
        assert_eq!(utf8_or_cp1252(b"M\xe9lo \x80"), "Mélo €");
        assert_eq!(
            utf8_or_cp1252(b"\x81\x8d\x8f\x90\x9d"),
            "\u{81}\u{8d}\u{8f}\u{90}\u{9d}"
        );
    }
}
