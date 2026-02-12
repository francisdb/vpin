//! Math utilities ported from VPinball
//!
//! This module provides math operations including:
//! - Vector types (vec submodule)
//! - Matrix transformations (matrix submodule)
//! - Quantization functions (quantize submodule)

mod matrix;
mod quantize;
mod vec;

pub use matrix::*;
pub use quantize::*;
pub use vec::*;
