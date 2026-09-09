//! Reading the `name` table of TrueType and OpenType fonts.
//!
//! An embedded font is registered under the names inside the file, so
//! those are what a textbox or decal refers to. Only the name table is
//! read; nothing else of the font is interpreted.

/// Reads name IDs 1 (family) and 4 (full name) from the `name` table of a
/// TrueType or OpenType font, or the first font of a collection
pub(crate) fn face_names(data: &[u8]) -> Vec<String> {
    let u16_at = |offset: usize| {
        data.get(offset..offset + 2)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
    };
    let u32_at = |offset: usize| {
        data.get(offset..offset + 4)
            .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    };
    let mut names = Vec::new();
    let base = if data.starts_with(b"ttcf") {
        u32_at(12).unwrap_or(0)
    } else {
        0
    };
    let Some(table_count) = u16_at(base + 4) else {
        return names;
    };
    for index in 0..usize::from(table_count) {
        let record = base + 12 + index * 16;
        if data.get(record..record + 4) != Some(b"name") {
            continue;
        }
        let Some(offset) = u32_at(record + 8) else {
            break;
        };
        let (Some(count), Some(strings)) = (u16_at(offset + 2), u16_at(offset + 4)) else {
            break;
        };
        let strings = offset + usize::from(strings);
        for index in 0..usize::from(count) {
            let entry = offset + 6 + index * 12;
            let (Some(platform), Some(name_id), Some(length), Some(string_offset)) = (
                u16_at(entry),
                u16_at(entry + 6),
                u16_at(entry + 8),
                u16_at(entry + 10),
            ) else {
                break;
            };
            if name_id != 1 && name_id != 4 {
                continue;
            }
            let start = strings + usize::from(string_offset);
            let Some(bytes) = data.get(start..start + usize::from(length)) else {
                continue;
            };
            // Windows and Unicode platforms store UTF-16, Macintosh a single
            // byte encoding of which the ASCII range is all that matters here
            let name = if platform == 0 || platform == 3 {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair))
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                bytes.iter().map(|byte| char::from(*byte)).collect()
            };
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
        break;
    }
    names
}

/// A minimal TrueType font holding only a `name` table with the given
/// family and full names, for tests
#[cfg(test)]
pub(crate) fn font_with_names(family: &str, full: &str) -> Vec<u8> {
    let mut strings = Vec::new();
    let mut records = Vec::new();
    for (name_id, name) in [(1u16, family), (4u16, full)] {
        let utf16: Vec<u8> = name.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        records.extend_from_slice(&3u16.to_be_bytes()); // platform windows
        records.extend_from_slice(&1u16.to_be_bytes()); // encoding unicode bmp
        records.extend_from_slice(&0x0409u16.to_be_bytes()); // english
        records.extend_from_slice(&name_id.to_be_bytes());
        records.extend_from_slice(&(utf16.len() as u16).to_be_bytes());
        records.extend_from_slice(&(strings.len() as u16).to_be_bytes());
        strings.extend_from_slice(&utf16);
    }
    let mut name_table = Vec::new();
    name_table.extend_from_slice(&0u16.to_be_bytes()); // format
    name_table.extend_from_slice(&2u16.to_be_bytes()); // count
    name_table.extend_from_slice(&(6 + records.len() as u16).to_be_bytes()); // string offset
    name_table.extend_from_slice(&records);
    name_table.extend_from_slice(&strings);

    let mut font = Vec::new();
    font.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfnt version
    font.extend_from_slice(&1u16.to_be_bytes()); // one table
    font.extend_from_slice(&[0; 6]); // search range, entry selector, range shift
    font.extend_from_slice(b"name");
    font.extend_from_slice(&0u32.to_be_bytes()); // checksum
    font.extend_from_slice(&28u32.to_be_bytes()); // offset: 12 header + 16 record
    font.extend_from_slice(&(name_table.len() as u32).to_be_bytes());
    font.extend_from_slice(&name_table);
    font
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_names_come_from_the_name_table() {
        let data = font_with_names("Advanced LED Board-7", "Advanced LED Board-7 Regular");
        assert_eq!(
            face_names(&data),
            vec![
                "Advanced LED Board-7".to_string(),
                "Advanced LED Board-7 Regular".to_string()
            ]
        );
        assert_eq!(face_names(&[1, 2, 3]), Vec::<String>::new());
        assert_eq!(face_names(&[]), Vec::<String>::new());
    }
}
