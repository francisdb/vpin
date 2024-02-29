use cfb::CompoundFile;
use pretty_assertions::assert_eq;
// use rayon::prelude::*;
use crate::common::assert_equal_vpx;
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
#[ignore = "slow integration test that only runs on correctly set up machines"]
fn read_extract_assemble_and_write_all() -> io::Result<()> {
    let home = dirs::home_dir().expect("no home dir");
    let folder = home.join("vpinball").join("tables");
    if !folder.exists() {
        panic!("folder does not exist: {:?}", folder);
    }
    let paths = common::find_files(&folder, "vpx")?;
    // testdir can not be used in non-main threads
    let dir: PathBuf = testdir!();
    // TODO why is par_iter() not faster but just consuming all cpu cores?
    paths.iter().try_for_each(|path| {
        println!("testing: {:?}", path);
        let test_vpx_path = read_and_write_vpx(&dir, &path)?;
        assert_equal_vpx(path, test_vpx_path);
        Ok(())
    })
}

fn read_and_write_vpx(dir: &PathBuf, path: &Path) -> io::Result<PathBuf> {
    let original = vpin::vpx::read(&path.to_path_buf())?;
    let extract_dir = dir.join("extracted");
    // make dir
    std::fs::create_dir_all(&extract_dir)?;
    vpin::vpx::expanded::write(&original, &extract_dir)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let expanded_read = vpin::vpx::expanded::read(&extract_dir)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    // create temp file and write the vpx to it
    let file_name = path.file_name().unwrap();
    let test_vpx_path = dir.join(file_name);
    vpin::vpx::write(&test_vpx_path, &expanded_read)?;
    Ok(test_vpx_path)
}