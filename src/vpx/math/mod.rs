//! Math utilities ported from VPinball
//!
//! This module provides math operations including:
//! - Matrix transformations (matrix submodule)
//! - Quantization functions (quantize submodule)

mod matrix;
mod quantize;

pub use matrix::*;
pub use quantize::*;
