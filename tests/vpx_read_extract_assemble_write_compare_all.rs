mod common;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod test {

    const EXTRACT_IN_MEMORY: bool = true;
    const PRIMITIVE_MESH_FORMAT: PrimitiveMeshFormat = PrimitiveMeshFormat::Obj;

    use crate::common::{find_files, init_logger, render_differences, report_failures, tables_dir};
    use log::info;
    use rayon::prelude::*;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use testdir::testdir;
    use vpin::filesystem::{FileSystem, MemoryFileSystem, RealFileSystem};
    use vpin::vpx::diff::Difference;
    use vpin::vpx::expanded::{ExpandOptions, PrimitiveMeshFormat};

    #[test]
    #[ignore = "slow integration test that only runs on correctly set up machines"]
    fn read_extract_assemble_and_write_all() -> io::Result<()> {
        init_logger();
        let folder = tables_dir();
        let paths = find_files(&folder, "vpx")?;
        // testdir can not be used in non-main threads
        let dir: PathBuf = testdir!();

        // Example tables with performance issues
        // * Dark Chaos
        // * Spooky Wednesday
        // * Street Fighter II (Gottlieb 1993) VPW 1.1
        // * Van Halen (Original 2025) - contains a 200mb mp3 sound file
        // * InvaderTable_2.260.vpx - contains about 5000 gameitems? Chokes the cfb writer.

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
        // * Stranger Things 4 (LPE 1.0 / Premium) - NaN texture coordinates,
        //   their payload bits are now preserved via obj comments
        // * Ghostbusters LE_4_1 - VLM - VLM - VLM2 - VLM4 - VLM.vpx is messed up, invalid bools and out of sync materials
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
                // a per table directory so parallel real filesystem
                // extractions do not collide
                let extract_dir = if EXTRACT_IN_MEMORY {
                    None
                } else {
                    Some(dir.join(format!("extracted_{n}")))
                };
                match table_differences(extract_dir.as_deref(), vpx_path) {
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

    fn table_differences(
        extract_dir: Option<&Path>,
        vpx_path: &PathBuf,
    ) -> io::Result<Vec<Difference>> {
        let original_vpx_bytes = std::fs::read(vpx_path)?;
        let ReadAndWriteResult {
            extracted,
            test_vpx_bytes,
        } = read_and_write_vpx(extract_dir, &original_vpx_bytes)?;
        let differences = vpin::vpx::diff::diff(&original_vpx_bytes, &test_vpx_bytes)?;
        if let Some(extracted) = extracted {
            std::fs::remove_dir_all(extracted)?;
        }
        Ok(differences)
    }

    struct ReadAndWriteResult {
        /// only set if extracted to real filesystem
        extracted: Option<PathBuf>,
        test_vpx_bytes: Vec<u8>,
    }

    fn read_and_write_vpx(
        extract_dir: Option<&Path>,
        original_vpx_bytes: &[u8],
    ) -> io::Result<ReadAndWriteResult> {
        let original = vpin::vpx::from_bytes(original_vpx_bytes)?;
        let (fs, extract_dir): (Box<dyn FileSystem>, PathBuf) = if let Some(dir) = extract_dir {
            std::fs::create_dir_all(dir)?;
            (Box::new(RealFileSystem), dir.to_path_buf())
        } else {
            (Box::new(MemoryFileSystem::new()), PathBuf::from("/vpx"))
        };

        let options = ExpandOptions::new().mesh_format(PRIMITIVE_MESH_FORMAT);
        vpin::vpx::expanded::write_fs(&original, &extract_dir, &options, &*fs)
            .map_err(io::Error::other)?;
        let expanded_read =
            vpin::vpx::expanded::read_fs(&extract_dir, &*fs).map_err(io::Error::other)?;
        // special case for comparing code, the diff of the written file
        // would also catch this but with a less precise message
        if original.gamedata.code != expanded_read.gamedata.code {
            return Err(io::Error::other(
                "script differs after the expanded round trip",
            ));
        }
        // several tables can be in flight at once, free the parsed model
        // and the expanded files before serializing the assembled copy
        drop(original);
        drop(fs);

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
