use rayon::prelude::*;
use std::io;
use std::io::{Error, ErrorKind, Read};
use std::path::PathBuf;
use vpin::directb2s;
use vpin::directb2s::DirectB2SData;

mod common;

#[test]
#[ignore = "slow integration test that only runs on correctly set up machines"]
fn read_all() -> io::Result<()> {
    let home = dirs::home_dir().expect("no home dir");
    let folder = home.join("vpinball").join("tables");
    if !folder.exists() {
        panic!("folder does not exist: {:?}", folder);
    }
    let paths = common::find_files(&folder, "directb2s")?;

    paths.par_iter().try_for_each(|path| {
        println!("testing: {:?}", path);

        // read file to string
        let loaded = read_directb2s(&path)?;

        // print name, version and type
        println!("name: {}", loaded.name.value);
        println!("version: {}", loaded.version);
        println!("dmd type: {}", loaded.dmd_type.value);
        Ok(())
    })
}

fn read_directb2s(path: &PathBuf) -> Result<DirectB2SData, Error> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    directb2s::read(reader).map_err(|e| {
        let msg = format!("Error for {}: {}", path.display(), e);
        io::Error::new(ErrorKind::Other, msg)
    })
}
