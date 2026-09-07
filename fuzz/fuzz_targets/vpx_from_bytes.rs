//! Fuzzes the full VPX read path: CFB compound-file container plus every
//! stream parser behind it (gamedata, gameitems, images, sounds, fonts,
//! collections). Parse errors are expected on garbage input; panics are bugs.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = vpin::vpx::from_bytes(data);
});
