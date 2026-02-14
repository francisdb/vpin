mod common;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod test {
    use crate::common::{assert_equal_vpx, find_files, init_logger, tables_dir};
    use log::info;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
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
        init_logger();
        let folder = tables_dir();
        let paths = find_files(&folder, "vpx")?;

        // Example tables caused problems in the past:
        //
        // * Affected by https://github.com/vpinball/vpinball/pull/2286
        //    - CAPTAINSPAULDINGv1.0.vpx - contains boolean value that is not 0 or 1
        //    - RM054.vpx (Rick & Morty Wip)
        // * invalid bools and out of sync materials
        //    - Ghostbusters LE_4_1 - VLM - VLM - VLM2 - VLM4 - VLM.vpx
        let filtered: Vec<&PathBuf> = paths
            .iter()
            .filter(|path| {
                let name = path.file_name().unwrap().to_str().unwrap();
                !name.contains("CAPTAINSPAULDINGv1.0")
                    && !name.contains("RM054")
                    && !name.contains("Ghostbusters LE_4_1 - VLM - VLM - VLM2 - VLM4 - VLM")
            })
            .collect();
        let counter = AtomicUsize::new(0);
        let total = filtered.len();
        // To run this superfast but no error output
        // use rayon::prelude::*;
        // filtered.par_iter().try_for_each(|vpx_path| {
        filtered.iter().try_for_each(|vpx_path| {
            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
            info!("testing {}/{}: {:?}", n, total, vpx_path);
            let vpx_bytes = std::fs::read(vpx_path)?;
            let original = vpin::vpx::from_bytes(&vpx_bytes)?;
            let test_vpx_bytes = vpin::vpx::to_bytes(&original)?;
            assert_equal_vpx(&vpx_bytes, &test_vpx_bytes, vpx_path);
            Ok(())
        })
    }
}
