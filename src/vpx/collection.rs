use super::biff::{self, BiffReader, BiffWriter};
use log::warn;
use std::io;
// TODO comment here a vpx file that contains font data

/// A named group of parts, mirroring vpinball's `Collection`.
///
/// A script addresses a collection by name to act on all of its parts at
/// once, and a collection can receive the events of its parts. vpinball
/// stores every collection as its own `Collection<n>` stream in the table
/// storage (`Collection::Save` and `Collection::Load` in
/// `src/parts/Collection.cpp`); the parts are stored by name and resolved
/// after the whole table is loaded.
#[derive(PartialEq, Debug)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Collection {
    /// Name of the collection, the identifier a script uses for it
    /// (`Collection::m_wzName`). Stored as a wide string.
    ///
    /// BIFF tag `NAME`
    pub name: String,
    /// Names of the parts in the collection, in collection order. vpinball
    /// writes one record per part and looks the parts up by name once the
    /// table is loaded. Stored as wide strings.
    ///
    /// BIFF tag `ITEM`, one per part
    pub items: Vec<String>,
    /// Whether the events of the parts are also raised on the collection
    /// (`Collection::m_fireEvents`), so a script can handle them in one
    /// `<collection>_<event>` handler that receives the part index. The
    /// "fire events for this collection" checkbox of the collection
    /// manager. Default: `false`.
    ///
    /// BIFF tag `EVNT`
    pub fire_events: bool,
    /// Whether the parts stop raising their own events
    /// (`Collection::m_stopSingleEvents`), so that only the collection
    /// event fires; vpinball clears the part's `m_singleEvents` when any
    /// of its collections has this set. The "suppress single events"
    /// checkbox of the collection manager. Default: `false`.
    ///
    /// BIFF tag `SSNG`
    pub stop_single_events: bool,
    /// Whether the parts are treated as one group
    /// (`Collection::m_groupElements`): the editor selects them together,
    /// and the renderer draws grouped primitives as a single mesh when
    /// they share material and texture. Default: the editor setting
    /// "group elements in collection", which vpinball ships as `true`.
    ///
    /// BIFF tag `GREL`
    pub group_elements: bool,
}

/// Read a collection from the bytes of a `CollectionN` stream.
///
/// Fails with [`io::ErrorKind::InvalidData`] when the stream is truncated or
/// structurally invalid instead of panicking.
pub fn read(input: &[u8]) -> io::Result<Collection> {
    let mut reader = BiffReader::new(input);
    let mut name: String = "".to_string();
    let mut items: Vec<String> = vec![];
    let mut fire_events: bool = false;
    let mut stop_single_events: bool = false;
    let mut group_elements: bool = false;
    while let Some(tag) = reader.next(biff::WARN)? {
        let tag_str = tag.as_str();
        match tag_str {
            "NAME" => {
                name = reader.get_wide_string()?;
            }
            "ITEM" => {
                let item = reader.get_wide_string()?;
                items.push(item);
            }
            "EVNT" => {
                fire_events = reader.get_bool()?;
            }
            "SSNG" => {
                stop_single_events = reader.get_bool()?;
            }
            "GREL" => {
                group_elements = reader.get_bool()?;
            }
            other => {
                warn!("Unknown tag: {other}");
                reader.skip_tag()?;
            }
        }
    }
    Ok(Collection {
        name,
        items,
        fire_events,
        stop_single_events,
        group_elements,
    })
}

/// Writes a collection as the bytes of a `Collection<n>` stream.
pub fn write(collection: &Collection) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    writer.write_tagged_wide_string("NAME", &collection.name);
    for item in &collection.items {
        writer.write_tagged_wide_string("ITEM", item);
    }
    writer.write_tagged_bool("EVNT", collection.fire_events);
    writer.write_tagged_bool("SSNG", collection.stop_single_events);
    writer.write_tagged_bool("GREL", collection.group_elements);
    writer.close(true);
    writer.get_data().to_owned()
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn write_read() {
        let collection = Collection {
            name: "Test Collection".to_string(),
            items: vec!["Item 1".to_string(), "Item 2".to_string()],
            fire_events: true,
            stop_single_events: true,
            group_elements: true,
        };
        let data = write(&collection);
        let collection2 = read(&data).unwrap();
        assert_eq!(collection, collection2);
    }
}

#[cfg(test)]
mod corrupt_input_tests {
    use super::*;

    #[test]
    fn truncated_collection_fails_without_panicking() {
        let collection = Collection {
            name: "collection".to_string(),
            items: vec!["a".to_string(), "b".to_string()],
            fire_events: true,
            stop_single_events: false,
            group_elements: true,
        };
        let bytes = write(&collection);
        assert!(read(&bytes).is_ok());
        for len in 0..bytes.len() {
            assert!(read(&bytes[..len]).is_err(), "truncated to {len}");
        }
    }
}
