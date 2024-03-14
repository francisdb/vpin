use cfb::CompoundFile;
use std::ffi::OsStr;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use vpin::vpx::biff::BiffReader;
use walkdir::WalkDir;

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn assert_equal_vpx(vpx_path: &PathBuf, test_vpx_path: PathBuf) {
    let mut comp = cfb::open(&vpx_path).unwrap();
    let mut test_comp = cfb::open(&test_vpx_path).unwrap();

    // let version = version::read_version(&mut comp).unwrap();
    // println!("version: {:?}", version);

    let original_paths = compound_file_paths_and_lengths(&vpx_path);
    let test_paths = compound_file_paths_and_lengths(&test_vpx_path);

    let gamestg_path = Path::new(MAIN_SEPARATOR_STR).join("GameStg");
    let mac_path = gamestg_path.join("MAC");
    let version_path = gamestg_path.join("Version");
    let tableinfo_path = Path::new(MAIN_SEPARATOR_STR).join("TableInfo");

    // sort original paths so that MAC is last
    let original_paths_sorted: Vec<(PathBuf, u64)> = original_paths
        .clone()
        .into_iter()
        .filter(|(path, _)| *path != mac_path)
        .collect();

    // check all streams
    for (path, _) in &original_paths_sorted {
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

                // let mut file = std::fs::File::create("original.bin").unwrap();
                // file.write_all(&original_data).unwrap();

                // let mut file = std::fs::File::create("test.bin").unwrap();
                // file.write_all(&test_data).unwrap();

                assert!(
                    original_data == test_data,
                    "non equal {:?} original:{} test:{} ",
                    path,
                    original_data.len(),
                    test_data.len()
                );
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

fn compound_file_paths_and_lengths(compound_file_path: &Path) -> Vec<(PathBuf, u64)> {
    let comp3 = cfb::open(compound_file_path).unwrap();
    comp3
        .walk()
        .map(|entry| {
            let path = entry.path();
            let size = entry.len();
            (path.to_path_buf(), size)
        })
        .collect()
}

fn tags_and_hashes<F: Seek + Read>(
    comp: &mut CompoundFile<F>,
    path: &Path,
    skip: u32,
) -> Vec<(String, usize, u64)> {
    let mut data = Vec::new();
    let mut stream = comp.open_stream(path).unwrap();
    stream.read_to_end(&mut data).unwrap();
    // skip skip bytes from the data
    let mut reader = BiffReader::new(&data[(skip as usize)..]);
    reader.disable_warn_remaining();
    biff_tags_and_hashes(&mut reader)
}

fn biff_tags_and_hashes(reader: &mut BiffReader) -> Vec<(String, usize, u64)> {
    let mut tags: Vec<(String, usize, u64)> = Vec::new();
    while let Some(tag) = &reader.next(true) {
        let tag_str = tag.as_str();
        match tag_str {
            "FONT" => {
                let _header = reader.get_data(3); // always? 0x01, 0x0, 0x0
                let _style = reader.get_u8_no_remaining_update();
                let _weight = reader.get_u16_no_remaining_update();
                let _size = reader.get_u32_no_remaining_update();
                let name_len = reader.get_u8_no_remaining_update();
                let _name = reader.get_str_no_remaining_update(name_len as usize);
            }
            "JPEG" => {
                tags.push(("--JPEG--SUB--BEGIN--".to_string(), 0, 0));
                let mut sub_reader = reader.child_reader();
                while let Some(tag) = &sub_reader.next(true) {
                    let data = sub_reader.get_record_data(false);
                    let mut hasher = DefaultHasher::new();
                    Hash::hash_slice(&data, &mut hasher);
                    let hash = hasher.finish();
                    tags.push((tag.clone(), data.len(), hash));
                }
                tags.push(("--JPEG--SUB--END--".to_string(), 0, 0));
                let pos = sub_reader.pos();
                reader.skip_end_tag(pos);
            }
            "BITS" => {
                let data = reader.data_until("ALTV".as_bytes());
                let mut hasher = DefaultHasher::new();
                Hash::hash_slice(&data, &mut hasher);
                let hash = hasher.finish();
                tags.push(("BITS".to_string(), data.len(), hash));
            }
            "CODE" => {
                let len = reader.get_u32_no_remaining_update();
                // at least at the time of 1060, some code was still encoded in latin1
                let data = reader.get_str_with_encoding_no_remaining_update(len as usize);
                let mut hasher = DefaultHasher::new();
                Hash::hash_slice(&data.string.as_bytes(), &mut hasher);
                let hash = hasher.finish();
                tags.push(("CODE".to_string(), len as usize, hash));
            }
            "MATE" => {
                let data = reader.get_record_data(false);
                // let mut hasher = DefaultHasher::new();
                // Hash::hash_slice(&data, &mut hasher);
                // let hash = hasher.finish();

                // This field in gamedata has padding applied that has random data
                // TODO one solution could be overwriting padding areas with 0's
                // For now we ignore the contents of this field
                tags.push(("MATE".to_string(), data.len(), 0));
            }
            "PHMA" => {
                let data = reader.get_record_data(false);
                // let mut hasher = DefaultHasher::new();
                // Hash::hash_slice(&data, &mut hasher);
                // let hash = hasher.finish();

                // This field in gamedata has a cstring with fixed length,
                // but again padding is applied that has random data
                // TODO one solution could be overwriting padding areas with 0's
                // For now we ignore the contents of this field
                tags.push(("PHMA".to_string(), data.len(), 0));
            }
            other => {
                let data = reader.get_record_data(false);
                let mut hasher = DefaultHasher::new();
                Hash::hash_slice(&data, &mut hasher);
                let hash = hasher.finish();
                tags.push((other.to_string(), data.len(), hash));
            }
        }
    }
    tags
}
