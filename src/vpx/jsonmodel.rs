//! Deprecated: the JSON conversions moved next to the types they convert.
//! Use [`crate::vpx::tableinfo::info_to_json`],
//! [`crate::vpx::tableinfo::json_to_info`],
//! [`crate::vpx::collection::collections_json`],
//! [`crate::vpx::collection::json_to_collections`] and
//! [`crate::vpx::gamedata::game_data_to_json`]. This module will be removed
//! in the next breaking release.

use crate::vpx::collection::Collection;
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::gamedata::GameData;
use crate::vpx::tableinfo::TableInfo;

/// Moved to [`crate::vpx::tableinfo::info_to_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::tableinfo::info_to_json")]
pub fn info_to_json(
    table_info: &TableInfo,
    custom_info_tags: &CustomInfoTags,
) -> serde_json::Value {
    crate::vpx::tableinfo::info_to_json(table_info, custom_info_tags)
}

/// Moved to [`crate::vpx::tableinfo::json_to_info`].
///
/// # Errors
///
/// See [`crate::vpx::tableinfo::json_to_info`].
#[deprecated(since = "0.34.0", note = "moved to vpx::tableinfo::json_to_info")]
pub fn json_to_info(
    json: serde_json::Value,
    screenshot: Option<Vec<u8>>,
) -> Result<(TableInfo, CustomInfoTags), serde_json::Error> {
    crate::vpx::tableinfo::json_to_info(json, screenshot)
}

/// Moved to [`crate::vpx::collection::collections_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::collection::collections_json")]
pub fn collections_json(collections: &[Collection]) -> serde_json::Value {
    crate::vpx::collection::collections_json(collections)
}

/// Moved to [`crate::vpx::collection::json_to_collections`].
///
/// # Errors
///
/// See [`crate::vpx::collection::json_to_collections`].
#[deprecated(
    since = "0.34.0",
    note = "moved to vpx::collection::json_to_collections"
)]
pub fn json_to_collections(json: serde_json::Value) -> Result<Vec<Collection>, serde_json::Error> {
    crate::vpx::collection::json_to_collections(json)
}

/// Moved to [`crate::vpx::gamedata::game_data_to_json`].
#[deprecated(since = "0.34.0", note = "moved to vpx::gamedata::game_data_to_json")]
pub fn game_data_to_json(game_data: &GameData) -> serde_json::Value {
    crate::vpx::gamedata::game_data_to_json(game_data)
}
