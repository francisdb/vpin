//! Deprecated: the JSON conversions moved next to the types they convert.
//! These wrappers keep the old entry points until the next breaking
//! release.

use crate::vpx::collection::Collection;
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::gamedata::GameData;
use crate::vpx::tableinfo::TableInfo;

/// Deprecated wrapper of [`crate::vpx::tableinfo::info_to_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::tableinfo::info_to_json")]
pub fn info_to_json(
    table_info: &TableInfo,
    custom_info_tags: &CustomInfoTags,
) -> serde_json::Value {
    crate::vpx::tableinfo::info_to_json(table_info, custom_info_tags)
}

/// Deprecated wrapper of [`crate::vpx::tableinfo::json_to_info`].
///
/// # Errors
///
/// Fails when the JSON does not have the shape the crate writes.
#[deprecated(since = "0.34.0", note = "moved to vpx::tableinfo::json_to_info")]
pub fn json_to_info(
    json: serde_json::Value,
    screenshot: Option<Vec<u8>>,
) -> Result<(TableInfo, CustomInfoTags), serde_json::Error> {
    crate::vpx::tableinfo::json_to_info(json, screenshot)
}

/// Deprecated wrapper of [`crate::vpx::collection::collections_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::collection::collections_json")]
pub fn collections_json(collections: &[Collection]) -> serde_json::Value {
    crate::vpx::collection::collections_json(collections)
}

/// Deprecated wrapper of [`crate::vpx::collection::json_to_collections`].
///
/// # Errors
///
/// Fails when the JSON does not have the shape the crate writes.
#[deprecated(
    since = "0.34.0",
    note = "moved to vpx::collection::json_to_collections"
)]
pub fn json_to_collections(json: serde_json::Value) -> Result<Vec<Collection>, serde_json::Error> {
    crate::vpx::collection::json_to_collections(json)
}

/// Deprecated wrapper of [`crate::vpx::gamedata::game_data_to_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::gamedata::game_data_to_json")]
pub fn game_data_to_json(game_data: &GameData) -> serde_json::Value {
    crate::vpx::gamedata::game_data_to_json(game_data)
}
