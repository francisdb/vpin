// Since this code is only used for tests, and not all tests use all function we allow dead code.
#![allow(dead_code)]
#![cfg(test)]

use cfb::CompoundFile;
use flate2::read::ZlibDecoder;
use std::ffi::OsStr;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use vpin::vpx::biff::BiffReader;
use vpin::vpx::lzw::from_lzw_blocks;
use walkdir::WalkDir;

pub(crate) fn find_files<P: AsRef<Path>>(
    tables_path: P,
    extension: &str,
) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut entries = WalkDir::new(tables_path).into_iter();
    let os_extension = OsStr::new(extension);
    entries.try_for_each(|entry| {
        let dir_entry = entry?;
        let path = dir_entry.path();
        if path.is_file() {
            match path.extension() {
                Some(ex) if ex == os_extension => found.push(path.to_path_buf()),
                _ => {}
            }
        }
        Ok::<(), io::Error>(())
    })?;
    Ok(found)
}

pub(crate) fn assert_equal_vpx(vpx_path: &PathBuf, test_vpx_path: PathBuf) {
    let mut comp = cfb::open(vpx_path).unwrap();
    let mut test_comp = cfb::open(&test_vpx_path).unwrap();

    assert_eq!(comp.version(), test_comp.version());

    // let version = version::read_version(&mut comp).unwrap();
    // println!("version: {:?}", version);

    let original_paths = compound_file_paths_and_lengths(vpx_path);
    let test_paths = compound_file_paths_and_lengths(&test_vpx_path);

    let gamestg_path = Path::new(MAIN_SEPARATOR_STR).join("GameStg");
    let mac_path = gamestg_path.join("MAC");
    let version_path = gamestg_path.join("Version");
    let tableinfo_path = Path::new(MAIN_SEPARATOR_STR).join("TableInfo");

    // sort original paths so that MAC is last
    let original_paths_sorted: Vec<(PathBuf, u64, String)> = original_paths
        .clone()
        .into_iter()
        .filter(|(path, _, _)| *path != mac_path)
        .collect();

    // check all streams
    for (path, _, _) in &original_paths_sorted {
        if comp.is_stream(path) {
            // println!("path: {:?}", path);

            // TODO more precise sound path check

            if *path == mac_path
                || *path == version_path
                || path.starts_with(&tableinfo_path)
                || path.to_string_lossy().contains("Sound")
            {
                let mut original_data = Vec::new();
                let mut test_data = Vec::new();
                let mut original_stream = comp.open_stream(path).unwrap();
                let mut test_stream = test_comp.open_stream(path).unwrap();
                original_stream.read_to_end(&mut original_data).unwrap();
                test_stream.read_to_end(&mut test_data).unwrap();

                if original_data != test_data {
                    let mut file = std::fs::File::create("original.bin").unwrap();
                    file.write_all(&original_data).unwrap();

                    let mut file = std::fs::File::create("test.bin").unwrap();
                    file.write_all(&test_data).unwrap();
                    panic!(
                        "Non equal lengths for {:?} in {} original:{} test:{}, check the files original.bin and test.bin!",
                        path,
                        test_vpx_path.file_name().unwrap().to_string_lossy(),
                        original_data.len(),
                        test_data.len()
                    );
                }
            } else {
                let skip = if path.to_string_lossy().contains("GameItem") {
                    // we need to skip the first 32 bits because they are the type of gameitem
                    4
                } else {
                    0
                };
                let item_tags = tags_and_hashes(&mut comp, path, skip);
                let test_item_tags = tags_and_hashes(&mut test_comp, path, skip);
                if item_tags != test_item_tags {
                    println!(
                        "non equal {:?} for {} vs {}",
                        path,
                        vpx_path.display(),
                        test_vpx_path.display()
                    );
                }
                pretty_assertions::assert_eq!(item_tags, test_item_tags);
            }
        }
    }

    // make sure we have the same paths and lengths
    pretty_assertions::assert_eq!(original_paths, test_paths, "non equal {:?}", vpx_path);
}

fn compound_file_paths_and_lengths(compound_file_path: &Path) -> Vec<(PathBuf, u64, String)> {
    let comp3 = cfb::open(compound_file_path).unwrap();
    comp3
        .walk()
        .filter(|entry| entry.is_stream())
        .map(|entry| {
            let path = entry.path();
            // Hack for compressed items
            // - If a GameItem is a primitive it's vertex data is compressed and the library we use
            // picks the most efficient compression algorithm. This means that the algorithm
            // can be different from what the library used by vpinball picks. So we ignore
            // the size of those items.
            // - If an image is a BMP it's data is compressed, however the algorithm is
            // slightly different from the standard lzw compression.
            // Later on in the test they will get a proper check on the decompressed data
            let size = if path.to_string_lossy().contains("GameItem")
                || path.to_string_lossy().contains("Image")
            {
                // take something obviously recognizable
                123456789
            } else {
                entry.len()
            };
            (path.to_path_buf(), size, entry.clsid().to_string())
        })
        .collect()
}

fn tags_and_hashes<F: Seek + Read>(
    comp: &mut CompoundFile<F>,
    path: &Path,
    skip: u32,
) -> Vec<(String, usize, usize, u64)> {
    let mut data = Vec::new();
    let mut stream = comp.open_stream(path).unwrap();
    stream.read_to_end(&mut data).unwrap();
    // skip skip bytes from the data
    let mut reader = BiffReader::new(&data[(skip as usize)..]);
    reader.disable_warn_remaining();
    biff_tags_and_hashes(&mut reader)
}

fn biff_tags_and_hashes(reader: &mut BiffReader) -> Vec<(String, usize, usize, u64)> {
    let mut tags: Vec<(String, usize, usize, u64)> = Vec::new();
    while let Some(tag) = &reader.next(true) {
        // Some tags have a size 0, where the data needs to be read in a specific way
        // these are mostly also the special cases you see below
        let read_tag_size = reader.remaining_in_record();
        let tag_str = tag.as_str();
        match tag_str {
            "FONT" => {
                let data = reader.data_until("ENDB".as_bytes());
                let hash = hash_data(&data);
                tags.push(("FONT".to_string(), read_tag_size, data.len(), hash));
                // let header = reader.get_data(3).to_owned(); // always? 0x01, 0x0, 0x0
                // let style = reader.get_u8_no_remaining_update();
                // let weight = reader.get_u16_no_remaining_update();
                // let size = reader.get_f32_no_remaining_update();
                // let name_len = reader.get_u8_no_remaining_update();
                // let name = reader.get_str_no_remaining_update(name_len as usize);
                // // reconstruct the bytes that were read
                // let mut data = Vec::new();
                // data.extend_from_slice(&header);
                // data.push(style);
                // data.extend_from_slice(&weight.to_le_bytes());
                // data.extend_from_slice(&size.to_le_bytes());
                // data.push(name_len);
                // data.extend_from_slice(name.as_bytes());
                // let hash = hash_data(&data);
                // println!("FONT: {:?}", name);
                // println!("style: {:?}", style);
                // println!("weight: {:?}", weight);
                // println!("size: {:?}", size);
                // println!("header: {:?}", header);
                // tags.push(("FONT".to_string(), data.len(), hash));
            }
            "JPEG" => {
                let remaining = reader.remaining_in_record();
                tags.push((
                    "--JPEG--SUB--BEGIN--".to_string(),
                    read_tag_size,
                    remaining,
                    0,
                ));
                let mut sub_reader = reader.child_reader();
                while let Some(tag) = &sub_reader.next(true) {
                    let data = sub_reader.get_record_data(false);
                    let mut hasher = DefaultHasher::new();
                    Hash::hash_slice(&data, &mut hasher);
                    let hash = hasher.finish();
                    tags.push((tag.clone(), read_tag_size, data.len(), hash));
                }
                tags.push(("--JPEG--SUB--END--".to_string(), read_tag_size, 0, 0));
                let pos = sub_reader.pos();
                reader.skip_end_tag(pos);
            }
            "BITS" => {
                let data = reader.data_until("ALTV".as_bytes());
                // Looks like vpinball encodes de lzw stream in a slightly different way. Ending
                // up with the same compressed size but different compressed data.
                // However, vpinball can also read the standard lzw stream we write.
                // So for these images we look at the raw data hash.
                let decompressed = from_lzw_blocks(&data);
                let hash = hash_data(&decompressed);
                tags.push((
                    "BITS (decompressed)".to_string(),
                    read_tag_size,
                    decompressed.len(),
                    hash,
                ));
            }
            "CODE" => {
                let len = reader.get_u32_no_remaining_update();
                // at least at the time of 1060, some code was still encoded in latin1
                let data = reader.get_str_with_encoding_no_remaining_update(len as usize);
                let hash = hash_data(data.string.as_bytes());
                tags.push(("CODE".to_string(), read_tag_size, len as usize, hash));
            }
            "MATE" => {
                let data = reader.get_record_data(false);
                // This field in gamedata has padding applied that has random data
                // TODO one solution could be overwriting padding areas with 0's
                // For now we ignore the contents of this field
                let hash = 0;
                tags.push((
                    "MATE (ignored)".to_string(),
                    read_tag_size,
                    data.len(),
                    hash,
                ));
            }
            "PHMA" => {
                let data = reader.get_record_data(false);
                // This field in gamedata has a cstring with fixed length,
                // but again padding is applied that has random data
                // TODO one solution could be overwriting padding areas with 0's
                // For now we ignore the contents of this field
                let hash = 0;
                tags.push((
                    "PHMA (ignored)".to_string(),
                    read_tag_size,
                    data.len(),
                    hash,
                ));
            }
            "M3CY" => {
                // Since the compressed indices size is depending on the selected compression
                // algorithm we can't expect the same size. So we just read the data and ignore it.
                let data = reader.get_record_data(false);
                tags.push(("M3CY (ignored)".to_string(), read_tag_size, data.len(), 0));
            }
            "M3CX" => {
                let decompressed = read_to_end_decompress(reader);
                let hash = hash_data(&decompressed);
                tags.push((
                    "M3CX (decompressed)".to_string(),
                    0, // compressed size ignored
                    decompressed.len(),
                    hash,
                ));
            }
            "M3CJ" => {
                // Since the compressed indices size is depending on the selected compression
                // algorithm we can't expect the same size. So we just read the data and ignore it.
                let data = reader.get_record_data(false);
                tags.push(("M3CJ (ignored)".to_string(), read_tag_size, data.len(), 0));
            }
            "M3CI" => {
                let decompressed = read_to_end_decompress(reader);
                let hash = hash_data(&decompressed);
                tags.push((
                    "M3CI (decompressed)".to_string(),
                    0, // compressed size ignored
                    decompressed.len(),
                    hash,
                ));
            }
            "M3AY" => {
                // Since the compressed indices size is depending on the selected compression
                // algorithm we can't expect the same size. So we just read the data and ignore it.
                let data = reader.get_record_data(false);
                tags.push(("M3AY (ignored)".to_string(), read_tag_size, data.len(), 0));
            }
            "M3AX" => {
                let decompressed = read_to_end_decompress(reader);
                let hash = hash_data(&decompressed);
                tags.push((
                    "M3AX (decompressed)".to_string(),
                    0, // compressed size ignored
                    decompressed.len(),
                    hash,
                ));
            }
            other => {
                let data = reader.get_record_data(false);
                let hash = hash_data(&data);
                tags.push((other.to_string(), read_tag_size, data.len(), hash));
            }
        }
    }
    tags
}

fn hash_data(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    Hash::hash_slice(data, &mut hasher);
    hasher.finish()
}

fn read_to_end_decompress(reader: &mut BiffReader) -> Vec<u8> {
    let compressed_data = reader.get_record_data(false);
    // decompress the data as best compression might be different
    let mut decoder: ZlibDecoder<&[u8]> = ZlibDecoder::new(compressed_data.as_ref());
    let mut data = Vec::new();
    decoder.read_to_end(&mut data).unwrap();
    data
}
