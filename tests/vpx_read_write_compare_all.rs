use cfb::CompoundFile;
use pretty_assertions::assert_eq;
use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use testdir::testdir;
use testresult::TestResult;
use vpin::vpx::biff::BiffReader;

mod common;

#[test]
fn read_and_write() -> TestResult {
    let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
    let original = vpin::vpx::read(&path)?;

    // create temp file and write the vpx to it
    let dir: PathBuf = testdir!();
    let test_vpx_path = dir.join("test.vpx");
    vpin::vpx::write(&test_vpx_path, &original)?;

    assert_equal_vpx(&path, test_vpx_path);
    Ok(())
}

#[test]
#[ignore = "slow integration test that only runs on correctly set up machines"]
fn read_and_write_all() -> io::Result<()> {
    let home = dirs::home_dir().expect("no home dir");
    let folder = home.join("vpinball").join("tables");
    if !folder.exists() {
        panic!("folder does not exist: {:?}", folder);
    }
    let paths = common::find_files(&folder, "vpx")?;
    // testdir can not be used in non-main threads
    let dir: PathBuf = testdir!();
    // TODO why is par_iter() not faster but just consuming all cpu cores?
    paths
        .iter()
        // .filter(|p| {
        //     p.file_name()
        //         .unwrap()
        //         .to_string_lossy()
        //         .to_ascii_lowercase()
        //         .contains("diehard")
        // })
        .try_for_each(|path| {
            println!("testing: {:?}", path);
            let test_vpx_path = read_and_write_vpx(&dir, &path)?;

            assert_equal_vpx(path, test_vpx_path);
            Ok(())
        })
}

fn read_and_write_vpx(dir: &PathBuf, path: &Path) -> io::Result<PathBuf> {
    let original = vpin::vpx::read(&path.to_path_buf())?;
    // create temp file and write the vpx to it
    let file_name = path.file_name().unwrap();
    let test_vpx_path = dir.join(file_name);
    vpin::vpx::write(&test_vpx_path, &original)?;
    Ok(test_vpx_path)
}

fn assert_equal_vpx(vpx_path: &PathBuf, test_vpx_path: PathBuf) {
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
            println!("path: {:?}", path);

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

                assert!(original_data == test_data);
            } else {
                let skip = if path.to_string_lossy().contains("GameItem") {
                    // we need to skip the first 32 bits because they are the type of gameitem
                    4
                } else {
                    0
                };
                let item_tags = tags_and_hashes(&mut comp, path, skip);
                let test_item_tags = tags_and_hashes(&mut test_comp, path, skip);
                assert_eq!(item_tags, test_item_tags);
            }
        }
    }

    // make sure we have the same paths and lengths
    assert_eq!(original_paths, test_paths, "non equal {:?}", vpx_path);
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
                // at least a the time of 1060, some code was still encoded in latin1
                let data = reader.get_str_with_encoding_no_remaining_update(len as usize);
                let mut hasher = DefaultHasher::new();
                Hash::hash_slice(&data.string.as_bytes(), &mut hasher);
                let hash = hasher.finish();
                tags.push(("CODE".to_string(), len as usize, hash));
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
