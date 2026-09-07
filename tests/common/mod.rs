// Since this code is only used for tests, and not all tests use all function we allow dead code.
#![allow(dead_code)]
#![cfg(test)]

pub mod tracing_duration_filter;

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub(crate) fn init_logger() {
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

/// if TABLES_DIR is set use that
/// otherwise use ~/vpinball/tables
pub(crate) fn tables_dir() -> PathBuf {
    let (folder, used_env) = if let Ok(tables_dir) = std::env::var("TABLES_DIR") {
        (PathBuf::from(tables_dir), true)
    } else {
        let home = dirs::home_dir().expect("no home dir");
        (home.join("vpinball").join("tables"), false)
    };
    if !folder.exists() {
        if used_env {
            panic!(
                "Tables folder does not exist: {folder:?}\n\
                The path was determined from the TABLES_DIR environment variable.\n\
                Please check that TABLES_DIR is set correctly and the directory exists."
            );
        } else {
            panic!(
                "Tables folder does not exist: {folder:?}\n\
                The path was determined from the default location ($HOME/vpinball/tables).\n\
                You can set the TABLES_DIR environment variable to override this location:\n\
                export TABLES_DIR=/path/to/your/tables"
            );
        }
    }
    folder
}

pub(crate) fn find_files<P: AsRef<Path>>(
    tables_path: P,
    extension: &str,
) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut entries = WalkDir::new(tables_path).into_iter();
    let os_extension = OsStr::new(extension);
    entries.try_for_each(|entry| {
        let dir_entry = entry?;
        let path = dir_entry.path();
        if path.is_file() {
            match path.extension() {
                Some(ex) if ex == os_extension => found.push(path.to_path_buf()),
                _ => {}
            }
        }
        Ok::<(), io::Error>(())
    })?;
    Ok(found)
}

pub(crate) fn assert_equal_vpx(vpx_bytes: &[u8], test_vpx_bytes: &[u8], vpx_path: &Path) {
    let differences = vpin::vpx::diff::diff(vpx_bytes, test_vpx_bytes)
        .unwrap_or_else(|e| panic!("diff failed for {}: {e}", vpx_path.display()));
    if !differences.is_empty() {
        let rendered = differences
            .iter()
            .map(|d| format!("  {d}"))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "{} difference(s) for {}:\n{rendered}",
            differences.len(),
            vpx_path.display()
        );
    }
}

/// Renders differences for a failure report, capped so a heavily broken
/// table does not flood the output
pub(crate) fn render_differences(differences: &[vpin::vpx::diff::Difference]) -> String {
    const LIMIT: usize = 20;
    let mut lines: Vec<String> = differences
        .iter()
        .take(LIMIT)
        .map(|d| format!("  {d}"))
        .collect();
    if differences.len() > LIMIT {
        lines.push(format!("  ... and {} more", differences.len() - LIMIT));
    }
    lines.join("\n")
}

/// Panics with a per table report when any table failed
pub(crate) fn report_failures(failures: &[(PathBuf, String)]) {
    if !failures.is_empty() {
        let report = failures
            .iter()
            .map(|(path, failure)| format!("{}:\n{failure}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        panic!("{} table(s) failed:\n{report}", failures.len());
    }
}
