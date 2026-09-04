use super::biff::{self, BiffReader, BiffWriter};
use log::warn;
use std::io;

pub type CustomInfoTags = Vec<String>;

/// Read the `CustomInfoTags` stream.
///
/// Fails with [`io::ErrorKind::InvalidData`] when the stream is truncated or
/// structurally invalid instead of panicking.
pub fn read_custominfotags(tags_data: &[u8]) -> io::Result<CustomInfoTags> {
    let mut reader = BiffReader::new(tags_data);
    let mut tags = CustomInfoTags::new();

    loop {
        reader.next(biff::WARN);
        if reader.is_eof() {
            break;
        }
        let tag = reader.tag();
        let tag_str = tag.as_str();

        let reader: &mut BiffReader<'_> = &mut reader;

        match tag_str {
            "CUST" => {
                let tag = reader.get_string();
                tags.push(tag);
            }
            other => {
                let data = reader.get_record_data(false);
                warn!("unhandled tag {} {} bytes", other, data.len());
            }
        }
    }
    reader.check()?;
    Ok(tags)
}

pub fn write_custominfotags(tags: &CustomInfoTags) -> Vec<u8> {
    let mut writer = BiffWriter::new();
    for tag in tags {
        writer.write_tagged_string("CUST", tag);
    }
    writer.close(true);
    writer.get_data().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_empty() {
        let game_data = CustomInfoTags::default();
        let bytes = write_custominfotags(&game_data);
        let read_game_data = read_custominfotags(&bytes).unwrap();

        assert_eq!(game_data, read_game_data);
    }
}

#[cfg(test)]
mod corrupt_input_tests {
    use super::*;

    #[test]
    fn truncated_tags_fail_without_panicking() {
        let bytes = write_custominfotags(&vec!["one".to_string(), "two".to_string()]);
        assert!(read_custominfotags(&bytes).is_ok());
        for len in 0..bytes.len() {
            assert!(
                read_custominfotags(&bytes[..len]).is_err(),
                "truncated to {len}"
            );
        }
    }
}
