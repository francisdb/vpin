//!
//! Vpin is a library for the virtual/visual pinball ecosystem.
//!
//! It provides a set of tools to work with the various file formats used by the different applications.
//!
//! The main focus is on the Visual Pinball X (VPX) file format: reading and
//! writing tables, extracting them to and assembling them from a directory
//! tree, and exporting whole tables to OBJ or glTF/GLB.

pub mod filesystem;
pub(crate) mod gltf;
pub mod vpx;

#[cfg(feature = "wasm")]
pub mod wasm;
