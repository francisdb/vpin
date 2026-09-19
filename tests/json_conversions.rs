//! The JSON conversions are part of the public API: callers outside the
//! crate use them to print a table's game data or to keep its info in a
//! JSON file.

use std::collections::HashMap;
use vpin::vpx::collection::{Collection, collections_json, json_to_collections};
use vpin::vpx::gamedata::{GameData, game_data_to_json};
use vpin::vpx::tableinfo::{TableInfo, info_to_json, json_to_info};

#[test]
fn table_info_round_trips_through_json() {
    let table_info = TableInfo {
        table_name: Some("Test Table".to_string()),
        properties: HashMap::from([("Tag".to_string(), "Value".to_string())]),
        ..TableInfo::default()
    };
    let custom_info_tags = vec!["Tag".to_string()];
    let json = info_to_json(&table_info, &custom_info_tags);
    let (read_info, read_tags) = json_to_info(json, None).unwrap();
    assert_eq!(read_info, table_info);
    assert_eq!(read_tags, custom_info_tags);
}

#[test]
fn collections_round_trip_through_json() {
    let collections = vec![Collection {
        name: "Flashers".to_string(),
        items: vec!["F1".to_string(), "F2".to_string()],
        fire_events: true,
        stop_single_events: false,
        group_elements: true,
    }];
    let json = collections_json(&collections);
    assert_eq!(json_to_collections(json).unwrap(), collections);
}

#[test]
fn game_data_converts_to_json() {
    let json = game_data_to_json(&GameData::default());
    assert!(json.is_object());
}
