//! Comparing two VPX files.
//!
//! [`diff`] reports where two tables differ, at the file representation
//! level: streams that were added or removed, and for changed streams the
//! exact record that differs. Where the file format allows it, differences
//! are labeled in user terms: the game item type and name, the image name,
//! or the table script.
//!
//! Two files with an empty diff hold the same table. The comparison knows
//! which byte differences are harmless and ignores them:
//!
//! - the `MAC` integrity signature, which changes on every save
//! - compressed data (primitive meshes, bitmap images) is compared after
//!   decompression, since equally valid encoders produce different bytes
//! - padding bytes vpinball leaves uninitialized (`MATE`, `PHMA`)
//!
//! This module compares the file representation, not the parsed model: a
//! difference in bytes the library does not even interpret is still
//! reported. That makes it the tool for validating that a write changed
//! nothing it should not have.

use super::biff::BiffReader;
use super::gameitem::GameItemEnum;
use super::lzw::from_lzw_blocks;
use cfb::CompoundFile;
use flate2::read::ZlibDecoder;
use std::fmt;
use std::io::{self, Cursor, Read};

/// A single difference between two VPX files.
///
/// The `path` is the stream inside the compound file (for example
/// `/GameStg/GameItem12`), and `label`, when present, describes the stream
/// in user terms (for example `Bumper "LeftSling"`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Difference {
    /// A stream that only exists in the modified file
    StreamAdded { path: String },
    /// A stream that only exists in the original file
    StreamRemoved { path: String },
    /// A stream that is compared byte for byte (sounds, table info,
    /// version) has different content
    StreamChanged {
        path: String,
        label: Option<String>,
        len_original: usize,
        len_modified: usize,
    },
    /// The compound file metadata (CLSID) of a stream differs
    StreamClsidChanged {
        path: String,
        original: String,
        modified: String,
    },
    /// A game item stream holds a different item type
    GameItemTypeChanged {
        path: String,
        original: String,
        modified: String,
    },
    /// A record with the same tag on both sides has different content
    RecordChanged {
        path: String,
        label: Option<String>,
        index: usize,
        tag: String,
        len_original: usize,
        len_modified: usize,
    },
    /// A record that appears at a different position in the modified file
    RecordMoved {
        path: String,
        label: Option<String>,
        tag: String,
        index_original: usize,
        index_modified: usize,
    },
    /// The modified file has more records in this stream
    RecordAdded {
        path: String,
        label: Option<String>,
        index: usize,
        tag: String,
    },
    /// The original file has more records in this stream
    RecordRemoved {
        path: String,
        label: Option<String>,
        index: usize,
        tag: String,
    },
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn location(path: &str, label: &Option<String>) -> String {
            match label {
                Some(label) => format!("{path} ({label})"),
                None => path.to_string(),
            }
        }
        match self {
            Difference::StreamAdded { path } => write!(f, "{path}: added"),
            Difference::StreamRemoved { path } => write!(f, "{path}: removed"),
            Difference::StreamChanged {
                path,
                label,
                len_original,
                len_modified,
            } => write!(
                f,
                "{}: content differs ({len_original} -> {len_modified} bytes)",
                location(path, label)
            ),
            Difference::StreamClsidChanged {
                path,
                original,
                modified,
            } => write!(f, "{path}: CLSID differs ({original} -> {modified})"),
            Difference::GameItemTypeChanged {
                path,
                original,
                modified,
            } => write!(f, "{path}: item type changed ({original} -> {modified})"),
            Difference::RecordChanged {
                path,
                label,
                index,
                tag,
                len_original,
                len_modified,
            } => write!(
                f,
                "{}: record {index} {tag} differs ({len_original} -> {len_modified} bytes)",
                location(path, label)
            ),
            Difference::RecordMoved {
                path,
                label,
                tag,
                index_original,
                index_modified,
            } => write!(
                f,
                "{}: record {tag} moved ({index_original} -> {index_modified})",
                location(path, label)
            ),
            Difference::RecordAdded {
                path,
                label,
                index,
                tag,
            } => write!(f, "{}: record {index} {tag} added", location(path, label)),
            Difference::RecordRemoved {
                path,
                label,
                index,
                tag,
            } => write!(f, "{}: record {index} {tag} removed", location(path, label)),
        }
    }
}

/// Compares two VPX files and returns the differences, empty when the
/// files hold the same table.
///
/// See the [module documentation](self) for what is compared and which
/// byte differences are deliberately ignored.
///
/// # Errors
///
/// Fails when either input is not a valid compound file or a stream that
/// should hold BIFF records cannot be walked.
pub fn diff(original: &[u8], modified: &[u8]) -> io::Result<Vec<Difference>> {
    let mut comp_original = CompoundFile::open_strict(Cursor::new(original))?;
    let mut comp_modified = CompoundFile::open_strict(Cursor::new(modified))?;

    if comp_original.version() != comp_modified.version() {
        // vpinball does not upgrade the container version of old tables while
        // this library always writes the latest, so this is not a difference
        log::warn!(
            "Compound file format versions differ: {:?} -> {:?}",
            comp_original.version(),
            comp_modified.version()
        );
    }

    let mut differences = Vec::new();

    let streams_original = stream_entries(&comp_original);
    let streams_modified = stream_entries(&comp_modified);

    for (path, _) in &streams_modified {
        if !streams_original.iter().any(|(p, _)| p == path) {
            differences.push(Difference::StreamAdded { path: path.clone() });
        }
    }

    for (path, clsid_original) in &streams_original {
        let Some((_, clsid_modified)) = streams_modified.iter().find(|(p, _)| p == path) else {
            differences.push(Difference::StreamRemoved { path: path.clone() });
            continue;
        };
        if clsid_original != clsid_modified {
            differences.push(Difference::StreamClsidChanged {
                path: path.clone(),
                original: clsid_original.clone(),
                modified: clsid_modified.clone(),
            });
        }

        if path == "/GameStg/MAC" {
            // the integrity signature changes on every save
            continue;
        }

        let data_original = read_stream(&mut comp_original, path)?;
        let data_modified = read_stream(&mut comp_modified, path)?;

        if path == "/GameStg/Version" || path.starts_with("/TableInfo/") || path.contains("Sound") {
            // not (purely) BIFF, compared byte for byte
            if data_original != data_modified {
                differences.push(Difference::StreamChanged {
                    path: path.clone(),
                    label: None,
                    len_original: data_original.len(),
                    len_modified: data_modified.len(),
                });
            }
            continue;
        }

        diff_biff_stream(path, &data_original, &data_modified, &mut differences)?;
    }

    Ok(differences)
}

fn stream_entries(comp: &CompoundFile<Cursor<&[u8]>>) -> Vec<(String, String)> {
    comp.walk()
        .filter(|entry| entry.is_stream())
        .map(|entry| (normalized_path(entry.path()), entry.clsid().to_string()))
        .collect()
}

/// The stream path with `/` separators on every platform; the standard
/// path type renders compound file paths with the platform separator
fn normalized_path(path: &std::path::Path) -> String {
    let mut normalized = String::new();
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            normalized.push('/');
            normalized.push_str(&name.to_string_lossy());
        }
    }
    normalized
}

fn read_stream(comp: &mut CompoundFile<Cursor<&[u8]>>, path: &str) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    comp.open_stream(path)?.read_to_end(&mut data)?;
    Ok(data)
}

fn diff_biff_stream(
    path: &str,
    data_original: &[u8],
    data_modified: &[u8],
    differences: &mut Vec<Difference>,
) -> io::Result<()> {
    let mut skip = 0;
    if path.contains("GameItem") {
        // game item streams start with the item type
        let type_original = item_type(data_original);
        let type_modified = item_type(data_modified);
        if type_original != type_modified {
            differences.push(Difference::GameItemTypeChanged {
                path: path.to_string(),
                original: GameItemEnum::type_name_for_id(type_original),
                modified: GameItemEnum::type_name_for_id(type_modified),
            });
            return Ok(());
        }
        skip = 4;
    }
    let label = stream_label(path, data_original);

    let records_original = biff_records(&data_original[skip..])?;
    let records_modified = biff_records(&data_modified[skip..])?;

    // Align the two record sequences on their tags so that inserted,
    // removed and reordered records are reported as such instead of
    // knocking every following comparison out of step
    let mut removed: Vec<usize> = Vec::new();
    let mut added: Vec<usize> = Vec::new();
    for pair in align(&records_original, &records_modified) {
        match pair {
            (Some(index_original), Some(index_modified)) => {
                let record_original = &records_original[index_original];
                let record_modified = &records_modified[index_modified];
                if record_original != record_modified {
                    differences.push(Difference::RecordChanged {
                        path: path.to_string(),
                        label: label.clone(),
                        index: index_original,
                        tag: record_original.tag.clone(),
                        len_original: record_original.len,
                        len_modified: record_modified.len,
                    });
                }
            }
            (Some(index_original), None) => removed.push(index_original),
            (None, Some(index_modified)) => added.push(index_modified),
            (None, None) => unreachable!(),
        }
    }

    // a record removed in one place and added in another with the same tag
    // is a move
    for index_original in removed {
        let tag = &records_original[index_original].tag;
        if let Some(position) = added
            .iter()
            .position(|&index| &records_modified[index].tag == tag)
        {
            let index_modified = added.remove(position);
            differences.push(Difference::RecordMoved {
                path: path.to_string(),
                label: label.clone(),
                tag: tag.clone(),
                index_original,
                index_modified,
            });
            if records_original[index_original] != records_modified[index_modified] {
                differences.push(Difference::RecordChanged {
                    path: path.to_string(),
                    label: label.clone(),
                    index: index_original,
                    tag: tag.clone(),
                    len_original: records_original[index_original].len,
                    len_modified: records_modified[index_modified].len,
                });
            }
        } else {
            differences.push(Difference::RecordRemoved {
                path: path.to_string(),
                label: label.clone(),
                index: index_original,
                tag: tag.clone(),
            });
        }
    }
    for index_modified in added {
        differences.push(Difference::RecordAdded {
            path: path.to_string(),
            label: label.clone(),
            index: index_modified,
            tag: records_modified[index_modified].tag.clone(),
        });
    }
    Ok(())
}

/// Pairs up the two record sequences on their tags with a longest common
/// subsequence, so a single inserted or moved record does not shift every
/// comparison after it
fn align(a: &[Record], b: &[Record]) -> Vec<(Option<usize>, Option<usize>)> {
    let n = a.len();
    let m = b.len();
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i].tag == b[j].tag {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i].tag == b[j].tag {
            pairs.push((Some(i), Some(j)));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            pairs.push((Some(i), None));
            i += 1;
        } else {
            pairs.push((None, Some(j)));
            j += 1;
        }
    }
    pairs.extend((i..n).map(|i| (Some(i), None)));
    pairs.extend((j..m).map(|j| (None, Some(j))));
    pairs
}

fn item_type(data: &[u8]) -> u32 {
    let mut bytes = [0u8; 4];
    if data.len() >= 4 {
        bytes.copy_from_slice(&data[..4]);
    }
    u32::from_le_bytes(bytes)
}

/// Best effort user facing description of a stream, `None` when the stream
/// has no name to offer
fn stream_label(path: &str, data: &[u8]) -> Option<String> {
    if path.contains("GameItem") {
        let type_name = GameItemEnum::type_name_for_id(item_type(data));
        let name = find_record(&data[4..], "NAME", |reader| reader.get_wide_string())?;
        Some(format!("{type_name} {name:?}"))
    } else if path.contains("Image") {
        let name = find_record(data, "NAME", |reader| reader.get_string())?;
        Some(format!("image {name:?}"))
    } else if path.contains("Collection") {
        let name = find_record(data, "NAME", |reader| reader.get_wide_string())?;
        Some(format!("collection {name:?}"))
    } else {
        None
    }
}

fn find_record<T>(
    data: &[u8],
    wanted: &str,
    read: impl Fn(&mut BiffReader) -> Result<T, super::biff::BiffError>,
) -> Option<T> {
    let mut reader = BiffReader::new(data);
    reader.disable_warn_remaining();
    while let Ok(Some(tag)) = reader.next(false) {
        if tag == wanted {
            return read(&mut reader).ok();
        }
        reader.skip_tag().ok()?;
    }
    None
}

/// One BIFF record, normalized so that only meaningful differences remain
#[derive(Debug, PartialEq, Eq)]
struct Record {
    tag: String,
    /// The record size announced in the stream
    size_field: usize,
    /// Length of the compared content
    len: usize,
    content: Content,
}

#[derive(Debug, PartialEq, Eq)]
enum Content {
    Bytes(Vec<u8>),
    /// Content that cannot be compared, only the lengths are
    Ignored,
}

/// Walks a BIFF stream into comparable records, mirroring the special
/// cases of the file format:
///
/// - `FONT`: an untagged FONTDESC blob up to `ENDB`
/// - `JPEG`: a nested BIFF stream, walked into its own records
/// - `BITS` and the `M3CX`/`M3CI`/`M3AX` mesh records: compared after
///   decompression since different encoders produce different bytes
/// - `M3CY`/`M3CJ`/`M3AY`: compressed sizes, which depend on the encoder
/// - `MATE`/`PHMA`: vpinball writes uninitialized padding bytes here
/// - `CODE`: compared as the decoded script text
fn biff_records(data: &[u8]) -> io::Result<Vec<Record>> {
    let mut reader = BiffReader::new(data);
    reader.disable_warn_remaining();
    let mut records = Vec::new();
    while let Some(tag) = reader.next(false)? {
        let size_field = reader.remaining_in_record();
        match tag.as_str() {
            "FONT" => {
                let data = reader.data_until("ENDB".as_bytes())?;
                records.push(Record {
                    tag,
                    size_field,
                    len: data.len(),
                    content: Content::Bytes(data),
                });
            }
            "JPEG" => {
                records.push(Record {
                    tag,
                    size_field,
                    len: reader.remaining_in_record(),
                    content: Content::Ignored,
                });
                let mut sub_reader = reader.child_reader();
                while let Some(sub_tag) = sub_reader.next(false)? {
                    let data = sub_reader.get_record_data(false)?;
                    records.push(Record {
                        tag: sub_tag,
                        size_field,
                        len: data.len(),
                        content: Content::Bytes(data),
                    });
                }
                let pos = sub_reader.pos();
                reader.skip_end_tag(pos)?;
            }
            "BITS" => {
                let data = reader.data_until("ALTV".as_bytes())?;
                let decompressed = from_lzw_blocks(&data)?;
                records.push(Record {
                    tag: "BITS (decompressed)".to_string(),
                    size_field,
                    len: decompressed.len(),
                    content: Content::Bytes(decompressed),
                });
            }
            "CODE" => {
                let len = reader.get_u32_no_remaining_update()?;
                let script = reader.get_str_with_encoding_no_remaining_update(len as usize)?;
                records.push(Record {
                    tag: "CODE (script)".to_string(),
                    size_field,
                    len: len as usize,
                    content: Content::Bytes(script.string.into_bytes()),
                });
            }
            "MATE" | "PHMA" => {
                let data = reader.get_record_data(false)?;
                records.push(Record {
                    tag: format!("{tag} (padding ignored)"),
                    size_field,
                    len: data.len(),
                    content: Content::Ignored,
                });
            }
            "M3CY" | "M3CJ" | "M3AY" => {
                let data = reader.get_record_data(false)?;
                records.push(Record {
                    tag: format!("{tag} (compressed size ignored)"),
                    size_field,
                    len: data.len(),
                    content: Content::Ignored,
                });
            }
            "M3CX" | "M3CI" | "M3AX" => {
                let compressed = reader.get_record_data(false)?;
                let mut decoder: ZlibDecoder<&[u8]> = ZlibDecoder::new(compressed.as_ref());
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                records.push(Record {
                    tag: format!("{tag} (decompressed)"),
                    // the compressed size depends on the encoder
                    size_field: 0,
                    len: decompressed.len(),
                    content: Content::Bytes(decompressed),
                });
            }
            _ => {
                let data = reader.get_record_data(false)?;
                records.push(Record {
                    tag,
                    size_field,
                    len: data.len(),
                    content: Content::Bytes(data),
                });
            }
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn blank_table() -> Vec<u8> {
        // embedded because the wasm test runner has no file system access
        include_bytes!("../../testdata/completely_blank_table_10_7_4.vpx").to_vec()
    }

    #[test]
    fn identical_files_have_no_differences() -> TestResult {
        let bytes = blank_table();
        assert_eq!(diff(&bytes, &bytes)?, Vec::new());
        Ok(())
    }

    #[test]
    fn a_written_blank_table_matches_itself() -> TestResult {
        let vpx = vpx::from_bytes(&blank_table())?;
        let written = vpx::to_bytes(&vpx)?;
        assert_eq!(diff(&written, &written)?, Vec::new());
        Ok(())
    }

    #[test]
    fn a_changed_table_name_is_reported_as_a_record_change() -> TestResult {
        let bytes = blank_table();
        let mut vpx = vpx::from_bytes(&bytes)?;
        vpx.gamedata.name = "renamed".to_string();
        let modified = vpx::to_bytes(&vpx)?;
        let original = vpx::to_bytes(&vpx::from_bytes(&bytes)?)?;

        let differences = diff(&original, &modified)?;
        assert_eq!(differences.len(), 1);
        assert!(
            matches!(
                &differences[0],
                Difference::RecordChanged { path, tag, .. }
                    if path == "/GameStg/GameData" && tag == "NAME"
            ),
            "unexpected difference: {}",
            differences[0]
        );
        Ok(())
    }

    #[test]
    fn a_changed_script_is_reported_as_the_script() -> TestResult {
        let bytes = blank_table();
        let mut vpx = vpx::from_bytes(&bytes)?;
        vpx.gamedata.code.string += "\n' a mod";
        let modified = vpx::to_bytes(&vpx)?;
        let original = vpx::to_bytes(&vpx::from_bytes(&bytes)?)?;

        let differences = diff(&original, &modified)?;
        assert_eq!(differences.len(), 1);
        assert!(
            matches!(
                &differences[0],
                Difference::RecordChanged { tag, .. } if tag == "CODE (script)"
            ),
            "unexpected difference: {}",
            differences[0]
        );
        Ok(())
    }

    #[test]
    fn a_reordered_record_is_reported_as_moved() -> TestResult {
        use crate::vpx::biff::BiffWriter;
        let mut writer = BiffWriter::new();
        writer.write_tagged_u32("AAAA", 1);
        writer.write_tagged_u32("BBBB", 2);
        writer.write_tagged_u32("CCCC", 3);
        writer.close(true);
        let original = writer.get_data().to_vec();

        let mut writer = BiffWriter::new();
        writer.write_tagged_u32("AAAA", 1);
        writer.write_tagged_u32("CCCC", 3);
        writer.write_tagged_u32("BBBB", 2);
        writer.close(true);
        let modified = writer.get_data().to_vec();

        let mut differences = Vec::new();
        diff_biff_stream("/GameStg/GameData", &original, &modified, &mut differences)?;
        assert_eq!(
            differences,
            vec![Difference::RecordMoved {
                path: "/GameStg/GameData".to_string(),
                label: None,
                tag: "BBBB".to_string(),
                index_original: 1,
                index_modified: 2,
            }]
        );
        Ok(())
    }

    #[test]
    fn a_removed_stream_is_reported() -> TestResult {
        let original = blank_table();
        let mut comp = cfb::CompoundFile::open(Cursor::new(original.clone()))?;
        comp.remove_stream("/TableInfo/TableName")?;
        comp.flush()?;
        let modified = comp.into_inner().into_inner();

        let differences = diff(&original, &modified)?;
        assert_eq!(
            differences,
            vec![Difference::StreamRemoved {
                path: "/TableInfo/TableName".to_string()
            }]
        );
        Ok(())
    }
}
