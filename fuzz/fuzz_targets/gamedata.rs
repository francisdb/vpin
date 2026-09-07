//! Fuzzes the BIFF gamedata parser directly, bypassing the CFB container so
//! the fuzzer spends its time in vpin's own record parsing instead of in the
//! container format. Parse errors are expected; panics are bugs.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vpin::vpx::gamedata::read_all_gamedata_records;
use vpin::vpx::version::Version;

fuzz_target!(|data: &[u8]| {
    let _ = read_all_gamedata_records(data, &Version::new(1092));
});
