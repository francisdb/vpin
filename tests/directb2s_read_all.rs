use rayon::prelude::*;
use std::io;
use std::io::{ErrorKind, Read};
use vpin::directb2s;

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
        let mut file = std::fs::File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let loaded = directb2s::load(&contents).map_err(|e| {
            let msg = format!("Error for {}: {}", path.display(), e);
            io::Error::new(ErrorKind::Other, msg)
        })?;

        // print name, version and type
        println!("name: {}", loaded.name.value);
        println!("version: {}", loaded.version);
        println!("dmd type: {}", loaded.dmd_type.value);
        Ok(())
    })
}
