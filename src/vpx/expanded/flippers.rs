//! Flipper mesh generation for expanded VPX export
//!
//! This module ports the flipper mesh generation from Visual Pinball's flipper.cpp.
//! Flippers use a pre-defined base mesh that is scaled and transformed based on
//! the flipper's parameters (base radius, end radius, length, height, etc.).
//!
//! Ported from: VisualPinball.Engine/VPT/Flipper/FlipperMeshGenerator.cs
//! Original C++: VPinball/src/parts/flipper.cpp

use super::mesh_common::{Matrix3D, Vec2, Vec3, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::flipper::Flipper;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

/// Number of vertices in the flipper base mesh
const FLIPPER_BASE_VERTICES: usize = 104;

/// Number of indices in the flipper base mesh
const FLIPPER_BASE_NUM_INDICES: usize = 300;

/// Result of flipper mesh generation with separate base and rubber meshes
pub struct FlipperMeshes {
    /// The base flipper mesh (uses flipper.material)
    pub base: (Vec<VertexWrapper>, Vec<VpxFace>),
    /// The rubber mesh on top of the flipper (uses flipper.rubber_material)
    /// Only present if rubber_thickness > 0
    pub rubber: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// Pre-defined flipper base mesh vertices
/// From VPinball src/meshes/flipperBase.h
#[rustfmt::skip]
#[allow(clippy::approx_constant)]
static FLIPPER_BASE_MESH: [Vertex3dNoTex2; FLIPPER_BASE_VERTICES] = [
    Vertex3dNoTex2 { x: -0.101425, y: 0.786319, z: 0.003753, nx: -0.997900, ny: 0.065000, nz: 0.0, tu: 0.126235, tv: 0.422635 },
    Vertex3dNoTex2 { x: -0.101425, y: 0.786319, z: 1.004253, nx: -0.997900, ny: 0.065000, nz: 0.0, tu: 0.068619, tv: 0.486620 },
    Vertex3dNoTex2 { x: -0.097969, y: 0.812569, z: 1.004253, nx: -0.965900, ny: 0.258800, nz: 0.0, tu: 0.055945, tv: 0.474362 },
    Vertex3dNoTex2 { x: -0.097969, y: 0.812569, z: 0.003753, nx: -0.965900, ny: 0.258800, nz: 0.0, tu: 0.113479, tv: 0.420353 },
    Vertex3dNoTex2 { x: -0.050713, y: 0.874155, z: 0.003753, nx: -0.500000, ny: 0.866000, nz: 0.0, tu: 0.095408, tv: 0.402450 },
    Vertex3dNoTex2 { x: -0.050713, y: 0.874155, z: 1.004253, nx: -0.500000, ny: 0.866000, nz: 0.0, tu: 0.024611, tv: 0.428958 },
    Vertex3dNoTex2 { x: -0.026251, y: 0.884288, z: 1.004253, nx: -0.258800, ny: 0.965900, nz: 0.0, tu: 0.018144, tv: 0.410811 },
    Vertex3dNoTex2 { x: -0.026251, y: 0.884288, z: 0.003753, nx: -0.258800, ny: 0.965900, nz: 0.0, tu: 0.092619, tv: 0.394091 },
    Vertex3dNoTex2 { x: 0.050713, y: 0.874155, z: 0.003753, nx: 0.500000, ny: 0.866000, nz: 0.0, tu: 0.094255, tv: 0.367888 },
    Vertex3dNoTex2 { x: 0.050713, y: 0.874155, z: 1.004253, nx: 0.500000, ny: 0.866000, nz: 0.0, tu: 0.015402, tv: 0.351824 },
    Vertex3dNoTex2 { x: 0.071718, y: 0.858037, z: 1.004253, nx: 0.707100, ny: 0.707100, nz: 0.0, tu: 0.020311, tv: 0.332136 },
    Vertex3dNoTex2 { x: 0.071718, y: 0.858037, z: 0.003753, nx: 0.707100, ny: 0.707100, nz: 0.0, tu: 0.098450, tv: 0.360184 },
    Vertex3dNoTex2 { x: -0.050713, y: 0.874155, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.020766, tv: 0.089615 },
    Vertex3dNoTex2 { x: -0.026251, y: 0.884288, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.015631, tv: 0.077247 },
    Vertex3dNoTex2 { x: 0.0, y: 0.887744, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.013871, tv: 0.063972 },
    Vertex3dNoTex2 { x: 0.026251, y: 0.884288, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.015608, tv: 0.050694 },
    Vertex3dNoTex2 { x: 0.050713, y: 0.874155, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.020722, tv: 0.038317 },
    Vertex3dNoTex2 { x: 0.071718, y: 0.858037, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.028865, tv: 0.027686 },
    Vertex3dNoTex2 { x: 0.087837, y: 0.837031, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.039483, tv: 0.019525 },
    Vertex3dNoTex2 { x: 0.097969, y: 0.812569, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.051850, tv: 0.014390 },
    Vertex3dNoTex2 { x: 0.101425, y: 0.786319, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.065126, tv: 0.012631 },
    Vertex3dNoTex2 { x: 0.100762, y: 0.0, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.462821, tv: 0.012629 },
    Vertex3dNoTex2 { x: 0.097329, y: -0.026079, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.476012, tv: 0.014355 },
    Vertex3dNoTex2 { x: 0.087263, y: -0.050381, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.488308, tv: 0.019436 },
    Vertex3dNoTex2 { x: 0.071250, y: -0.071250, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.498869, tv: 0.027526 },
    Vertex3dNoTex2 { x: 0.050381, y: -0.087263, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.506977, tv: 0.038073 },
    Vertex3dNoTex2 { x: 0.026079, y: -0.097329, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.512079, tv: 0.050360 },
    Vertex3dNoTex2 { x: 0.0, y: -0.100762, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.513826, tv: 0.063549 },
    Vertex3dNoTex2 { x: -0.026079, y: -0.097329, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.512101, tv: 0.076740 },
    Vertex3dNoTex2 { x: -0.050381, y: -0.087263, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.507020, tv: 0.089036 },
    Vertex3dNoTex2 { x: -0.071250, y: -0.071250, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.498930, tv: 0.099597 },
    Vertex3dNoTex2 { x: -0.087263, y: -0.050381, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.488382, tv: 0.107705 },
    Vertex3dNoTex2 { x: -0.097329, y: -0.026079, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.476096, tv: 0.112806 },
    Vertex3dNoTex2 { x: -0.100762, y: 0.0, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.462907, tv: 0.114554 },
    Vertex3dNoTex2 { x: -0.101425, y: 0.786319, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.065212, tv: 0.115226 },
    Vertex3dNoTex2 { x: -0.097969, y: 0.812569, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.051934, tv: 0.113489 },
    Vertex3dNoTex2 { x: -0.087837, y: 0.837031, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.039558, tv: 0.108375 },
    Vertex3dNoTex2 { x: -0.071718, y: 0.858037, z: 0.003753, nx: 0.0, ny: 0.0, nz: -1.0, tu: 0.028927, tv: 0.100232 },
    Vertex3dNoTex2 { x: -0.087837, y: 0.837031, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.047295, tv: 0.152337 },
    Vertex3dNoTex2 { x: -0.097969, y: 0.812569, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.059663, tv: 0.147202 },
    Vertex3dNoTex2 { x: -0.101425, y: 0.786319, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.072938, tv: 0.145443 },
    Vertex3dNoTex2 { x: -0.100762, y: 0.0, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.470633, tv: 0.145442 },
    Vertex3dNoTex2 { x: -0.097329, y: -0.026079, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.483825, tv: 0.147167 },
    Vertex3dNoTex2 { x: -0.087263, y: -0.050381, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.496120, tv: 0.152248 },
    Vertex3dNoTex2 { x: -0.071250, y: -0.071250, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.506682, tv: 0.160338 },
    Vertex3dNoTex2 { x: -0.050381, y: -0.087263, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.514789, tv: 0.170886 },
    Vertex3dNoTex2 { x: -0.026079, y: -0.097329, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.519891, tv: 0.183173 },
    Vertex3dNoTex2 { x: 0.0, y: -0.100762, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.521639, tv: 0.196361 },
    Vertex3dNoTex2 { x: 0.026079, y: -0.097329, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.519913, tv: 0.209553 },
    Vertex3dNoTex2 { x: 0.050381, y: -0.087263, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.514832, tv: 0.221848 },
    Vertex3dNoTex2 { x: 0.071250, y: -0.071250, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.506743, tv: 0.232410 },
    Vertex3dNoTex2 { x: 0.087263, y: -0.050381, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.496195, tv: 0.240517 },
    Vertex3dNoTex2 { x: 0.097329, y: -0.026079, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.483908, tv: 0.245619 },
    Vertex3dNoTex2 { x: 0.100762, y: 0.0, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.470719, tv: 0.247367 },
    Vertex3dNoTex2 { x: 0.101425, y: 0.786319, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.073025, tv: 0.248038 },
    Vertex3dNoTex2 { x: 0.097969, y: 0.812569, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.059747, tv: 0.246302 },
    Vertex3dNoTex2 { x: 0.087837, y: 0.837031, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.047370, tv: 0.241188 },
    Vertex3dNoTex2 { x: 0.071718, y: 0.858037, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.036739, tv: 0.233045 },
    Vertex3dNoTex2 { x: 0.050713, y: 0.874155, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.028578, tv: 0.222427 },
    Vertex3dNoTex2 { x: 0.026251, y: 0.884288, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.023443, tv: 0.210059 },
    Vertex3dNoTex2 { x: 0.0, y: 0.887744, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.021684, tv: 0.196784 },
    Vertex3dNoTex2 { x: -0.026251, y: 0.884288, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.023421, tv: 0.183506 },
    Vertex3dNoTex2 { x: -0.050713, y: 0.874155, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.028535, tv: 0.171130 },
    Vertex3dNoTex2 { x: -0.071718, y: 0.858037, z: 1.004253, nx: 0.0, ny: 0.0, nz: 1.0, tu: 0.036678, tv: 0.160499 },
    Vertex3dNoTex2 { x: 0.050381, y: -0.087263, z: 0.003753, nx: 0.500000, ny: -0.866000, nz: 0.0, tu: 0.438656, tv: 0.363047 },
    Vertex3dNoTex2 { x: 0.050381, y: -0.087263, z: 1.004253, nx: 0.500000, ny: -0.866000, nz: 0.0, tu: 0.506767, tv: 0.339752 },
    Vertex3dNoTex2 { x: 0.026079, y: -0.097329, z: 1.004253, nx: 0.258800, ny: -0.965900, nz: 0.0, tu: 0.511832, tv: 0.357037 },
    Vertex3dNoTex2 { x: 0.026079, y: -0.097329, z: 0.003753, nx: 0.258800, ny: -0.965900, nz: 0.0, tu: 0.441094, tv: 0.370485 },
    Vertex3dNoTex2 { x: -0.087837, y: 0.837031, z: 0.003753, nx: -0.866000, ny: 0.500000, nz: 0.0, tu: 0.105813, tv: 0.416117 },
    Vertex3dNoTex2 { x: -0.087837, y: 0.837031, z: 1.004253, nx: -0.866000, ny: 0.500000, nz: 0.0, tu: 0.044005, tv: 0.460868 },
    Vertex3dNoTex2 { x: -0.071718, y: 0.858037, z: 1.004253, nx: -0.707100, ny: 0.707100, nz: 0.0, tu: 0.033382, tv: 0.445712 },
    Vertex3dNoTex2 { x: -0.071718, y: 0.858037, z: 0.003753, nx: -0.707100, ny: 0.707100, nz: 0.0, tu: 0.099801, tv: 0.409952 },
    Vertex3dNoTex2 { x: 0.026251, y: 0.884288, z: 0.003753, nx: 0.258800, ny: 0.965900, nz: 0.0, tu: 0.091964, tv: 0.376411 },
    Vertex3dNoTex2 { x: 0.026251, y: 0.884288, z: 1.004253, nx: 0.258800, ny: 0.965900, nz: 0.0, tu: 0.013389, tv: 0.371796 },
    Vertex3dNoTex2 { x: 0.100762, y: 0.0, z: 1.004253, nx: 0.997800, ny: -0.065800, nz: 0.0, tu: 0.468620, tv: 0.280370 },
    Vertex3dNoTex2 { x: 0.097329, y: -0.026079, z: 1.004253, nx: 0.965900, ny: -0.258800, nz: 0.0, tu: 0.479895, tv: 0.294135 },
    Vertex3dNoTex2 { x: 0.097329, y: -0.026079, z: 0.003753, nx: 0.965900, ny: -0.258800, nz: 0.0, tu: 0.417618, tv: 0.344945 },
    Vertex3dNoTex2 { x: 0.100762, y: 0.0, z: 0.003753, nx: 0.997800, ny: -0.065800, nz: 0.0, tu: 0.407171, tv: 0.339603 },
    Vertex3dNoTex2 { x: -0.050381, y: -0.087263, z: 0.003753, nx: -0.500000, ny: -0.866000, nz: 0.0, tu: 0.437853, tv: 0.393942 },
    Vertex3dNoTex2 { x: -0.050381, y: -0.087263, z: 1.004253, nx: -0.500000, ny: -0.866000, nz: 0.0, tu: 0.510807, tv: 0.410912 },
    Vertex3dNoTex2 { x: -0.071250, y: -0.071250, z: 1.004253, nx: -0.707100, ny: -0.707100, nz: 0.0, tu: 0.505895, tv: 0.428541 },
    Vertex3dNoTex2 { x: -0.071250, y: -0.071250, z: 0.003753, nx: -0.707100, ny: -0.707100, nz: 0.0, tu: 0.433266, tv: 0.401633 },
    Vertex3dNoTex2 { x: 0.0, y: -0.100762, z: 1.004253, nx: 0.0, ny: -1.0, nz: 0.0, tu: 0.513324, tv: 0.374983 },
    Vertex3dNoTex2 { x: 0.0, y: -0.100762, z: 0.003753, nx: 0.0, ny: -1.0, nz: 0.0, tu: 0.441814, tv: 0.378280 },
    Vertex3dNoTex2 { x: 0.087263, y: -0.050381, z: 1.004253, nx: 0.866000, ny: -0.500000, nz: 0.0, tu: 0.490376, tv: 0.308278 },
    Vertex3dNoTex2 { x: 0.087263, y: -0.050381, z: 0.003753, nx: 0.866000, ny: -0.500000, nz: 0.0, tu: 0.427220, tv: 0.349765 },
    Vertex3dNoTex2 { x: 0.0, y: 0.887744, z: 0.003753, nx: 0.0, ny: 1.0, nz: 0.0, tu: 0.091451, tv: 0.385282 },
    Vertex3dNoTex2 { x: 0.0, y: 0.887744, z: 1.004253, nx: 0.0, ny: 1.0, nz: 0.0, tu: 0.014330, tv: 0.391613 },
    Vertex3dNoTex2 { x: 0.101425, y: 0.786319, z: 0.003753, nx: 0.997900, ny: 0.065000, nz: 0.0, tu: 0.138673, tv: 0.343607 },
    Vertex3dNoTex2 { x: 0.101425, y: 0.786319, z: 1.004253, nx: 0.997900, ny: 0.065000, nz: 0.0, tu: 0.049811, tv: 0.277514 },
    Vertex3dNoTex2 { x: 0.097969, y: 0.812569, z: 0.003753, nx: 0.965900, ny: 0.258800, nz: 0.0, tu: 0.115647, tv: 0.347988 },
    Vertex3dNoTex2 { x: 0.097969, y: 0.812569, z: 1.004253, nx: 0.965900, ny: 0.258800, nz: 0.0, tu: 0.037908, tv: 0.294943 },
    Vertex3dNoTex2 { x: -0.026079, y: -0.097329, z: 0.003753, nx: -0.258800, ny: -0.965900, nz: 0.0, tu: 0.440688, tv: 0.386117 },
    Vertex3dNoTex2 { x: -0.026079, y: -0.097329, z: 1.004253, nx: -0.258800, ny: -0.965900, nz: 0.0, tu: 0.513324, tv: 0.392950 },
    Vertex3dNoTex2 { x: -0.087263, y: -0.050381, z: 1.004253, nx: -0.866000, ny: -0.500000, nz: 0.0, tu: 0.498810, tv: 0.445589 },
    Vertex3dNoTex2 { x: -0.087263, y: -0.050381, z: 0.003753, nx: -0.866000, ny: -0.500000, nz: 0.0, tu: 0.426819, tv: 0.409057 },
    Vertex3dNoTex2 { x: 0.071250, y: -0.071250, z: 0.003753, nx: 0.707100, ny: -0.707100, nz: 0.0, tu: 0.434158, tv: 0.356035 },
    Vertex3dNoTex2 { x: 0.071250, y: -0.071250, z: 1.004253, nx: 0.707100, ny: -0.707100, nz: 0.0, tu: 0.499490, tv: 0.323467 },
    Vertex3dNoTex2 { x: -0.097329, y: -0.026079, z: 0.003753, nx: -0.965900, ny: -0.258800, nz: 0.0, tu: 0.418318, tv: 0.416061 },
    Vertex3dNoTex2 { x: -0.097329, y: -0.026079, z: 1.004253, nx: -0.965900, ny: -0.258800, nz: 0.0, tu: 0.489842, tv: 0.461919 },
    Vertex3dNoTex2 { x: -0.100762, y: 0.0, z: 1.004253, nx: -0.997800, ny: -0.065800, nz: 0.0, tu: 0.479319, tv: 0.477522 },
    Vertex3dNoTex2 { x: -0.100762, y: 0.0, z: 0.003753, nx: -0.997800, ny: -0.065800, nz: 0.0, tu: 0.407449, tv: 0.422464 },
    Vertex3dNoTex2 { x: 0.087837, y: 0.837031, z: 0.003753, nx: 0.866000, ny: 0.500000, nz: 0.0, tu: 0.104696, tv: 0.353845 },
    Vertex3dNoTex2 { x: 0.087837, y: 0.837031, z: 1.004253, nx: 0.866000, ny: 0.500000, nz: 0.0, tu: 0.027922, tv: 0.313093 },
];

/// Pre-defined flipper base mesh indices
/// From VPinball src/meshes/flipperBase.h
#[rustfmt::skip]
static FLIPPER_BASE_INDICES: [u16; FLIPPER_BASE_NUM_INDICES] = [
    12, 13, 14,  12, 14, 15,  12, 15, 16,  12, 16, 17,  12, 17, 18,  12, 18, 19,
    12, 19, 20,  12, 20, 21,  12, 21, 22,  12, 22, 23,  12, 23, 24,  12, 24, 25,
    12, 25, 26,  12, 26, 27,  12, 27, 28,  12, 28, 29,  12, 29, 30,  12, 30, 31,
    12, 31, 32,  12, 32, 33,  12, 33, 34,  12, 34, 35,  12, 35, 36,  12, 36, 37,
    0, 1, 2,  101, 1, 0,  0, 2, 3,  101, 100, 1,  3, 2, 69,  98, 100, 101,
    3, 69, 68,  98, 99, 100,  68, 69, 70,  95, 99, 98,  68, 70, 71,  95, 94, 99,
    71, 70, 5,  81, 94, 95,  71, 5, 4,  81, 80, 94,  4, 5, 6,  78, 80, 81,
    4, 6, 7,  78, 79, 80,  7, 6, 87,  92, 79, 78,  7, 87, 86,  92, 93, 79,
    86, 87, 73,  83, 93, 92,  86, 73, 72,  83, 82, 93,  72, 73, 9,  66, 82, 83,
    72, 9, 8,  66, 83, 67,  8, 9, 10,  64, 66, 67,  8, 10, 11,  64, 65, 66,
    11, 10, 103,  96, 65, 64,  11, 103, 102,  96, 97, 65,  102, 103, 91,  85, 97, 96,
    102, 91, 90,  85, 84, 97,  90, 91, 89,  76, 84, 85,  90, 89, 88,  76, 75, 84,
    88, 89, 74,  74, 75, 76,  88, 74, 77,  74, 76, 77,
    38, 39, 40,  38, 40, 41,  38, 41, 42,  38, 42, 43,  38, 43, 44,  38, 44, 45,
    38, 45, 46,  38, 46, 47,  38, 47, 48,  38, 48, 49,  38, 49, 50,  38, 50, 51,
    38, 51, 52,  38, 52, 53,  38, 53, 54,  38, 54, 55,  38, 55, 56,  38, 56, 57,
    38, 57, 58,  38, 58, 59,  38, 59, 60,  38, 60, 61,  38, 61, 62,  38, 62, 63,
];

/// Reference vertices for the flipper TIP bottom (the end that hits the ball)
/// Note: These are at y ≈ 0.786-0.887 (the tip end of the flipper)
/// From VPinball src/parts/flipper.cpp vertsTipBottomf
#[rustfmt::skip]
static VERTS_TIP_BOTTOM: [Vec3; 13] = [
    Vec3 { x: -0.101425, y: 0.786319, z: 0.003753 },
    Vec3 { x: -0.097969, y: 0.812569, z: 0.003753 },
    Vec3 { x: -0.087837, y: 0.837031, z: 0.003753 },
    Vec3 { x: -0.071718, y: 0.858037, z: 0.003753 },
    Vec3 { x: -0.050713, y: 0.874155, z: 0.003753 },
    Vec3 { x: -0.026251, y: 0.884288, z: 0.003753 },
    Vec3 { x: 0.0, y: 0.887744, z: 0.003753 },
    Vec3 { x: 0.026251, y: 0.884288, z: 0.003753 },
    Vec3 { x: 0.050713, y: 0.874155, z: 0.003753 },
    Vec3 { x: 0.071718, y: 0.858037, z: 0.003753 },
    Vec3 { x: 0.087837, y: 0.837031, z: 0.003753 },
    Vec3 { x: 0.097969, y: 0.812569, z: 0.003753 },
    Vec3 { x: 0.101425, y: 0.786319, z: 0.003753 },
];

/// Reference vertices for the flipper TIP top
/// From VPinball src/parts/flipper.cpp vertsTipTopf
#[rustfmt::skip]
static VERTS_TIP_TOP: [Vec3; 13] = [
    Vec3 { x: -0.101425, y: 0.786319, z: 1.004253 },
    Vec3 { x: -0.097969, y: 0.812569, z: 1.004253 },
    Vec3 { x: -0.087837, y: 0.837031, z: 1.004253 },
    Vec3 { x: -0.071718, y: 0.858037, z: 1.004253 },
    Vec3 { x: -0.050713, y: 0.874155, z: 1.004253 },
    Vec3 { x: -0.026251, y: 0.884288, z: 1.004253 },
    Vec3 { x: 0.0, y: 0.887744, z: 1.004253 },
    Vec3 { x: 0.026251, y: 0.884288, z: 1.004253 },
    Vec3 { x: 0.050713, y: 0.874155, z: 1.004253 },
    Vec3 { x: 0.071718, y: 0.858037, z: 1.004253 },
    Vec3 { x: 0.087837, y: 0.837031, z: 1.004253 },
    Vec3 { x: 0.097969, y: 0.812569, z: 1.004253 },
    Vec3 { x: 0.101425, y: 0.786319, z: 1.004253 },
];

/// Reference vertices for the flipper BASE bottom (the pivot end)
/// Note: These are at y ≈ -0.1 to 0 (the base/pivot of the flipper)
/// From VPinball src/parts/flipper.cpp vertsBaseBottomf
#[rustfmt::skip]
static VERTS_BASE_BOTTOM: [Vec3; 13] = [
    Vec3 { x: -0.100762, y: 0.0, z: 0.003753 },
    Vec3 { x: -0.097329, y: -0.026079, z: 0.003753 },
    Vec3 { x: -0.087263, y: -0.050381, z: 0.003753 },
    Vec3 { x: -0.071250, y: -0.071250, z: 0.003753 },
    Vec3 { x: -0.050381, y: -0.087263, z: 0.003753 },
    Vec3 { x: -0.026079, y: -0.097329, z: 0.003753 },
    Vec3 { x: 0.0, y: -0.100762, z: 0.003753 },
    Vec3 { x: 0.026079, y: -0.097329, z: 0.003753 },
    Vec3 { x: 0.050381, y: -0.087263, z: 0.003753 },
    Vec3 { x: 0.071250, y: -0.071250, z: 0.003753 },
    Vec3 { x: 0.087263, y: -0.050381, z: 0.003753 },
    Vec3 { x: 0.097329, y: -0.026079, z: 0.003753 },
    Vec3 { x: 0.100762, y: 0.0, z: 0.003753 },
];

/// Reference vertices for the flipper BASE top
/// From VPinball src/parts/flipper.cpp vertsBaseTopf
#[rustfmt::skip]
static VERTS_BASE_TOP: [Vec3; 13] = [
    Vec3 { x: -0.100762, y: 0.0, z: 1.004253 },
    Vec3 { x: -0.097329, y: -0.026079, z: 1.004253 },
    Vec3 { x: -0.087263, y: -0.050381, z: 1.004253 },
    Vec3 { x: -0.071250, y: -0.071250, z: 1.004253 },
    Vec3 { x: -0.050381, y: -0.087263, z: 1.004253 },
    Vec3 { x: -0.026079, y: -0.097329, z: 1.004253 },
    Vec3 { x: 0.0, y: -0.100762, z: 1.004253 },
    Vec3 { x: 0.026079, y: -0.097329, z: 1.004253 },
    Vec3 { x: 0.050381, y: -0.087263, z: 1.004253 },
    Vec3 { x: 0.071250, y: -0.071250, z: 1.004253 },
    Vec3 { x: 0.087263, y: -0.050381, z: 1.004253 },
    Vec3 { x: 0.097329, y: -0.026079, z: 1.004253 },
    Vec3 { x: 0.100762, y: 0.0, z: 1.004253 },
];

/// Degrees to radians conversion
fn deg_to_rad(deg: f32) -> f32 {
    deg * PI / 180.0
}

/// Sign function matching VPinball's sgn()
fn sgn(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Apply fix for flipper vertex scaling
/// Ported from VPinball flipper.cpp ApplyFix()
///
/// This function adjusts vertex positions and normals to scale the flipper
/// base and tip to the desired radii.
fn apply_fix(
    vert: &mut Vertex3dNoTex2,
    center: Vec2,
    mid_angle: f32,
    radius: f32,
    new_center: Vec2,
    fix_angle_scale: f32,
) {
    let mut v_angle = (vert.y - center.y).atan2(vert.x - center.x);
    let mut n_angle = vert.ny.atan2(vert.nx);

    // We want to have angles with same sign as mid_angle, fix it:
    if mid_angle < 0.0 {
        if v_angle > 0.0 {
            v_angle -= PI * 2.0;
        }
        if n_angle > 0.0 {
            n_angle -= PI * 2.0;
        }
    } else {
        if v_angle < 0.0 {
            v_angle += PI * 2.0;
        }
        if n_angle < 0.0 {
            n_angle += PI * 2.0;
        }
    }

    let sgn_mid = sgn(mid_angle);
    n_angle -= (v_angle - mid_angle) * fix_angle_scale * sgn_mid;
    v_angle -= (v_angle - mid_angle) * fix_angle_scale * sgn_mid;

    let n_length = (vert.nx * vert.nx + vert.ny * vert.ny).sqrt();

    vert.x = v_angle.cos() * radius + new_center.x;
    vert.y = v_angle.sin() * radius + new_center.y;
    vert.nx = n_angle.cos() * n_length;
    vert.ny = n_angle.sin() * n_length;
}

/// Check if vertex matches a reference vertex (with floating point tolerance)
fn vertex_matches(v: &Vertex3dNoTex2, r: &Vec3) -> bool {
    const EPSILON: f32 = 0.0001;
    (v.x - r.x).abs() < EPSILON && (v.y - r.y).abs() < EPSILON && (v.z - r.z).abs() < EPSILON
}

pub(super) fn write_flipper_meshes(
    gameitems_dir: &Path,
    flipper: &Flipper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_flipper_mesh(flipper, 0.0) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &flipper.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}

/// Build flipper mesh geometry
///
/// # Arguments
/// * `flipper` - The flipper data
/// * `surface_height` - The height of the surface the flipper is on (typically 0.0)
///
/// # Returns
/// Tuple of (vertices, indices) for the flipper mesh, or None if flipper is not visible
pub fn build_flipper_mesh(
    flipper: &Flipper,
    surface_height: f32,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if !flipper.is_visible {
        return None;
    }

    let rubber_thickness = flipper
        .rubber_thickness
        .unwrap_or(flipper.rubber_thickness_int as f32);
    let rubber_height = flipper
        .rubber_height
        .unwrap_or(flipper.rubber_height_int as f32);
    let rubber_width = flipper
        .rubber_width
        .unwrap_or(flipper.rubber_width_int as f32);

    // Calculate angle needed to fix P0 location
    let sin_angle =
        ((flipper.base_radius - flipper.end_radius) / flipper.flipper_radius_max).clamp(-1.0, 1.0);
    let fix_angle = sin_angle.asin();
    let fix_angle_scale = fix_angle / (PI * 0.5);

    let base_radius = flipper.base_radius - rubber_thickness;
    let end_radius = flipper.end_radius - rubber_thickness;

    // Generate base flipper mesh
    let mut temp: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

    // Scale the base and tip vertices
    for t in 0..13 {
        for vert in temp.iter_mut() {
            if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
        }
    }

    // Apply rotation (180 degrees) and transformations
    let rotation_matrix = Matrix3D::rotate_z(deg_to_rad(180.0));
    let start_angle_rad = deg_to_rad(flipper.start_angle);

    let mut vertices = Vec::with_capacity(FLIPPER_BASE_VERTICES * 2);

    for (i, temp_vert) in temp.iter().enumerate() {
        let rotated = rotation_matrix.multiply_vector(Vec3 {
            x: temp_vert.x,
            y: temp_vert.y,
            z: temp_vert.z,
        });

        let mut vert = Vertex3dNoTex2 {
            x: rotated.x,
            y: rotated.y,
            z: rotated.z * flipper.height + surface_height,
            nx: FLIPPER_BASE_MESH[i].nx,
            ny: FLIPPER_BASE_MESH[i].ny,
            nz: FLIPPER_BASE_MESH[i].nz,
            tu: FLIPPER_BASE_MESH[i].tu,
            tv: FLIPPER_BASE_MESH[i].tv,
        };

        // Apply normal rotation
        let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
            x: vert.nx,
            y: vert.ny,
            z: vert.nz,
        });
        vert.nx = rotated_normal.x;
        vert.ny = rotated_normal.y;
        vert.nz = rotated_normal.z;

        // Apply start angle rotation and translate to flipper center
        let (sin_a, cos_a) = start_angle_rad.sin_cos();
        let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
        let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

        // Rotate normal as well
        let final_nx = vert.nx * cos_a - vert.ny * sin_a;
        let final_ny = vert.nx * sin_a + vert.ny * cos_a;

        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: final_x,
                y: final_y,
                z: vert.z,
                nx: final_nx,
                ny: final_ny,
                nz: vert.nz,
                tu: vert.tu,
                tv: vert.tv,
            },
        ));
    }

    // Generate rubber mesh if rubber_thickness > 0
    if rubber_thickness > 0.0 {
        let mut temp_rubber: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

        // Scale for rubber (with thickness added back)
        for t in 0..13 {
            for vert in temp_rubber.iter_mut() {
                if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
            }
        }

        for (i, temp_vert) in temp_rubber.iter().enumerate() {
            let rotated = rotation_matrix.multiply_vector(Vec3 {
                x: temp_vert.x,
                y: temp_vert.y,
                z: temp_vert.z,
            });

            let mut vert = Vertex3dNoTex2 {
                x: rotated.x,
                y: rotated.y,
                z: rotated.z * rubber_width + (surface_height + rubber_height),
                nx: FLIPPER_BASE_MESH[i].nx,
                ny: FLIPPER_BASE_MESH[i].ny,
                nz: FLIPPER_BASE_MESH[i].nz,
                tu: FLIPPER_BASE_MESH[i].tu,
                tv: FLIPPER_BASE_MESH[i].tv + 0.5,
            };

            // Apply normal rotation
            let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
                x: vert.nx,
                y: vert.ny,
                z: vert.nz,
            });
            vert.nx = rotated_normal.x;
            vert.ny = rotated_normal.y;
            vert.nz = rotated_normal.z;

            // Apply start angle rotation and translate to flipper center
            let (sin_a, cos_a) = start_angle_rad.sin_cos();
            let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
            let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

            // Rotate normal as well
            let final_nx = vert.nx * cos_a - vert.ny * sin_a;
            let final_ny = vert.nx * sin_a + vert.ny * cos_a;

            vertices.push(VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: final_x,
                    y: final_y,
                    z: vert.z,
                    nx: final_nx,
                    ny: final_ny,
                    nz: vert.nz,
                    tu: vert.tu,
                    tv: vert.tv,
                },
            ));
        }
    }

    // Build indices
    let mut indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES * 2 / 3);

    // Base mesh indices
    for chunk in FLIPPER_BASE_INDICES.chunks(3) {
        indices.push(VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        });
    }

    // Rubber mesh indices (offset by base vertex count)
    if rubber_thickness > 0.0 {
        for chunk in FLIPPER_BASE_INDICES.chunks(3) {
            indices.push(VpxFace {
                i0: (chunk[0] as i64) + FLIPPER_BASE_VERTICES as i64,
                i1: (chunk[1] as i64) + FLIPPER_BASE_VERTICES as i64,
                i2: (chunk[2] as i64) + FLIPPER_BASE_VERTICES as i64,
            });
        }
    }

    Some((vertices, indices))
}

/// Build flipper mesh geometry with separate base and rubber meshes
///
/// This is the preferred function for GLB export as it allows assigning
/// different materials to the base flipper and rubber.
///
/// # Arguments
/// * `flipper` - The flipper data
/// * `surface_height` - The height of the surface the flipper is on (typically 0.0)
///
/// # Returns
/// FlipperMeshes with separate base and rubber meshes, or None if flipper is not visible
pub fn build_flipper_meshes(flipper: &Flipper, surface_height: f32) -> Option<FlipperMeshes> {
    if !flipper.is_visible {
        return None;
    }

    let rubber_thickness = flipper
        .rubber_thickness
        .unwrap_or(flipper.rubber_thickness_int as f32);
    let rubber_height = flipper
        .rubber_height
        .unwrap_or(flipper.rubber_height_int as f32);
    let rubber_width = flipper
        .rubber_width
        .unwrap_or(flipper.rubber_width_int as f32);

    // Calculate angle needed to fix P0 location
    let sin_angle =
        ((flipper.base_radius - flipper.end_radius) / flipper.flipper_radius_max).clamp(-1.0, 1.0);
    let fix_angle = sin_angle.asin();
    let fix_angle_scale = fix_angle / (PI * 0.5);

    let base_radius = flipper.base_radius - rubber_thickness;
    let end_radius = flipper.end_radius - rubber_thickness;

    // Generate base flipper mesh
    let mut temp: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

    // Scale the base and tip vertices
    for t in 0..13 {
        for vert in temp.iter_mut() {
            if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
        }
    }

    // Apply rotation (180 degrees) and transformations
    let rotation_matrix = Matrix3D::rotate_z(deg_to_rad(180.0));
    let start_angle_rad = deg_to_rad(flipper.start_angle);

    // Build base mesh vertices
    let mut base_vertices = Vec::with_capacity(FLIPPER_BASE_VERTICES);
    for (i, temp_vert) in temp.iter().enumerate() {
        let rotated = rotation_matrix.multiply_vector(Vec3 {
            x: temp_vert.x,
            y: temp_vert.y,
            z: temp_vert.z,
        });

        let mut vert = Vertex3dNoTex2 {
            x: rotated.x,
            y: rotated.y,
            z: rotated.z * flipper.height + surface_height,
            nx: FLIPPER_BASE_MESH[i].nx,
            ny: FLIPPER_BASE_MESH[i].ny,
            nz: FLIPPER_BASE_MESH[i].nz,
            tu: FLIPPER_BASE_MESH[i].tu,
            tv: FLIPPER_BASE_MESH[i].tv,
        };

        // Apply normal rotation
        let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
            x: vert.nx,
            y: vert.ny,
            z: vert.nz,
        });
        vert.nx = rotated_normal.x;
        vert.ny = rotated_normal.y;
        vert.nz = rotated_normal.z;

        // Apply start angle rotation and translate to flipper center
        let (sin_a, cos_a) = start_angle_rad.sin_cos();
        let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
        let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

        // Rotate normal as well
        let final_nx = vert.nx * cos_a - vert.ny * sin_a;
        let final_ny = vert.nx * sin_a + vert.ny * cos_a;

        base_vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: final_x,
                y: final_y,
                z: vert.z,
                nx: final_nx,
                ny: final_ny,
                nz: vert.nz,
                tu: vert.tu,
                tv: vert.tv,
            },
        ));
    }

    // Build base mesh indices
    let mut base_indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES / 3);
    for chunk in FLIPPER_BASE_INDICES.chunks(3) {
        base_indices.push(VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        });
    }

    // Generate rubber mesh if rubber_thickness > 0
    let rubber = if rubber_thickness > 0.0 {
        let mut temp_rubber: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

        // Scale for rubber (with thickness added back)
        for t in 0..13 {
            for vert in temp_rubber.iter_mut() {
                if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
            }
        }

        let mut rubber_vertices = Vec::with_capacity(FLIPPER_BASE_VERTICES);
        for (i, temp_vert) in temp_rubber.iter().enumerate() {
            let rotated = rotation_matrix.multiply_vector(Vec3 {
                x: temp_vert.x,
                y: temp_vert.y,
                z: temp_vert.z,
            });

            let mut vert = Vertex3dNoTex2 {
                x: rotated.x,
                y: rotated.y,
                z: rotated.z * rubber_width + (surface_height + rubber_height),
                nx: FLIPPER_BASE_MESH[i].nx,
                ny: FLIPPER_BASE_MESH[i].ny,
                nz: FLIPPER_BASE_MESH[i].nz,
                tu: FLIPPER_BASE_MESH[i].tu,
                tv: FLIPPER_BASE_MESH[i].tv + 0.5,
            };

            // Apply normal rotation
            let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
                x: vert.nx,
                y: vert.ny,
                z: vert.nz,
            });
            vert.nx = rotated_normal.x;
            vert.ny = rotated_normal.y;
            vert.nz = rotated_normal.z;

            // Apply start angle rotation and translate to flipper center
            let (sin_a, cos_a) = start_angle_rad.sin_cos();
            let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
            let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

            // Rotate normal as well
            let final_nx = vert.nx * cos_a - vert.ny * sin_a;
            let final_ny = vert.nx * sin_a + vert.ny * cos_a;

            rubber_vertices.push(VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: final_x,
                    y: final_y,
                    z: vert.z,
                    nx: final_nx,
                    ny: final_ny,
                    nz: vert.nz,
                    tu: vert.tu,
                    tv: vert.tv,
                },
            ));
        }

        // Build rubber mesh indices (same as base, no offset needed since separate mesh)
        let mut rubber_indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES / 3);
        for chunk in FLIPPER_BASE_INDICES.chunks(3) {
            rubber_indices.push(VpxFace {
                i0: chunk[0] as i64,
                i1: chunk[1] as i64,
                i2: chunk[2] as i64,
            });
        }

        Some((rubber_vertices, rubber_indices))
    } else {
        None
    };

    Some(FlipperMeshes {
        base: (base_vertices, base_indices),
        rubber,
    })
}
