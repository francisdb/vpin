mod common;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod test {

    // use pretty_assertions::assert_eq;
    // use rayon::prelude::*;
    use crate::common::{assert_equal_vpx, find_files, tables_dir};
    use std::io;
    use std::path::{Path, PathBuf};
    use testdir::testdir;
    use testresult::TestResult;

    #[test]
    fn read_and_write() -> TestResult {
        let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
        let original = vpin::vpx::read(&path)?;

        // create temp file and write the vpx to it
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        vpin::vpx::write(&test_vpx_path, &original)?;

        let vpx_bytes = std::fs::read(&test_vpx_path)?;
        let test_vpx_bytes = std::fs::read(&test_vpx_path)?;
        assert_equal_vpx(&vpx_bytes, &test_vpx_bytes, &path);
        Ok(())
    }

    #[test]
    #[ignore = "slow integration test that only runs on correctly set up machines"]
    fn read_and_write_all() -> io::Result<()> {
        let folder = tables_dir();
        let paths = find_files(&folder, "vpx")?;
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
            .try_for_each(|vpx_path| {
                println!("testing: {vpx_path:?}");
                let test_vpx_path = read_and_write_vpx(&dir, vpx_path)?;
                let vpx_bytes = std::fs::read(vpx_path)?;
                let test_vpx_bytes = std::fs::read(&test_vpx_path)?;
                assert_equal_vpx(&vpx_bytes, &test_vpx_bytes, vpx_path);
                // if all is good we remove the test file
                std::fs::remove_file(&test_vpx_path)?;
                Ok(())
            })
    }

    fn read_and_write_vpx(dir: &Path, path: &Path) -> io::Result<PathBuf> {
        let original = vpin::vpx::read(path)?;
        // create temp file and write the vpx to it
        let file_name = path.file_name().unwrap();
        let test_vpx_path = dir.join(file_name);
        vpin::vpx::write(&test_vpx_path, &original)?;
        Ok(test_vpx_path)
    }
}
