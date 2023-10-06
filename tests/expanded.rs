// TODO write test that extracts testdata table, reassambles it and compares it to the original

use std::io;
use std::path::PathBuf;
use testdir::testdir;
use vpin::vpx::expanded::extract;

#[test]
fn vpx_to_expanded_to_vpx() -> io::Result<()> {
    // let home = dirs::home_dir().expect("no home dir");
    // let folder = home.join("vpinball").join("tables");
    // if !folder.exists() {
    //     panic!("folder does not exist: {:?}", folder);
    // }
    // let paths = find_vpx_files(true, &folder)?;

    let paths = ["testdata/completely_blank_table_10_7_4.vpx"];

    let dir: PathBuf = testdir!();

    paths.iter().try_for_each(|path_str| {
        let path = PathBuf::from(path_str);
        println!("testing: {:?}", path);
        extract(&path, &dir)?;

        //assert_equal_vpx(path, test_vpx_path);
        Ok(())
    })
}
