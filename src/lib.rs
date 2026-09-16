//!
//! Vpin is a library for the virtual/visual pinball ecosystem.
//!
//! It provides a set of tools to work with the various file formats used by the different applications.
//!
//! The main focus is on the Visual Pinball X (VPX) file format, but it also provides tools for backglass DirectB2S and Point of View POV files.

/// The [`FileSystem`](filesystem::FileSystem) abstraction that the
/// extracted directory format is read from and written to, with a real
/// and an in-memory implementation.
pub mod filesystem;
pub(crate) mod gltf;
pub mod vpx;

/// The `wasm-bindgen` entry points of the browser build: extract a table
/// to a file map, audit it, assemble it again and export its meshes.
#[cfg(feature = "wasm")]
pub mod wasm;
