mod common;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod test {

    const EXTRACT_IN_MEMORY: bool = true;

    use pretty_assertions::assert_eq;
    // TODO once we can capture logs per extract / assemble we can re-enable parallel tests
    // use rayon::prelude::*;
    use crate::common::{assert_equal_vpx, find_files, tables_dir};
    use log::info;
    use std::io;
    use std::path::{Path, PathBuf};
    use testdir::testdir;
    use vpin::filesystem::{FileSystem, MemoryFileSystem, RealFileSystem};
    use vpin::vpx::expanded::PrimitiveMeshFormat;

    fn init() {
        use crate::common::tracing_duration_filter::DurationFilterLayer;
        use std::time::Duration;
        use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

        // let _ = env_logger::builder()
        //     .is_test(true)
        //     .filter_level(log::LevelFilter::Info)
        //     .try_init();

        // let _ = fmt()
        //     .with_env_filter(
        //         EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        //     )
        //     .with_test_writer()
        //     .with_span_events(fmt::format::FmtSpan::CLOSE)
        //     .try_init();

        let _ = tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with(
                fmt::layer()
                    .with_test_writer()
                    // Enable colored output
                    .with_ansi(true), // Show timing when spans close
                                      //.with_span_events(fmt::format::FmtSpan::CLOSE),
            )
            .with(DurationFilterLayer::new(Duration::from_millis(300)))
            .try_init();
    }

    #[test]
    #[ignore = "slow integration test that only runs on correctly set up machines"]
    fn read_extract_assemble_and_write_all() -> io::Result<()> {
        init();
        let folder = tables_dir();
        let paths = find_files(&folder, "vpx")?;
        // testdir can not be used in non-main threads
        let dir: PathBuf = testdir!();

        // Example tables with performance issues
        // * Dark Chaos
        // * Spooky Wednesday
        // * Street Fighter II (Gottlieb 1993) VPW 1.1
        // * Van Halen (Original 2025) - contains a 200mb mp3 sound file

        // Example tables caused problems in the past:
        //
        // * Inhabiting Mars RC 4 - for animation frame
        // * DieHard_272.vpx - primitive "BM_pAirDuctGate" has a NaN value for nx
        // * Johnny Mnemonic (Williams 1995) VPW v1.0.2.vpx - animated frames that overlap with primitive names
        // * Future Spa (Bally 1979) v4.3.vpx - NaN in table setup values
        // * InvaderTable_2.260.vpx - Symbol fonts
        // * Guns N Roses (Data East 1994).vpx - contains BMP with non-255 alpha values
        // * Affected by https://github.com/vpinball/vpinball/pull/2286
        //    - CAPTAINSPAULDINGv1.0.vpx - contains boolean value that is not 0 or 1
        //    - RM054.vpx (Rick & Morty Wip)
        // * TODO something with the M3CX
        //    - Stranger Things 4 LPE 1.0 (Limited PRO Edition).vpx something with the M3CX
        //    - Stranger Things 4 Premium.vpx also has problems with the M3CX
        let filtered: Vec<&PathBuf> = paths
            .iter()
            .filter(|path| {
                let name = path.file_name().unwrap().to_str().unwrap();
                !name.contains("CAPTAINSPAULDINGv1.0")
                    && !name.contains("RM054")
                    && !name.contains("Stranger Things 4")
            })
            .collect();

        // TODO why is par_iter() not faster but just consuming all cpu cores?
        filtered.iter().enumerate().try_for_each(|(n, path)| {
            info!("testing {}/{}: {:?}", n + 1, filtered.len(), path);
            let original_vpx_bytes = std::fs::read(path)?;
            let extract_dir = if EXTRACT_IN_MEMORY {
                None
            } else {
                Some(&dir as &Path)
            };
            let ReadAndWriteResult {
                extracted,
                test_vpx_bytes,
            } = read_and_write_vpx(extract_dir, &original_vpx_bytes)?;
            assert_equal_vpx(&original_vpx_bytes, &test_vpx_bytes, path);
            if let Some(extracted) = extracted {
                std::fs::remove_dir_all(extracted)?;
            }
            Ok(())
        })
    }

    struct ReadAndWriteResult {
        /// only set if extracted to real filesystem
        extracted: Option<PathBuf>,
        test_vpx_bytes: Vec<u8>,
    }

    fn read_and_write_vpx(
        extractr_dir: Option<&Path>,
        original_vpx_bytes: &[u8],
    ) -> io::Result<ReadAndWriteResult> {
        let original = vpin::vpx::from_bytes(original_vpx_bytes)?;
        let (fs, extract_dir): (Box<dyn FileSystem>, PathBuf) = if let Some(dir) = extractr_dir {
            let extract_dir = dir.join("extracted");
            std::fs::create_dir_all(&extract_dir)?;
            (Box::new(RealFileSystem), extract_dir)
        } else {
            (Box::new(MemoryFileSystem::new()), PathBuf::from("/vpx"))
        };

        vpin::vpx::expanded::write_fs(&original, &extract_dir, PrimitiveMeshFormat::Obj, &*fs)
            .map_err(io::Error::other)?;
        let expanded_read =
            vpin::vpx::expanded::read_fs(&extract_dir, &*fs).map_err(io::Error::other)?;
        // special case for comparing code
        assert_eq!(original.gamedata.code, expanded_read.gamedata.code);

        let test_vpx_bytes = vpin::vpx::to_bytes(&expanded_read)?;
        Ok(ReadAndWriteResult {
            extracted: if EXTRACT_IN_MEMORY {
                None
            } else {
                Some(extract_dir)
            },
            test_vpx_bytes,
        })
    }
}
