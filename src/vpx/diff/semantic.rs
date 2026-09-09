//! Comparing two tables in the terms a table author uses.
//!
//! Where [`diff`](super::diff) compares two files and reports which stream
//! and record differ, [`diff`] compares two parsed [`VPX`] tables and
//! reports what changed: the wall that was removed, the sound that was
//! replaced, the property that has a new value.
//!
//! Every named entity (game item, image, sound, font, material, render
//! probe, collection) is paired by name, so reordering the items in a
//! file, which shifts every stream index, reports one
//! [`Change::Reordered`] instead of a difference for every stream.
//! Properties are compared on the same JSON model the expanded directory
//! format uses, so field names match those files. Media is compared as
//! content: an image that was re-encoded to the same pixels says so, a
//! sound reports its format and duration, a primitive its mesh size.
//!
//! Differences that are not edits are left out: floats that moved by
//! less than a millionth, a mesh whose triangles were only reordered, and
//! properties an older file version did not store yet where the newer
//! file holds their default.
//!
//! The script is only reported as changed, with a line count; a text diff
//! of the two scripts is left to the caller, which has both tables.
//!
//! ```no_run
//! use vpin::vpx;
//! use vpin::vpx::diff::semantic;
//!
//! # fn main() -> std::io::Result<()> {
//! let original = vpx::read(std::path::Path::new("original.vpx"))?;
//! let modified = vpx::read(std::path::Path::new("modified.vpx"))?;
//! for change in semantic::diff(&original, &modified) {
//!     println!("{change}");
//! }
//! # Ok(())
//! # }
//! ```

use super::super::VPX;
use super::super::collection::Collection;
use super::super::font::FontData;
use super::super::gamedata::GameData;
use super::super::gameitem::GameItemEnum;
use super::super::gameitem::primitive::Primitive;
use super::super::image::{ImageData, ImageDataJson};
use super::super::jsonmodel;
use super::super::lzw::from_lzw_blocks;
use super::super::material::{Material, MaterialType, SaveMaterial, SavePhysicsMaterial};
use super::super::math::dequantize_u8;
use super::super::obj::VpxFace;
use super::super::renderprobe::RenderProbeJson;
use super::super::sound::{SoundData, SoundDataJson};
use super::zlib_decompress;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// When more elements of an array changed than this, the array is reported
/// as a whole instead of per element
const MAX_ARRAY_ELEMENT_CHANGES: usize = 3;

/// Arrays of scalars up to this length are shown in full when their length
/// changed
const MAX_INLINE_ARRAY_LEN: usize = 8;

/// Floats that differ by less than this fraction are the same value; the
/// model stores single precision and different vpinball builds round the
/// last bit differently
const FLOAT_TOLERANCE: f64 = 1e-6;

/// A single change between two tables, see [`diff`]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    /// An entity only exists in the modified table
    Added(Entity),
    /// An entity only exists in the original table
    Removed(Entity),
    /// An entity exists in both tables with different properties
    Changed {
        entity: Entity,
        fields: Vec<FieldChange>,
    },
    /// The entities of a kind appear in a different order. Only reported
    /// for game items, where the order is part of the table
    Reordered(EntityKind),
    /// The script text differs. The counts are the lines that only appear
    /// on one side; both zero means only the line endings changed
    Script {
        lines_added: usize,
        lines_removed: usize,
    },
}

/// What a [`Change`] is about
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Entity {
    /// The table info: name, author, version, description, screenshot
    TableInfo,
    /// The table wide settings: playfield size, camera, physics, rendering
    TableSettings,
    /// A game item, identified by its type and name
    GameItem {
        type_name: String,
        name: String,
    },
    Image {
        name: String,
    },
    Sound {
        name: String,
    },
    Font {
        name: String,
    },
    Material {
        name: String,
    },
    RenderProbe {
        name: String,
    },
    Collection {
        name: String,
    },
}

/// The kind of [`Entity`] a [`Change::Reordered`] is about
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntityKind {
    GameItem,
    Image,
    Sound,
    Font,
    Material,
    RenderProbe,
    Collection,
}

/// One property of an entity that differs.
///
/// `field` is the property path in the JSON model of the expanded
/// directory format, for example `top_material` or `drag_points[2].x`.
/// The values are rendered for display; `None` means the property is
/// absent or null on that side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: String,
    pub original: Option<String>,
    pub modified: Option<String>,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entity::TableInfo => write!(f, "table info"),
            Entity::TableSettings => write!(f, "table settings"),
            Entity::GameItem { type_name, name } => write!(f, "{type_name} {name:?}"),
            Entity::Image { name } => write!(f, "image {name:?}"),
            Entity::Sound { name } => write!(f, "sound {name:?}"),
            Entity::Font { name } => write!(f, "font {name:?}"),
            Entity::Material { name } => write!(f, "material {name:?}"),
            Entity::RenderProbe { name } => write!(f, "render probe {name:?}"),
            Entity::Collection { name } => write!(f, "collection {name:?}"),
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            EntityKind::GameItem => "game item",
            EntityKind::Image => "image",
            EntityKind::Sound => "sound",
            EntityKind::Font => "font",
            EntityKind::Material => "material",
            EntityKind::RenderProbe => "render probe",
            EntityKind::Collection => "collection",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for FieldChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn value(value: &Option<String>) -> &str {
            value.as_deref().unwrap_or("(none)")
        }
        write!(
            f,
            "{} {} -> {}",
            self.field.replace('_', " "),
            value(&self.original),
            value(&self.modified)
        )
    }
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Added(entity) => write!(f, "{entity} added"),
            Change::Removed(entity) => write!(f, "{entity} removed"),
            Change::Changed { entity, fields } => {
                write!(f, "{entity}:")?;
                for (index, field) in fields.iter().enumerate() {
                    let separator = if index == 0 { "" } else { "," };
                    write!(f, "{separator} {field}")?;
                }
                Ok(())
            }
            Change::Reordered(kind) => write!(f, "{kind}s reordered"),
            Change::Script {
                lines_added: 0,
                lines_removed: 0,
            } => write!(f, "script changed (line endings only)"),
            Change::Script {
                lines_added,
                lines_removed,
            } => write!(f, "script changed (+{lines_added} -{lines_removed} lines)"),
        }
    }
}

/// Compares two tables and returns what changed, empty when they hold the
/// same table.
///
/// The changes are grouped by kind: table info, table settings, script,
/// materials, render probes, images, sounds, fonts, collections and game
/// items. Within a kind the removed and changed entities come in the
/// order of the original, then the added ones in the order of the
/// modified table.
pub fn diff(original: &VPX, modified: &VPX) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_table_info(original, modified, &mut changes);
    diff_table_settings(original, modified, &mut changes);
    diff_script(&original.gamedata, &modified.gamedata, &mut changes);
    let mut named = NamedDiff {
        // an older file version stores fewer properties; the newer file
        // then holds what vpinball filled in, which is not an edit
        ignore_new_defaults: original.version != modified.version,
        changes: &mut changes,
    };
    named.diff(
        EntityKind::Material,
        &materials(&original.gamedata),
        &materials(&modified.gamedata),
        |(name, _)| Entity::Material { name: name.clone() },
        |(_, a), (_, b)| json_leaves(a, b),
    );
    named.diff(
        EntityKind::RenderProbe,
        &render_probes(&original.gamedata),
        &render_probes(&modified.gamedata),
        |(name, _)| Entity::RenderProbe { name: name.clone() },
        |(_, a), (_, b)| json_leaves(a, b),
    );
    named.diff(
        EntityKind::Image,
        &original.images,
        &modified.images,
        |image| Entity::Image {
            name: image.name.clone(),
        },
        image_leaves,
    );
    named.diff(
        EntityKind::Sound,
        &original.sounds,
        &modified.sounds,
        |sound| Entity::Sound {
            name: sound.name.clone(),
        },
        sound_leaves,
    );
    named.diff(
        EntityKind::Font,
        &original.fonts,
        &modified.fonts,
        |font| Entity::Font {
            name: font.name.clone(),
        },
        font_leaves,
    );
    named.diff(
        EntityKind::Collection,
        &original.collections,
        &modified.collections,
        |collection| Entity::Collection {
            name: collection.name.clone(),
        },
        collection_leaves,
    );
    named.diff(
        EntityKind::GameItem,
        &original.gameitems,
        &modified.gameitems,
        |item| Entity::GameItem {
            type_name: item.type_name(),
            name: item.name().to_string(),
        },
        game_item_leaves,
    );
    changes
}

/// One differing property before the per entity clean up: the array it
/// sits in, when it does, lets many changed elements collapse into one
/// line
struct Leaf {
    change: FieldChange,
    /// The outermost array on the path: its path, the element index and
    /// the array length
    array: Option<(String, usize, usize)>,
}

impl Leaf {
    fn new(field: impl Into<String>, original: Option<String>, modified: Option<String>) -> Self {
        Leaf {
            change: FieldChange {
                field: field.into(),
                original,
                modified,
            },
            array: None,
        }
    }
}

struct NamedDiff<'a> {
    ignore_new_defaults: bool,
    changes: &'a mut Vec<Change>,
}

impl NamedDiff<'_> {
    /// Pairs the entities of two lists by name and reports the removed,
    /// changed and added ones. A second entity with the same name is
    /// paired with the second one on the other side, so duplicate names
    /// still line up. For game items a different order of the paired
    /// entities is reported as well.
    fn diff<T>(
        &mut self,
        kind: EntityKind,
        original: &[T],
        modified: &[T],
        entity: impl Fn(&T) -> Entity,
        leaves: impl Fn(&T, &T) -> Vec<Leaf>,
    ) {
        let keys_original = numbered(original, &entity);
        let keys_modified = numbered(modified, &entity);
        let index_modified: HashMap<&(Entity, usize), usize> = keys_modified
            .iter()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect();
        let index_original: HashMap<&(Entity, usize), usize> = keys_original
            .iter()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect();

        // the leaves of every paired entity, `None` for a removed one, so
        // the report keeps the order of the original
        let mut paired: Vec<Option<Vec<Leaf>>> = keys_original
            .iter()
            .enumerate()
            .map(|(index, key)| {
                index_modified
                    .get(key)
                    .map(|&other| leaves(&original[index], &modified[other]))
            })
            .collect();
        if self.ignore_new_defaults {
            drop_new_defaults(paired.iter_mut().flatten());
        }
        for (key, leaves) in keys_original.iter().zip(paired) {
            let Some(leaves) = leaves else {
                self.changes.push(Change::Removed(key.0.clone()));
                continue;
            };
            let fields = finish(leaves);
            if !fields.is_empty() {
                self.changes.push(Change::Changed {
                    entity: key.0.clone(),
                    fields,
                });
            }
        }
        for key in &keys_modified {
            if !index_original.contains_key(key) {
                self.changes.push(Change::Added(key.0.clone()));
            }
        }

        if kind == EntityKind::GameItem {
            let common_original = keys_original
                .iter()
                .filter(|key| index_modified.contains_key(key));
            let common_modified = keys_modified
                .iter()
                .filter(|key| index_original.contains_key(key));
            if !common_original.eq(common_modified) {
                self.changes.push(Change::Reordered(kind));
            }
        }
    }
}

/// The entity of every element with a running number per name, so
/// duplicates get distinct keys
fn numbered<T>(items: &[T], entity: &impl Fn(&T) -> Entity) -> Vec<(Entity, usize)> {
    let mut seen: HashMap<Entity, usize> = HashMap::new();
    items
        .iter()
        .map(|item| {
            let entity = entity(item);
            let count = seen.entry(entity.clone()).or_insert(0);
            let key = (entity, *count);
            *count += 1;
            key
        })
        .collect()
}

/// Properties vpinball fills in when it upgrades a file to the 10.7 layer
/// system, derived from the numeric layer of every item
const DERIVED_ON_UPGRADE: &[&str] = &["editor_layer_name", "editor_layer_visibility"];

/// Drops properties that are absent on one side because the other file
/// version did not store them yet, and hold what vpinball filled in: a
/// value that most entities of the kind share (the default), or one of
/// the layer properties derived on upgrade. A property only one entity
/// gained, or with a value few entities share, is an edit and stays.
fn drop_new_defaults<'a>(paired: impl Iterator<Item = &'a mut Vec<Leaf>>) {
    let paired: Vec<&mut Vec<Leaf>> = paired.collect();
    // per property path (without array indexes) and side: how many
    // entities gained it with each value
    let mut gained: HashMap<(String, bool), HashMap<String, usize>> = HashMap::new();
    for leaves in &paired {
        let mut seen_here: HashSet<((String, bool), String)> = HashSet::new();
        for leaf in leaves.iter() {
            let Some((key, value)) = new_property(leaf) else {
                continue;
            };
            if seen_here.insert((key.clone(), value.clone())) {
                *gained
                    .entry(key)
                    .or_default()
                    .entry(value.clone())
                    .or_default() += 1;
            }
        }
    }
    let common: HashMap<&(String, bool), &String> = gained
        .iter()
        .filter_map(|(key, values)| {
            let (value, entities) = values.iter().max_by_key(|(_, count)| **count)?;
            (*entities >= 2).then_some((key, value))
        })
        .collect();
    for leaves in paired {
        leaves.retain(|leaf| {
            let Some((key, value)) = new_property(leaf) else {
                return true;
            };
            let derived = DERIVED_ON_UPGRADE
                .iter()
                .any(|property| key.0.rsplit('.').next() == Some(property));
            !derived && common.get(&key) != Some(&value)
        });
    }
}

/// The key and value of a property that is absent on one side; the key
/// carries which side so an upgrade and a downgrade do not mix
fn new_property(leaf: &Leaf) -> Option<((String, bool), &String)> {
    let path = without_indexes(&leaf.change.field);
    match (&leaf.change.original, &leaf.change.modified) {
        (None, Some(value)) => Some(((path, true), value)),
        (Some(value), None) => Some(((path, false), value)),
        _ => None,
    }
}

/// `drag_points[3].x` as `drag_points[].x`
fn without_indexes(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut in_index = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_index = true;
                result.push(c);
            }
            ']' => {
                in_index = false;
                result.push(c);
            }
            _ if in_index => {}
            _ => result.push(c),
        }
    }
    result
}

/// The reportable changes of one entity: properties that only went from
/// absent to a zero value are dropped, and an array with many changed
/// elements is reported as one line
fn finish(leaves: Vec<Leaf>) -> Vec<FieldChange> {
    let mut fields = Vec::new();
    let mut leaves = leaves
        .into_iter()
        .filter(|leaf| !absent_to_zero(&leaf.change))
        .peekable();
    while let Some(leaf) = leaves.next() {
        let Some((array_path, index, len)) = leaf.array.clone() else {
            fields.push(leaf.change);
            continue;
        };
        let mut group = vec![leaf.change];
        let mut indexes: HashSet<usize> = HashSet::from([index]);
        while let Some(next) = leaves.next_if(|next| {
            next.array
                .as_ref()
                .is_some_and(|(path, _, _)| *path == array_path)
        }) {
            if let Some((_, index, _)) = next.array {
                indexes.insert(index);
            }
            group.push(next.change);
        }
        if indexes.len() > MAX_ARRAY_ELEMENT_CHANGES {
            fields.push(FieldChange {
                field: array_path,
                original: Some(count(len, "item")),
                modified: Some(format!("{}, {} changed", count(len, "item"), indexes.len())),
            });
        } else {
            fields.append(&mut group);
        }
    }
    fields
}

/// A property that is absent on one side and zero, false or empty on the
/// other is a difference in what the file stores, not in the table
fn absent_to_zero(change: &FieldChange) -> bool {
    const ZERO: &[&str] = &["0", "false", "\"\"", "[]", "{}"];
    match (&change.original, &change.modified) {
        (None, Some(value)) | (Some(value), None) => ZERO.contains(&value.as_str()),
        _ => false,
    }
}

fn diff_table_info(original: &VPX, modified: &VPX, changes: &mut Vec<Change>) {
    let mut json_original = jsonmodel::info_to_json(&original.info, &original.custominfotags);
    let mut json_modified = jsonmodel::info_to_json(&modified.info, &modified.custominfotags);
    // the order of the custom properties is not something an author sees
    remove_keys(&mut json_original, &["properties_order"]);
    remove_keys(&mut json_modified, &["properties_order"]);
    let mut leaves = json_leaves(&json_original, &json_modified);
    if original.info.screenshot != modified.info.screenshot {
        leaves.push(Leaf::new(
            "screenshot",
            original.info.screenshot.as_deref().map(describe_screenshot),
            modified.info.screenshot.as_deref().map(describe_screenshot),
        ));
    }
    let fields = finish(leaves);
    if !fields.is_empty() {
        changes.push(Change::Changed {
            entity: Entity::TableInfo,
            fields,
        });
    }
}

fn describe_screenshot(data: &[u8]) -> String {
    let format = if data.starts_with(b"\x89PNG") {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8]) {
        "jpeg"
    } else {
        "image"
    };
    format!("{format}, {}", describe_bytes(data.len()))
}

fn diff_table_settings(original: &VPX, modified: &VPX, changes: &mut Vec<Change>) {
    let mut leaves = Vec::new();
    if original.version != modified.version {
        leaves.push(Leaf::new(
            "file_version",
            Some(original.version.to_string()),
            Some(modified.version.to_string()),
        ));
    }
    let mut json_original = jsonmodel::game_data_to_json(&original.gamedata);
    let mut json_modified = jsonmodel::game_data_to_json(&modified.gamedata);
    // a marker for files written by a few 10.8 beta builds, not a setting
    remove_keys(&mut json_original, &["is_10_8_0_beta1_to_beta4"]);
    remove_keys(&mut json_modified, &["is_10_8_0_beta1_to_beta4"]);
    leaves.extend(json_leaves(&json_original, &json_modified));
    let fields = finish(leaves);
    if !fields.is_empty() {
        changes.push(Change::Changed {
            entity: Entity::TableSettings,
            fields,
        });
    }
}

fn diff_script(original: &GameData, modified: &GameData, changes: &mut Vec<Change>) {
    if original.code.string == modified.code.string {
        return;
    }
    let (lines_added, lines_removed) = line_delta(&original.code.string, &modified.code.string);
    changes.push(Change::Script {
        lines_added,
        lines_removed,
    });
}

/// The number of lines that only appear in `modified` and only in
/// `original`, counting repeated lines as often as they occur. This is a
/// summary, not a diff: a moved line counts for nothing.
fn line_delta(original: &str, modified: &str) -> (usize, usize) {
    let mut counts: HashMap<&str, isize> = HashMap::new();
    for line in original.lines() {
        *counts.entry(line).or_default() -= 1;
    }
    for line in modified.lines() {
        *counts.entry(line).or_default() += 1;
    }
    let added = counts.values().filter(|count| **count > 0).sum::<isize>();
    let removed = counts.values().filter(|count| **count < 0).sum::<isize>();
    (added.unsigned_abs(), removed.unsigned_abs())
}

/// The materials as name and JSON properties. Tables from 10.8 on carry
/// full materials; older ones split them in a quantized render part and a
/// physics part, which are converted to the full form so both file
/// versions compare on the same properties.
fn materials(gamedata: &GameData) -> Vec<(String, Value)> {
    if let Some(materials) = &gamedata.materials {
        return materials
            .iter()
            .map(|material| (material.name.clone(), to_value(material)))
            .collect();
    }
    let physics: HashMap<&str, &SavePhysicsMaterial> = gamedata
        .materials_physics_old
        .iter()
        .flatten()
        .map(|material| (material.name.as_str(), material))
        .collect();
    gamedata
        .materials_old
        .iter()
        .map(|material| {
            let full = full_material(material, physics.get(material.name.as_str()).copied());
            (material.name.clone(), to_value(&full))
        })
        .collect()
}

/// The inverse of the quantization vpinball applies when it saves a
/// material in the pre 10.8 layout
fn full_material(material: &SaveMaterial, physics: Option<&SavePhysicsMaterial>) -> Material {
    let mut full = Material::default();
    full.name = material.name.clone();
    full.type_ = if material.is_metal {
        MaterialType::Metal
    } else {
        MaterialType::Basic
    };
    full.wrap_lighting = material.wrap_lighting;
    full.roughness = material.roughness;
    // stored as '255 -' for compatibility with older tables
    full.glossy_image_lerp = dequantize_u8(8, 255 - material.glossy_image_lerp);
    full.thickness = dequantize_u8(8, material.thickness);
    full.edge = material.edge;
    full.edge_alpha = dequantize_u8(7, material.opacity_active_edge_alpha >> 1);
    full.opacity = material.opacity;
    full.base_color = material.base_color;
    full.glossy_color = material.glossy_color;
    full.clearcoat_color = material.clearcoat_color;
    full.opacity_active = material.opacity_active_edge_alpha & 1 == 1;
    if let Some(physics) = physics {
        full.elasticity = physics.elasticity;
        full.elasticity_falloff = physics.elasticity_falloff;
        full.friction = physics.friction;
        full.scatter_angle = physics.scatter_angle;
    }
    full
}

fn render_probes(gamedata: &GameData) -> Vec<(String, Value)> {
    gamedata
        .render_probes
        .iter()
        .flatten()
        .map(|probe| {
            let mut json = to_value(RenderProbeJson::from_renderprobe(probe));
            // bytes vpinball writes past the end of the record
            remove_keys(&mut json, &["trailing_data"]);
            (probe.render_probe.name.clone(), json)
        })
        .collect()
}

fn image_leaves(original: &ImageData, modified: &ImageData) -> Vec<Leaf> {
    // the internal names and hash are derived from the content, the
    // dedup name is an artifact of the expanded format
    const NOISE: &[&str] = &[
        "internal_name",
        "jpeg_name",
        "jpeg_path",
        "jpeg_internal_name",
        "md5_hash",
        "name_dedup",
    ];
    let mut json_original = to_value(ImageDataJson::from_image_data(original));
    let mut json_modified = to_value(ImageDataJson::from_image_data(modified));
    remove_keys(&mut json_original, NOISE);
    remove_keys(&mut json_modified, NOISE);
    let mut leaves = json_leaves(&json_original, &json_modified);
    if (original.width, original.height) != (modified.width, modified.height) {
        leaves.push(Leaf::new(
            "size",
            Some(format!("{}x{}", original.width, original.height)),
            Some(format!("{}x{}", modified.width, modified.height)),
        ));
    }
    if let Some(leaf) = image_content_change(original, modified) {
        leaves.push(leaf);
    }
    leaves
}

enum ImageContent<'a> {
    /// A png, jpeg, webp, ... as imported
    Encoded(&'a [u8]),
    /// A lzw compressed bitmap
    Bitmap(&'a [u8]),
    /// A reference to a file outside the table, or no data at all
    None,
}

fn image_content(image: &ImageData) -> ImageContent<'_> {
    if let Some(jpeg) = &image.jpeg {
        ImageContent::Encoded(&jpeg.data)
    } else if let Some(bits) = &image.bits {
        ImageContent::Bitmap(&bits.lzw_compressed_data)
    } else {
        ImageContent::None
    }
}

fn image_content_change(original: &ImageData, modified: &ImageData) -> Option<Leaf> {
    let mut note = "";
    match (image_content(original), image_content(modified)) {
        (ImageContent::None, ImageContent::None) => return None,
        (ImageContent::Encoded(a), ImageContent::Encoded(b)) => {
            if a == b {
                return None;
            }
            if same_pixels(a, b) {
                note = " (re-encoded, same pixels)";
            }
        }
        (ImageContent::Bitmap(a), ImageContent::Bitmap(b)) => {
            if a == b {
                return None;
            }
            if let (Ok(a), Ok(b)) = (from_lzw_blocks(a), from_lzw_blocks(b))
                && a == b
            {
                // different lzw encoders, same bitmap
                return None;
            }
        }
        _ => {}
    }
    Some(Leaf::new(
        "data",
        Some(describe_image(original)),
        Some(format!("{}{note}", describe_image(modified))),
    ))
}

/// Whether two encoded images decode to the same pixels
fn same_pixels(a: &[u8], b: &[u8]) -> bool {
    match (::image::load_from_memory(a), ::image::load_from_memory(b)) {
        (Ok(a), Ok(b)) => {
            a.width() == b.width() && a.height() == b.height() && a.to_rgba8() == b.to_rgba8()
        }
        _ => false,
    }
}

fn describe_image(image: &ImageData) -> String {
    let size = format!("{}x{}", image.width, image.height);
    match image_content(image) {
        ImageContent::Encoded(data) => format!(
            "{} {size}, {}",
            image.ext().to_lowercase(),
            describe_bytes(data.len())
        ),
        ImageContent::Bitmap(data) => {
            format!("bitmap {size}, {} compressed", describe_bytes(data.len()))
        }
        ImageContent::None if image.is_link() => "link".to_string(),
        ImageContent::None => "no data".to_string(),
    }
}

fn sound_leaves(original: &SoundData, modified: &SoundData) -> Vec<Leaf> {
    // not used by vpinball, kept for compatibility
    const NOISE: &[&str] = &["internal_name", "name_dedup"];
    let mut json_original = to_value(SoundDataJson::from_sound_data(original));
    let mut json_modified = to_value(SoundDataJson::from_sound_data(modified));
    remove_keys(&mut json_original, NOISE);
    remove_keys(&mut json_modified, NOISE);
    let mut leaves = json_leaves(&json_original, &json_modified);
    if original.data != modified.data || original.wave_form != modified.wave_form {
        leaves.push(Leaf::new(
            "data",
            Some(describe_sound(original)),
            Some(describe_sound(modified)),
        ));
    }
    leaves
}

fn describe_sound(sound: &SoundData) -> String {
    let ext = sound
        .path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        // a sound without extension is a wav
        .unwrap_or_else(|| "wav".to_string());
    let size = describe_bytes(sound.data.len());
    if ext != "wav" {
        return format!("{ext}, {size}");
    }
    let wave = &sound.wave_form;
    let format = match wave.format_tag {
        1 => "pcm".to_string(),
        3 => "float".to_string(),
        other => format!("format {other}"),
    };
    let channels = match wave.channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        other => format!("{other} channels"),
    };
    let mut description = format!(
        "wav {format} {} Hz {}-bit {channels}",
        wave.samples_per_sec, wave.bits_per_sample
    );
    if wave.avg_bytes_per_sec > 0 {
        let seconds = sound.data.len() as f64 / f64::from(wave.avg_bytes_per_sec);
        description.push_str(&format!(", {seconds:.2} s"));
    }
    format!("{description}, {size}")
}

fn font_leaves(original: &FontData, modified: &FontData) -> Vec<Leaf> {
    let mut leaves = Vec::new();
    if original.path != modified.path {
        leaves.push(Leaf::new(
            "path",
            Some(quote(&original.path)),
            Some(quote(&modified.path)),
        ));
    }
    if original.data != modified.data {
        leaves.push(Leaf::new(
            "data",
            Some(describe_bytes(original.data.len())),
            Some(describe_bytes(modified.data.len())),
        ));
    }
    leaves
}

fn collection_leaves(original: &Collection, modified: &Collection) -> Vec<Leaf> {
    fn to_json(collection: &Collection) -> Value {
        json!({
            "fire_events": collection.fire_events,
            "stop_single_events": collection.stop_single_events,
            "group_elements": collection.group_elements,
        })
    }
    let mut leaves = Vec::new();
    if original.items != modified.items {
        leaves.push(Leaf::new(
            "items",
            Some(count(original.items.len(), "item")),
            Some(describe_item_list_change(&original.items, &modified.items)),
        ));
    }
    leaves.extend(json_leaves(&to_json(original), &to_json(modified)));
    leaves
}

/// The new item count with the names that came and went, or a note when
/// only the order changed
fn describe_item_list_change(original: &[String], modified: &[String]) -> String {
    let set_original: HashSet<&String> = original.iter().collect();
    let set_modified: HashSet<&String> = modified.iter().collect();
    let added: Vec<String> = modified
        .iter()
        .filter(|item| !set_original.contains(item))
        .map(|item| quote(item))
        .collect();
    let removed: Vec<String> = original
        .iter()
        .filter(|item| !set_modified.contains(item))
        .map(|item| quote(item))
        .collect();
    let mut notes = Vec::new();
    if !added.is_empty() {
        notes.push(format!("added {}", added.join(" ")));
    }
    if !removed.is_empty() {
        notes.push(format!("removed {}", removed.join(" ")));
    }
    if notes.is_empty() {
        notes.push("reordered".to_string());
    }
    format!("{} ({})", count(modified.len(), "item"), notes.join(", "))
}

fn game_item_leaves(original: &GameItemEnum, modified: &GameItemEnum) -> Vec<Leaf> {
    let mut leaves = json_leaves(&game_item_json(original), &game_item_json(modified));
    if let (GameItemEnum::Primitive(original), GameItemEnum::Primitive(modified)) =
        (original, modified)
        && let Some(leaf) = mesh_change(original, modified)
    {
        leaves.push(leaf);
    }
    leaves
}

/// The properties of a game item without the enum wrapper serde adds,
/// plus the editor attributes the expanded format keeps in its item list
/// rather than in the item JSON
fn game_item_json(item: &GameItemEnum) -> Value {
    let mut json = to_value(item);
    if let Value::Object(map) = &mut json
        && map.len() == 1
        && let Some((_, inner)) = map.iter_mut().next()
    {
        json = match inner.take() {
            // a tuple variant: the type id and the properties
            Value::Array(mut parts) => parts.pop().unwrap_or(Value::Null),
            inner => inner,
        };
    }
    if let Value::Object(map) = &mut json {
        map.insert("is_locked".to_string(), to_value(item.is_locked()));
        map.insert("editor_layer".to_string(), to_value(item.editor_layer()));
        map.insert(
            "editor_layer_name".to_string(),
            to_value(item.editor_layer_name()),
        );
        map.insert(
            "editor_layer_visibility".to_string(),
            to_value(item.editor_layer_visibility()),
        );
    }
    json
}

/// The mesh is not part of the JSON model (the expanded format keeps it
/// in a separate file). It is compared decoded: the compressed bytes
/// depend on the encoder, and vpinball reorders the triangles of a mesh
/// when it depth sorts them, which leaves the shape as it was.
fn mesh_change(original: &Primitive, modified: &Primitive) -> Option<Leaf> {
    if same_mesh(original, modified) && same_animation(original, modified) {
        return None;
    }
    let description_original = describe_mesh(original);
    let mut description_modified = describe_mesh(modified);
    if description_original == description_modified {
        description_modified.push_str(", different geometry");
    }
    Some(Leaf::new(
        "mesh",
        Some(description_original),
        Some(description_modified),
    ))
}

fn same_mesh(original: &Primitive, modified: &Primitive) -> bool {
    if original.compressed_vertices_data == modified.compressed_vertices_data
        && original.compressed_indices_data == modified.compressed_indices_data
    {
        return true;
    }
    match (original.read_mesh(), modified.read_mesh()) {
        (Ok(Some(a)), Ok(Some(b))) => {
            a.vertices.len() == b.vertices.len()
                && a.vertices
                    .iter()
                    .zip(&b.vertices)
                    .all(|(a, b)| a.vpx_encoded_vertex == b.vpx_encoded_vertex)
                && sorted_faces(&a.indices) == sorted_faces(&b.indices)
        }
        (Ok(None), Ok(None)) => true,
        _ => false,
    }
}

fn sorted_faces(faces: &[VpxFace]) -> Vec<(i64, i64, i64)> {
    let mut sorted: Vec<(i64, i64, i64)> = faces
        .iter()
        .map(|face| (face.i0, face.i1, face.i2))
        .collect();
    sorted.sort_unstable();
    sorted
}

fn same_animation(original: &Primitive, modified: &Primitive) -> bool {
    match (
        &original.compressed_animation_vertices_data,
        &modified.compressed_animation_vertices_data,
    ) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_compressed(a, b))
        }
        _ => false,
    }
}

/// Whether two zlib streams hold the same data; different encoders
/// produce different bytes for the same input
fn same_compressed(original: &[u8], modified: &[u8]) -> bool {
    original == modified
        || matches!(
            (zlib_decompress(original), zlib_decompress(modified)),
            (Ok(a), Ok(b)) if a == b
        )
}

fn describe_mesh(primitive: &Primitive) -> String {
    match (primitive.num_vertices, primitive.num_indices) {
        (None, None) => "no mesh".to_string(),
        (vertices, indices) => {
            let mut description = format!(
                "{} vertices, {} indices",
                vertices.unwrap_or(0),
                indices.unwrap_or(0)
            );
            if let Some(frames) = &primitive.compressed_animation_vertices_data
                && !frames.is_empty()
            {
                description.push_str(&format!(", {} animation frames", frames.len()));
            }
            description
        }
    }
}

/// Serializing the JSON model structs cannot fail; a NaN float becomes null
fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn remove_keys(json: &mut Value, keys: &[&str]) {
    if let Value::Object(map) = json {
        for key in keys {
            map.remove(*key);
        }
    }
}

/// The differing properties of two JSON values, as paths from the root
fn json_leaves(original: &Value, modified: &Value) -> Vec<Leaf> {
    let mut leaves = Vec::new();
    diff_values("", original, modified, None, &mut leaves);
    leaves
}

/// The differing properties of two JSON values after the per entity
/// clean up
#[cfg(test)]
fn json_fields(original: &Value, modified: &Value) -> Vec<FieldChange> {
    finish(json_leaves(original, modified))
}

fn diff_values(
    path: &str,
    original: &Value,
    modified: &Value,
    array: Option<(String, usize, usize)>,
    out: &mut Vec<Leaf>,
) {
    let leaf = |field: String, original: Option<String>, modified: Option<String>| Leaf {
        change: FieldChange {
            field,
            original,
            modified,
        },
        array: array.clone(),
    };
    match (original, modified) {
        (Value::Object(original), Value::Object(modified)) => {
            for (key, value_original) in original {
                let sub_path = join_path(path, key);
                match modified.get(key) {
                    Some(value_modified) => diff_values(
                        &sub_path,
                        value_original,
                        value_modified,
                        array.clone(),
                        out,
                    ),
                    None => out.push(leaf(sub_path, render(value_original), None)),
                }
            }
            for (key, value_modified) in modified {
                if !original.contains_key(key) {
                    out.push(leaf(join_path(path, key), None, render(value_modified)));
                }
            }
        }
        (Value::Array(original), Value::Array(modified)) if original.len() == modified.len() => {
            for (index, (value_original, value_modified)) in
                original.iter().zip(modified).enumerate()
            {
                // the outermost array is the one that collapses
                let array = array
                    .clone()
                    .or_else(|| Some((path.to_string(), index, original.len())));
                diff_values(
                    &format!("{path}[{index}]"),
                    value_original,
                    value_modified,
                    array,
                    out,
                );
            }
        }
        (Value::Number(a), Value::Number(b)) if same_number(a, b) => {}
        _ if original == modified => {}
        _ => out.push(leaf(path.to_string(), render(original), render(modified))),
    }
}

/// Numbers within [`FLOAT_TOLERANCE`] of each other
fn same_number(a: &serde_json::Number, b: &serde_json::Number) -> bool {
    if a == b {
        return true;
    }
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= FLOAT_TOLERANCE * a.abs().max(b.abs()),
        _ => false,
    }
}

fn join_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// A JSON value for display, `None` for null
fn render(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(number) => Some(render_number(number)),
        Value::String(value) => Some(quote(value)),
        Value::Array(items)
            if items.len() <= MAX_INLINE_ARRAY_LEN
                && items
                    .iter()
                    .all(|item| !item.is_object() && !item.is_array()) =>
        {
            Some(value.to_string())
        }
        Value::Array(items) => Some(count(items.len(), "item")),
        Value::Object(_) => Some(value.to_string()),
    }
}

/// A string in quotes, as is: names and Windows paths read better without
/// the escaping a debug format adds. Control characters, which old
/// vpinball builds wrote as uninitialized memory in names, are escaped so
/// a line stays a line.
fn quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        if c.is_control() {
            quoted.extend(c.escape_default());
        } else {
            quoted.push(c);
        }
    }
    quoted.push('"');
    quoted
}

/// Floats in the model are single precision; serde widens them, which
/// turns 0.075 into 0.07500000298023224
fn render_number(number: &serde_json::Number) -> String {
    match number.as_f64() {
        Some(value) if number.is_f64() => {
            let narrowed = value as f32;
            if f64::from(narrowed) == value {
                narrowed.to_string()
            } else {
                value.to_string()
            }
        }
        _ => number.to_string(),
    }
}

/// `1 item`, `2 items`
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn describe_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx;
    use crate::vpx::gameitem::dragpoint::DragPoint;
    use crate::vpx::gameitem::primitive::compress_mesh_data;
    use crate::vpx::gameitem::timer::Timer;
    use crate::vpx::gameitem::wall::Wall;
    use crate::vpx::image::ImageDataJpeg;
    use crate::vpx::model::Vertex3dNoTex2;
    use crate::vpx::sound::{OutputTarget, WaveForm};
    use crate::vpx::version::Version;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn blank_table() -> VPX {
        // embedded because the wasm test runner has no file system access
        let bytes = include_bytes!("../../../testdata/completely_blank_table_10_7_4.vpx");
        vpx::from_bytes(bytes).expect("the test table reads")
    }

    fn wall(name: &str) -> GameItemEnum {
        GameItemEnum::Wall(Wall {
            name: name.to_string(),
            ..Wall::default()
        })
    }

    fn wall_entity(name: &str) -> Entity {
        Entity::GameItem {
            type_name: "Wall".to_string(),
            name: name.to_string(),
        }
    }

    fn png_image(name: &str, data: Vec<u8>) -> ImageData {
        ImageData {
            name: name.to_string(),
            internal_name: None,
            path: format!("C:\\{name}.png"),
            width: 1,
            height: 1,
            link: None,
            alpha_test_value: -1.0,
            is_opaque: None,
            is_signed: None,
            jpeg: Some(ImageDataJpeg {
                path: format!("C:\\{name}.png"),
                name: name.to_string(),
                internal_name: None,
                data,
            }),
            bits: None,
            md5_hash: None,
        }
    }

    fn wav_sound(name: &str, data: Vec<u8>) -> SoundData {
        SoundData {
            name: name.to_string(),
            path: format!("C:\\{name}.wav"),
            wave_form: WaveForm {
                format_tag: 1,
                channels: 1,
                samples_per_sec: 44100,
                avg_bytes_per_sec: 88200,
                block_align: 2,
                bits_per_sample: 16,
                cb_size: 0,
            },
            data,
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        }
    }

    /// A primitive with a mesh of the given vertex x coordinates and
    /// triangles, compressed the way the file stores them
    fn primitive_with_mesh(xs: &[f32], faces: &[(u16, u16, u16)]) -> TestResult<GameItemEnum> {
        let vertices: Vec<u8> = xs
            .iter()
            .flat_map(|&x| {
                Vertex3dNoTex2 {
                    x,
                    y: 0.0,
                    z: 0.0,
                    nx: 0.0,
                    ny: 0.0,
                    nz: 1.0,
                    tu: 0.0,
                    tv: 0.0,
                }
                .to_vpx_bytes()
            })
            .collect();
        let indices: Vec<u8> = faces
            .iter()
            .flat_map(|&(i0, i1, i2)| [i0, i1, i2])
            .flat_map(|index| index.to_le_bytes())
            .collect();
        let compressed_vertices = compress_mesh_data(&vertices)?;
        let compressed_indices = compress_mesh_data(&indices)?;
        Ok(GameItemEnum::Primitive(Box::new(Primitive {
            name: "Prim".to_string(),
            num_vertices: Some(xs.len() as u32),
            compressed_vertices_len: Some(compressed_vertices.len() as u32),
            compressed_vertices_data: Some(compressed_vertices),
            num_indices: Some(faces.len() as u32 * 3),
            compressed_indices_len: Some(compressed_indices.len() as u32),
            compressed_indices_data: Some(compressed_indices),
            ..Primitive::default()
        })))
    }

    #[test]
    fn identical_tables_have_no_changes() {
        let table = blank_table();
        assert_eq!(diff(&table, &table), Vec::new());
    }

    #[test]
    fn a_table_setting_is_reported_by_name() {
        let original = blank_table();
        let mut modified = blank_table();
        modified.gamedata.name = "renamed".to_string();
        modified.gamedata.bg_fov_desktop = 50.5;

        let changes = diff(&original, &modified);
        assert_eq!(
            changes,
            vec![Change::Changed {
                entity: Entity::TableSettings,
                fields: vec![
                    FieldChange {
                        field: "bg_fov_desktop".to_string(),
                        original: Some(original.gamedata.bg_fov_desktop.to_string()),
                        modified: Some("50.5".to_string()),
                    },
                    FieldChange {
                        field: "name".to_string(),
                        original: Some(format!("\"{}\"", original.gamedata.name)),
                        modified: Some("\"renamed\"".to_string()),
                    },
                ],
            }]
        );
        assert_eq!(
            changes[0].to_string(),
            format!(
                "table settings: bg fov desktop {} -> 50.5, name \"{}\" -> \"renamed\"",
                original.gamedata.bg_fov_desktop, original.gamedata.name
            )
        );
    }

    #[test]
    fn table_info_and_script_changes_are_reported() {
        let mut original = blank_table();
        let mut modified = blank_table();
        modified.info.table_rules = Some("no tilting".to_string());
        modified.info.table_name = None;
        original
            .gamedata
            .set_code("Option Explicit\r\nDim x\r\n".to_string());
        modified
            .gamedata
            .set_code("Option Explicit\r\nDim x\r\n' a mod\r\n' another line\r\n".to_string());

        let changes = diff(&original, &modified);
        assert_eq!(
            changes,
            vec![
                Change::Changed {
                    entity: Entity::TableInfo,
                    fields: vec![
                        FieldChange {
                            field: "table_name".to_string(),
                            original: Some(format!(
                                "\"{}\"",
                                original.info.table_name.as_deref().unwrap_or("")
                            )),
                            modified: None,
                        },
                        FieldChange {
                            field: "table_rules".to_string(),
                            original: None,
                            modified: Some("\"no tilting\"".to_string()),
                        },
                    ],
                },
                Change::Script {
                    lines_added: 2,
                    lines_removed: 0,
                },
            ]
        );
        assert_eq!(changes[1].to_string(), "script changed (+2 -0 lines)");
    }

    #[test]
    fn game_items_are_paired_by_name() {
        let mut original = blank_table();
        original.gameitems = vec![wall("A"), wall("B"), wall("C")];
        let mut modified = blank_table();
        let mut changed = Wall {
            name: "B".to_string(),
            ..Wall::default()
        };
        changed.height_top += 10.0;
        changed.top_material = "Metal".to_string();
        // C removed, D added, B changed, order of A and B swapped
        modified.gameitems = vec![GameItemEnum::Wall(changed), wall("A"), wall("D")];

        let changes = diff(&original, &modified);
        assert_eq!(
            changes,
            vec![
                Change::Changed {
                    entity: wall_entity("B"),
                    fields: vec![
                        FieldChange {
                            field: "top_material".to_string(),
                            original: Some("\"\"".to_string()),
                            modified: Some("\"Metal\"".to_string()),
                        },
                        FieldChange {
                            field: "height_top".to_string(),
                            original: Some(Wall::default().height_top.to_string()),
                            modified: Some((Wall::default().height_top + 10.0).to_string()),
                        },
                    ],
                },
                Change::Removed(wall_entity("C")),
                Change::Added(wall_entity("D")),
                Change::Reordered(EntityKind::GameItem),
            ]
        );
        assert_eq!(changes[1].to_string(), "Wall \"C\" removed");
        assert_eq!(changes[3].to_string(), "game items reordered");
    }

    #[test]
    fn a_same_name_item_of_another_type_is_removed_and_added() {
        let mut original = blank_table();
        original.gameitems = vec![wall("X")];
        let mut modified = blank_table();
        modified.gameitems = vec![GameItemEnum::Timer(Timer {
            name: "X".to_string(),
            ..Default::default()
        })];
        let changes = diff(&original, &modified);
        assert!(
            matches!(&changes[0], Change::Removed(Entity::GameItem { type_name, .. }) if type_name == "Wall")
        );
        assert!(
            matches!(&changes[1], Change::Added(Entity::GameItem { type_name, .. }) if type_name == "Timer")
        );
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn duplicate_names_pair_up_in_order() {
        let mut original = blank_table();
        original.gameitems = vec![wall("A"), wall("A")];
        let mut modified = blank_table();
        modified.gameitems = vec![wall("A"), wall("A"), wall("A")];
        let changes = diff(&original, &modified);
        assert_eq!(changes, vec![Change::Added(wall_entity("A"))]);
    }

    #[test]
    fn a_re_encoded_image_is_reported_once() -> TestResult {
        let png = include_bytes!("../../../testdata/1x1.png").to_vec();
        let png_len = png.len();
        let decoded = ::image::load_from_memory(&png)?;
        let mut webp = Vec::new();
        decoded.write_to(
            &mut std::io::Cursor::new(&mut webp),
            ::image::ImageFormat::WebP,
        )?;
        assert_ne!(png, webp);

        let mut original = blank_table();
        original.images = vec![png_image("pixel", png), png_image("gone", Vec::new())];
        let mut modified = blank_table();
        let mut re_encoded = png_image("pixel", webp);
        re_encoded.change_extension("webp");
        modified.images = vec![re_encoded];

        let changes = diff(&original, &modified);
        assert_eq!(changes.len(), 2, "{changes:?}");
        let Change::Changed { entity, fields } = &changes[0] else {
            panic!("expected a change, got {}", changes[0]);
        };
        assert_eq!(
            entity,
            &Entity::Image {
                name: "pixel".to_string()
            }
        );
        assert_eq!(fields.len(), 2, "{fields:?}");
        assert_eq!(fields[0].field, "path");
        assert_eq!(fields[0].original.as_deref(), Some("\"C:\\pixel.png\""));
        assert_eq!(fields[1].field, "data");
        assert_eq!(
            fields[1].original.as_deref(),
            Some(format!("png 1x1, {png_len} bytes").as_str())
        );
        assert!(
            fields[1]
                .modified
                .as_deref()
                .is_some_and(|value| value.starts_with("webp 1x1, ")
                    && value.ends_with(" (re-encoded, same pixels)")),
            "{:?}",
            fields[1].modified
        );
        assert_eq!(
            changes[1],
            Change::Removed(Entity::Image {
                name: "gone".to_string()
            })
        );
        Ok(())
    }

    #[test]
    fn a_replaced_sound_reports_its_format() {
        let mut original = blank_table();
        original.sounds = vec![wav_sound("ball_roll", vec![0; 88200])];
        let mut modified = blank_table();
        let mut replaced = wav_sound("ball_roll", vec![0; 44100]);
        replaced.wave_form.channels = 2;
        replaced.volume = 50;
        modified.sounds = vec![replaced, wav_sound("new", Vec::new())];

        let changes = diff(&original, &modified);
        assert_eq!(
            changes,
            vec![
                Change::Changed {
                    entity: Entity::Sound {
                        name: "ball_roll".to_string()
                    },
                    fields: vec![
                        FieldChange {
                            field: "volume".to_string(),
                            original: Some("0".to_string()),
                            modified: Some("50".to_string()),
                        },
                        FieldChange {
                            field: "data".to_string(),
                            original: Some(
                                "wav pcm 44100 Hz 16-bit mono, 1.00 s, 86.1 KB".to_string()
                            ),
                            modified: Some(
                                "wav pcm 44100 Hz 16-bit stereo, 0.50 s, 43.1 KB".to_string()
                            ),
                        },
                    ],
                },
                Change::Added(Entity::Sound {
                    name: "new".to_string()
                }),
            ]
        );
    }

    #[test]
    fn a_collection_reports_its_items() {
        let collection = |items: &[&str], fire_events: bool| Collection {
            name: "Bumpers".to_string(),
            items: items.iter().map(|item| item.to_string()).collect(),
            fire_events,
            stop_single_events: false,
            group_elements: false,
        };
        let mut original = blank_table();
        original.collections = vec![collection(&["B1", "B2"], false)];
        let mut modified = blank_table();
        modified.collections = vec![collection(&["B3", "B1"], true)];
        let changes = diff(&original, &modified);
        assert_eq!(
            changes,
            vec![Change::Changed {
                entity: Entity::Collection {
                    name: "Bumpers".to_string()
                },
                fields: vec![
                    FieldChange {
                        field: "items".to_string(),
                        original: Some("2 items".to_string()),
                        modified: Some("2 items (added \"B3\", removed \"B2\")".to_string()),
                    },
                    FieldChange {
                        field: "fire_events".to_string(),
                        original: Some("false".to_string()),
                        modified: Some("true".to_string()),
                    },
                ],
            }]
        );
        assert_eq!(
            changes[0].to_string(),
            "collection \"Bumpers\": items 2 items -> 2 items (added \"B3\", removed \"B2\"), fire events false -> true"
        );

        let mut reordered = blank_table();
        reordered.collections = vec![collection(&["B2", "B1"], false)];
        let changes = diff(&original, &reordered);
        assert_eq!(
            changes[0].to_string(),
            "collection \"Bumpers\": items 2 items -> 2 items (reordered)"
        );
    }

    #[test]
    fn many_changed_array_elements_are_summarized() {
        let original = json!({ "points": [{"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5}] });
        let modified = json!({ "points": [{"x": 9}, {"x": 9}, {"x": 9}, {"x": 9}, {"x": 5}] });
        assert_eq!(
            json_fields(&original, &modified),
            vec![FieldChange {
                field: "points".to_string(),
                original: Some("5 items".to_string()),
                modified: Some("5 items, 4 changed".to_string()),
            }]
        );

        let few = json!({ "points": [{"x": 9}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5}] });
        assert_eq!(
            json_fields(&original, &few),
            vec![FieldChange {
                field: "points[0].x".to_string(),
                original: Some("1".to_string()),
                modified: Some("9".to_string()),
            }]
        );

        let shorter = json!({ "points": [{"x": 1}] });
        assert_eq!(
            json_fields(&original, &shorter),
            vec![FieldChange {
                field: "points".to_string(),
                original: Some("5 items".to_string()),
                modified: Some("1 item".to_string()),
            }]
        );
    }

    #[test]
    fn a_property_that_became_zero_from_absent_is_not_a_change() {
        let original = json!({ "a": null, "b": 1, "c": 1 });
        let modified = json!({ "a": 0, "b": null, "c": 2, "d": false, "e": "", "f": "x" });
        assert_eq!(
            json_fields(&original, &modified),
            vec![
                FieldChange {
                    field: "b".to_string(),
                    original: Some("1".to_string()),
                    modified: None,
                },
                FieldChange {
                    field: "c".to_string(),
                    original: Some("1".to_string()),
                    modified: Some("2".to_string()),
                },
                FieldChange {
                    field: "f".to_string(),
                    original: None,
                    modified: Some("\"x\"".to_string()),
                },
            ]
        );
    }

    #[test]
    fn nearly_equal_floats_are_equal() {
        assert_eq!(
            json_fields(
                &json!({ "t": 0.04705882_f32 }),
                &json!({ "t": 0.047058824_f32 })
            ),
            Vec::new()
        );
        assert_eq!(
            json_fields(&json!({ "t": 1.0 }), &json!({ "t": 1.001 })).len(),
            1
        );
        assert_eq!(
            json_fields(&json!({ "t": 0.0 }), &json!({ "t": 0.0 })),
            Vec::new()
        );
    }

    #[test]
    fn defaults_a_newer_file_version_adds_are_not_changes() {
        // three walls that gained properties in the newer version: the
        // elasticity falloff mostly holds one value, every drag point became
        // a slingshot, and the layer names differ but are derived
        let point = |x: f32| DragPoint {
            x,
            ..DragPoint::default()
        };
        let mut original = blank_table();
        original.version = Version::new(1060);
        original.gameitems = vec![wall("A"), wall("B"), wall("C")];
        for item in &mut original.gameitems {
            if let GameItemEnum::Wall(wall) = item {
                wall.drag_points = vec![point(1.0), point(2.0), point(3.0), point(4.0)];
            }
        }
        let mut modified = blank_table();
        modified.version = Version::new(1072);
        modified.gameitems = Vec::new();
        for (name, layer, falloff) in [
            ("A", "Layer_1", 0.5),
            ("B", "Layer_1", 0.5),
            ("C", "Layer_2", 0.7),
        ] {
            let mut wall = Wall {
                name: name.to_string(),
                ..Wall::default()
            };
            wall.editor_layer_name = Some(layer.to_string());
            wall.elasticity_falloff = Some(falloff);
            wall.drag_points = [1.0, 2.0, 3.0, 4.0]
                .map(|x| DragPoint {
                    is_slingshot: Some(true),
                    ..point(x)
                })
                .to_vec();
            modified.gameitems.push(GameItemEnum::Wall(wall));
        }

        assert_eq!(
            diff(&original, &modified),
            vec![
                Change::Changed {
                    entity: Entity::TableSettings,
                    fields: vec![FieldChange {
                        field: "file_version".to_string(),
                        original: Some("10.6".to_string()),
                        modified: Some("10.72".to_string()),
                    }],
                },
                Change::Changed {
                    entity: wall_entity("C"),
                    fields: vec![FieldChange {
                        field: "elasticity_falloff".to_string(),
                        original: None,
                        modified: Some("0.7".to_string()),
                    }],
                },
            ]
        );

        // with the same file version the gained properties are edits
        modified.version = Version::new(1060);
        let changes = diff(&original, &modified);
        assert_eq!(changes.len(), 3);
        assert_eq!(
            changes[0].to_string(),
            "Wall \"A\": elasticity falloff (none) -> 0.5, drag points 4 items -> 4 items, 4 changed, editor layer name (none) -> \"Layer_1\""
        );
    }

    #[test]
    fn a_mesh_with_reordered_triangles_is_the_same_mesh() -> TestResult {
        let xs = [0.0, 1.0, 2.0, 3.0];
        let mut original = blank_table();
        original.gameitems = vec![primitive_with_mesh(&xs, &[(0, 1, 2), (1, 2, 3)])?];
        let mut reordered = blank_table();
        reordered.gameitems = vec![primitive_with_mesh(&xs, &[(1, 2, 3), (0, 1, 2)])?];
        let mut reshaped = blank_table();
        reshaped.gameitems = vec![primitive_with_mesh(&xs, &[(0, 1, 2), (0, 2, 3)])?];

        assert_eq!(diff(&original, &reordered), Vec::new());
        assert_eq!(
            diff(&original, &reshaped),
            vec![Change::Changed {
                entity: Entity::GameItem {
                    type_name: "Primitive".to_string(),
                    name: "Prim".to_string()
                },
                fields: vec![FieldChange {
                    field: "mesh".to_string(),
                    original: Some("4 vertices, 6 indices".to_string()),
                    modified: Some("4 vertices, 6 indices, different geometry".to_string()),
                }],
            }]
        );
        Ok(())
    }

    #[test]
    fn old_and_new_material_layouts_compare_equal() {
        let material = |friction: f32| {
            let mut material = Material::default();
            material.name = "Rubber".to_string();
            material.type_ = MaterialType::Metal;
            material.thickness = 0.2;
            material.glossy_image_lerp = 0.0;
            material.opacity_active = true;
            material.elasticity = 0.8;
            material.friction = friction;
            material
        };
        let mut original = blank_table();
        original.gamedata.materials = Some(vec![material(0.6)]);
        let mut modified = blank_table();
        modified.gamedata.materials = None;
        modified.gamedata.materials_old = vec![SaveMaterial::from(&material(0.6))];
        modified.gamedata.materials_physics_old =
            Some(vec![SavePhysicsMaterial::from(&material(0.6))]);
        assert_eq!(diff(&original, &modified), Vec::new());

        modified.gamedata.materials_physics_old =
            Some(vec![SavePhysicsMaterial::from(&material(0.3))]);
        assert_eq!(
            diff(&original, &modified),
            vec![Change::Changed {
                entity: Entity::Material {
                    name: "Rubber".to_string()
                },
                fields: vec![FieldChange {
                    field: "friction".to_string(),
                    original: Some("0.6".to_string()),
                    modified: Some("0.3".to_string()),
                }],
            }]
        );
    }

    #[test]
    fn control_characters_in_names_are_escaped() {
        assert_eq!(quote("a\u{1}b\nc"), "\"a\\u{1}b\\nc\"");
        assert_eq!(quote("Décor"), "\"Décor\"");
    }

    #[test]
    fn single_precision_floats_render_short() {
        let value = to_value(0.075f32);
        assert_eq!(render(&value), Some("0.075".to_string()));
        assert_eq!(render(&json!(0.1)), Some("0.1".to_string()));
        assert_eq!(render(&json!(2.5e-9)), Some("0.0000000025".to_string()));
        assert_eq!(render(&json!(null)), None);
        assert_eq!(
            render(&json!("C:\\tables\\x.png")),
            Some("\"C:\\tables\\x.png\"".to_string())
        );
    }

    #[test]
    fn line_delta_counts_only_the_lines_on_one_side() {
        assert_eq!(line_delta("a\nb\nc", "a\nc\nd\nd"), (2, 1));
        assert_eq!(line_delta("a\r\nb", "a\nb"), (0, 0));
        assert_eq!(
            Change::Script {
                lines_added: 0,
                lines_removed: 0
            }
            .to_string(),
            "script changed (line endings only)"
        );
    }

    #[test]
    fn bytes_are_described_in_units() {
        assert_eq!(describe_bytes(512), "512 bytes");
        assert_eq!(describe_bytes(11150), "10.9 KB");
        assert_eq!(describe_bytes(13_300_000), "12.7 MB");
    }

    #[test]
    fn array_indexes_are_stripped_from_paths() {
        assert_eq!(without_indexes("drag_points[12].x"), "drag_points[].x");
        assert_eq!(without_indexes("a[1].b[22].c"), "a[].b[].c");
        assert_eq!(without_indexes("plain"), "plain");
    }
}
