use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::collections::HashMap;

use crate::vpx::collection::Collection;
use crate::vpx::custominfotags::CustomInfoTags;
use crate::vpx::gamedata::{GameData, GameDataJson};
use crate::vpx::tableinfo::TableInfo;

#[derive(Serialize, Deserialize)]
struct CollectionJson {
    name: String,
    items: Vec<String>,
    fire_events: bool,
    stop_single_events: bool,
    group_elements: bool,
}

/// Since we want to decouple out json model from the vpx model, we need to
/// define a json model that we can serialize to and from. This is a bit of a
/// pain, but it's the only way to do it.
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
    // since the ordering is important, we need to keep track of it
    properties_order: Vec<String>,
}

pub fn info_to_json(
    table_info: &TableInfo,
    custom_info_tags: &CustomInfoTags,
) -> serde_json::Value {
    // TODO convert to a serde
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
    to_value(info_json).unwrap()
}

pub fn json_to_info(
    json: serde_json::Value,
    screenshot: Option<Vec<u8>>,
) -> Result<(TableInfo, CustomInfoTags), serde_json::Error> {
    let info_json: TableInfoJson = serde_json::from_value(json.clone())?;
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

pub fn collections_json(collections: &[Collection]) -> serde_json::Value {
    let mut collections_json = Vec::new();
    for collection in collections {
        let collection_json = CollectionJson {
            name: collection.name.clone(),
            items: collection.items.clone(),
            fire_events: collection.fire_events,
            stop_single_events: collection.stop_single_events,
            group_elements: collection.group_elements,
        };
        collections_json.push(collection_json);
    }
    to_value(collections_json).unwrap()
}

pub fn json_to_collections(json: serde_json::Value) -> Result<Vec<Collection>, serde_json::Error> {
    let collections_json: Vec<CollectionJson> = serde_json::from_value(json)?;
    let mut collections = Vec::new();
    for collection_json in collections_json {
        let collection = Collection {
            name: collection_json.name,
            items: collection_json.items,
            fire_events: collection_json.fire_events,
            stop_single_events: collection_json.stop_single_events,
            group_elements: collection_json.group_elements,
        };
        collections.push(collection);
    }
    Ok(collections)
}

pub fn game_data_to_json(game_data: &GameData) -> serde_json::Value {
    let game_data_json = GameDataJson::from_game_data(game_data);
    to_value(game_data_json).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::collection::Collection;
    use crate::vpx::custominfotags::CustomInfoTags;
    use crate::vpx::gamedata::GameData;
    use crate::vpx::tableinfo::TableInfo;
    use serde_json::Value;

    #[test]
    fn test_info_to_json() {
        let table_info = TableInfo::default();
        let custom_info_tags = CustomInfoTags::default();
        let json = info_to_json(&table_info, &custom_info_tags);
        let (table_info2, custom_info_tags2) = json_to_info(json, None).unwrap();
        assert_eq!(table_info, table_info2);
        assert_eq!(custom_info_tags, custom_info_tags2);
    }

    #[test]
    fn test_collections_to_json() {
        let collections = vec![
            Collection {
                name: "collection1".to_string(),
                items: vec!["item1".to_string(), "item2".to_string()],
                fire_events: true,
                stop_single_events: false,
                group_elements: true,
            },
            Collection {
                name: "collection2".to_string(),
                items: vec!["item3".to_string(), "item4".to_string()],
                fire_events: false,
                stop_single_events: true,
                group_elements: false,
            },
        ];
        let json = collections_json(&collections);
        let collections2 = json_to_collections(json).unwrap();
        assert_eq!(collections, collections2);
    }

    #[test]
    fn test_game_data_to_json() {
        let game_data = GameData::default();
        let json = game_data_to_json(&game_data);
        // assert that we have a value of type object that at least contains "name": String("Table1")
        let map = json.as_object().unwrap();
        let name = map.get("name").unwrap();
        assert_eq!(name, &Value::String("Table1".to_string()));
    }
}
