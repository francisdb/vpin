//! Fuzzes the game item parser directly with raw BIFF bytes. The first four
//! bytes select the item type, so the fuzzer explores all item parsers.
//! Parse errors are expected; panics are bugs.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = vpin::vpx::gameitem::read(data);
});
