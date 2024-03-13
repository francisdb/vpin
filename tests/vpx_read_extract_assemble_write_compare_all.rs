mod common;

#[cfg(test)]
mod test {

    use pretty_assertions::assert_eq;
    // use rayon::prelude::*;
    use crate::common::{assert_equal_vpx, find_files};
    use std::io;
    use std::path::{Path, PathBuf};
    use testdir::testdir;

    #[test]
    #[ignore = "slow integration test that only runs on correctly set up machines"]
    fn read_extract_assemble_and_write_all() -> io::Result<()> {
        let home = dirs::home_dir().expect("no home dir");
        let folder = home.join("vpinball").join("tables");
        if !folder.exists() {
            panic!("folder does not exist: {:?}", folder);
        }
        let paths = find_files(&folder, "vpx")?;
        // testdir can not be used in non-main threads
        let dir: PathBuf = testdir!();
        // TODO why is par_iter() not faster but just consuming all cpu cores?
        paths
            .iter()
            .filter(|path| {
                let name = path.file_name().unwrap().to_str().unwrap();
                name.contains("Addams")
            })
            .try_for_each(|path| {
                println!("testing: {:?}", path);
                let ReadAndWriteResult {
                    extracted,
                    test_vpx,
                } = read_and_write_vpx(&dir, &path)?;
                assert_equal_vpx(path, test_vpx.clone());
                // if all is good we remove the test file and the extracted dir
                std::fs::remove_file(&test_vpx)?;
                std::fs::remove_dir_all(&extracted)?;
                Ok(())
            })
    }

    struct ReadAndWriteResult {
        extracted: PathBuf,
        test_vpx: PathBuf,
    }

    fn read_and_write_vpx(dir: &PathBuf, path: &Path) -> io::Result<ReadAndWriteResult> {
        let original = vpin::vpx::read(&path.to_path_buf())?;
        let extract_dir = dir.join("extracted");
        // make dir
        std::fs::create_dir_all(&extract_dir)?;
        vpin::vpx::expanded::write(&original, &extract_dir)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let expanded_read = vpin::vpx::expanded::read(&extract_dir)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        // special case for comparing code
        assert_eq!(original.gamedata.code, expanded_read.gamedata.code);
        let file_name = path.file_name().unwrap();
        let test_vpx_path = dir.join(file_name);
        vpin::vpx::write(&test_vpx_path, &expanded_read)?;
        Ok(ReadAndWriteResult {
            extracted: extract_dir,
            test_vpx: test_vpx_path,
        })
    }
}
