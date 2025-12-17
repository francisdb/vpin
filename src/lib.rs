//!
//! Vpin is a library for the virtual/visual pinball ecosystem.
//!
//! It provides a set of tools to work with the various file formats used by the different applications.
//!
//! The main focus is on the Visual Pinball X (VPX) file format, but it also provides tools for backglass DirectB2S and Point of View POV files.

pub mod directb2s;

pub mod vpx;
pub(crate) mod wavefront_obj_io;
