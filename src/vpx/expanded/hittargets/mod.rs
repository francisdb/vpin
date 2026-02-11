//! Hit target mesh generation for expanded VPX export
//!
//! This module ports the hit target mesh generation from Visual Pinball's hittarget.cpp.
//! Hit targets use pre-defined meshes that are scaled and transformed based on
//! the target's parameters (position, size, rotation).
//!
//! There are 9 target types:
//! - DropTargetBeveled (T2)
//! - DropTargetSimple (T3)
//! - DropTargetFlatSimple (T4)
//! - HitTargetRound
//! - HitTargetRectangle
//! - HitFatTargetRectangle
//! - HitFatTargetSquare
//! - HitTargetSlim (T1)
//! - HitFatTargetSlim (T2 slim)
//!
//! Ported from: VPinball/src/parts/hittarget.cpp

mod drop_target_t2_mesh;
mod drop_target_t3_mesh;
mod drop_target_t4_mesh;
mod hit_target_fat_rectangle_mesh;
mod hit_target_fat_square_mesh;
mod hit_target_rectangle_mesh;
mod hit_target_round_mesh;
mod hit_target_t1_slim_mesh;
mod hit_target_t2_slim_mesh;

use super::mesh_common::{Matrix3D, Vec3, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::hittarget::{HitTarget, TargetType};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

use drop_target_t2_mesh::{DROP_TARGET_T2_INDICES, DROP_TARGET_T2_MESH};
use drop_target_t3_mesh::{DROP_TARGET_T3_INDICES, DROP_TARGET_T3_MESH};
use drop_target_t4_mesh::{DROP_TARGET_T4_INDICES, DROP_TARGET_T4_MESH};
use hit_target_fat_rectangle_mesh::{
    HIT_TARGET_FAT_RECTANGLE_INDICES, HIT_TARGET_FAT_RECTANGLE_MESH,
};
use hit_target_fat_square_mesh::{HIT_TARGET_FAT_SQUARE_INDICES, HIT_TARGET_FAT_SQUARE_MESH};
use hit_target_rectangle_mesh::{HIT_TARGET_RECTANGLE_INDICES, HIT_TARGET_RECTANGLE_MESH};
use hit_target_round_mesh::{HIT_TARGET_ROUND_INDICES, HIT_TARGET_ROUND_MESH};
use hit_target_t1_slim_mesh::{HIT_TARGET_T1_SLIM_INDICES, HIT_TARGET_T1_SLIM_MESH};
use hit_target_t2_slim_mesh::{HIT_TARGET_T2_SLIM_INDICES, HIT_TARGET_T2_SLIM_MESH};

/// Degrees to radians conversion
fn deg_to_rad(deg: f32) -> f32 {
    deg * PI / 180.0
}

/// Write hit target mesh to a file
pub(super) fn write_hit_target_meshes(
    gameitems_dir: &Path,
    hit_target: &HitTarget,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_hit_target_mesh(hit_target) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &hit_target.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}

/// Get the mesh data for a target type
fn get_mesh_for_type(target_type: &TargetType) -> (&'static [Vertex3dNoTex2], &'static [u16]) {
    match target_type {
        TargetType::DropTargetBeveled => (&DROP_TARGET_T2_MESH, &DROP_TARGET_T2_INDICES),
        TargetType::DropTargetSimple => (&DROP_TARGET_T3_MESH, &DROP_TARGET_T3_INDICES),
        TargetType::DropTargetFlatSimple => (&DROP_TARGET_T4_MESH, &DROP_TARGET_T4_INDICES),
        TargetType::HitTargetRound => (&HIT_TARGET_ROUND_MESH, &HIT_TARGET_ROUND_INDICES),
        TargetType::HitTargetRectangle => {
            (&HIT_TARGET_RECTANGLE_MESH, &HIT_TARGET_RECTANGLE_INDICES)
        }
        TargetType::HitFatTargetRectangle => (
            &HIT_TARGET_FAT_RECTANGLE_MESH,
            &HIT_TARGET_FAT_RECTANGLE_INDICES,
        ),
        TargetType::HitFatTargetSquare => {
            (&HIT_TARGET_FAT_SQUARE_MESH, &HIT_TARGET_FAT_SQUARE_INDICES)
        }
        TargetType::HitTargetSlim => (&HIT_TARGET_T1_SLIM_MESH, &HIT_TARGET_T1_SLIM_INDICES),
        TargetType::HitFatTargetSlim => (&HIT_TARGET_T2_SLIM_MESH, &HIT_TARGET_T2_SLIM_INDICES),
    }
}

/// Generate hit target mesh based on the target parameters
///
/// From VPinball HitTarget::GenerateMesh
///
/// The transformation is:
/// 1. Scale vertex by size (x, y, z)
/// 2. Rotate by rot_z around Z axis
/// 3. Translate to position
///
/// # Arguments
/// * `target` - The hit target definition
///
/// # Returns
/// A tuple of (vertices, faces) for the target mesh, or None if not visible
pub fn build_hit_target_mesh(target: &HitTarget) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if !target.is_visible {
        return None;
    }

    let (mesh, indices) = get_mesh_for_type(&target.target_type);
    let full_matrix = Matrix3D::rotate_z(deg_to_rad(target.rot_z));

    let vertices: Vec<VertexWrapper> = mesh
        .iter()
        .map(|v| {
            // Scale by size
            let mut vert = Vec3 {
                x: v.x * target.size.x,
                y: v.y * target.size.y,
                z: v.z * target.size.z,
            };

            // Rotate by rot_z
            vert = full_matrix.multiply_vector(vert);

            // Translate to position
            let x = vert.x + target.position.x;
            let y = vert.y + target.position.y;
            let z = vert.z + target.position.z;

            // Transform normal (rotation only, no translation)
            let norm = full_matrix.multiply_vector_no_translate(Vec3 {
                x: v.nx,
                y: v.ny,
                z: v.nz,
            });

            VertexWrapper {
                vpx_encoded_vertex: [0u8; 32], // Not used for generated meshes
                vertex: Vertex3dNoTex2 {
                    x,
                    y,
                    z,
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            }
        })
        .collect();

    // Convert indices to faces (triangles)
    let faces: Vec<VpxFace> = indices
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    Some((vertices, faces))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex3d::Vertex3D;

    fn make_test_target(target_type: TargetType, is_visible: bool) -> HitTarget {
        let mut target = HitTarget::default();
        target.position = Vertex3D::new(100.0, 200.0, 0.0);
        target.size = Vertex3D::new(32.0, 32.0, 32.0);
        target.rot_z = 0.0;
        target.target_type = target_type;
        target.is_visible = is_visible;
        target
    }

    #[test]
    fn test_build_hit_target_mesh_drop_target_beveled() {
        let target = make_test_target(TargetType::DropTargetBeveled, true);

        let result = build_hit_target_mesh(&target);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!faces.is_empty());

        // Check that vertices are transformed to position
        for v in &vertices {
            // All vertices should be roughly around the position
            // (within the scaled mesh bounds)
            assert!(v.vertex.x > 50.0 && v.vertex.x < 150.0);
            assert!(v.vertex.y > 150.0 && v.vertex.y < 250.0);
        }
    }

    #[test]
    fn test_build_hit_target_mesh_invisible() {
        let target = make_test_target(TargetType::DropTargetBeveled, false);

        let result = build_hit_target_mesh(&target);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_hit_target_mesh_all_types() {
        let types = [
            TargetType::DropTargetBeveled,
            TargetType::DropTargetSimple,
            TargetType::DropTargetFlatSimple,
            TargetType::HitTargetRound,
            TargetType::HitTargetRectangle,
            TargetType::HitFatTargetRectangle,
            TargetType::HitFatTargetSquare,
            TargetType::HitTargetSlim,
            TargetType::HitFatTargetSlim,
        ];

        for target_type in types {
            let target = make_test_target(target_type.clone(), true);

            let result = build_hit_target_mesh(&target);
            assert!(
                result.is_some(),
                "Failed to generate mesh for {:?}",
                target_type
            );
        }
    }
}
