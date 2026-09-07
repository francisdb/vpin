mod common;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod test {
    use crate::common::{
        assert_equal_vpx, find_files, init_logger, render_differences, report_failures, tables_dir,
    };
    use log::info;
    use rayon::prelude::*;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use testdir::testdir;
    use testresult::TestResult;
    use vpin::vpx::diff::Difference;

    #[test]
    fn read_and_write() -> TestResult {
        let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
        let original = vpin::vpx::read(&path)?;

        // create temp file and write the vpx to it
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        vpin::vpx::write(&test_vpx_path, &original)?;

        let vpx_bytes = std::fs::read(&path)?;
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
        // Every table is checked and all failures are reported together at
        // the end. The per-table tracing span ties each log line a parser
        // emits to the table it came from, since parallel output interleaves.
        let failures: Vec<(PathBuf, String)> = filtered
            .par_iter()
            .filter_map(|vpx_path| {
                let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
                let file = vpx_path.file_name().unwrap().to_string_lossy().to_string();
                // a WARN level span so the table prefix also survives a
                // warnings-only filter like RUST_LOG=warn
                let _guard = tracing::span!(tracing::Level::WARN, "table", %file).entered();
                info!("testing {n}/{total}");
                match table_differences(vpx_path) {
                    Ok(differences) if differences.is_empty() => None,
                    Ok(differences) => {
                        Some(((*vpx_path).clone(), render_differences(&differences)))
                    }
                    Err(e) => Some(((*vpx_path).clone(), format!("error: {e}"))),
                }
            })
            .collect();

        report_failures(&failures);
        Ok(())
    }

    fn table_differences(vpx_path: &PathBuf) -> io::Result<Vec<Difference>> {
        let vpx_bytes = std::fs::read(vpx_path)?;
        let original = vpin::vpx::from_bytes(&vpx_bytes)?;
        let test_vpx_bytes = vpin::vpx::to_bytes(&original)?;
        vpin::vpx::diff::diff(&vpx_bytes, &test_vpx_bytes)
    }
}
