//! Kicker mesh generation for expanded VPX export
//!
//! This module ports the kicker mesh generation from Visual Pinball's kicker.cpp.
//! Kickers use pre-defined meshes that are scaled and rotated based on the kicker's
//! parameters (radius, orientation, type, etc.).
//!
//! A kicker consists of 2 parts:
//! - Plate: A flat circular plate at the base (same for all kicker types)
//! - Kicker: The main kicker body (different mesh per kicker type)
//!
//! Kicker types:
//! - Invisible: No mesh generated
//! - Cup: Standard cup kicker
//! - Cup2 (T1): Alternative cup design
//! - Hole: Hole kicker with wood texture
//! - HoleSimple: Simplified hole kicker
//! - Williams: Williams-style kicker
//! - Gottlieb: Gottlieb-style kicker
//!
//! Ported from: VPinball/src/parts/kicker.cpp

mod kicker_cup_mesh;
mod kicker_gottlieb_mesh;
mod kicker_hole_mesh;
mod kicker_plate_mesh;
mod kicker_simple_hole_mesh;
mod kicker_t1_mesh;
mod kicker_williams_mesh;

use crate::vpx::gameitem::kicker::{Kicker, KickerType};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::math::Matrix3D;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;

pub use kicker_cup_mesh::*;
pub use kicker_gottlieb_mesh::*;
pub use kicker_hole_mesh::*;
pub use kicker_plate_mesh::*;
pub use kicker_simple_hole_mesh::*;
pub use kicker_t1_mesh::*;
pub use kicker_williams_mesh::*;

/// Result of kicker mesh generation with separate meshes for plate and kicker body
pub struct KickerMeshes {
    /// The plate mesh (flat circular base)
    pub plate: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The kicker body mesh (varies by kicker type)
    pub kicker: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// Degrees to radians conversion
fn deg_to_rad(deg: f32) -> f32 {
    deg * PI / 180.0
}

/// Generate all kicker meshes based on the kicker parameters
///
/// # Arguments
/// * `kicker` - The kicker definition
/// * `base_height` - The height of the surface the kicker sits on (from table surface lookup)
///
/// # Returns
/// A KickerMeshes struct containing plate and kicker body meshes
pub fn build_kicker_meshes(kicker: &Kicker, base_height: f32) -> KickerMeshes {
    // Invisible kickers have no mesh
    if matches!(kicker.kicker_type, KickerType::Invisible) {
        return KickerMeshes {
            plate: None,
            kicker: None,
        };
    }

    KickerMeshes {
        plate: Some(generate_plate_mesh(kicker, base_height)),
        kicker: Some(generate_kicker_mesh(kicker, base_height)),
    }
}

/// Generate the plate mesh for a kicker
///
/// The plate is a flat circular base that's the same for all kicker types,
/// but scaled differently based on kicker type.
///
/// Ported from VPinball kicker.cpp RenderSetup() plate section
fn generate_plate_mesh(kicker: &Kicker, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    // Calculate plate radius based on kicker type
    // From kicker.cpp lines 211-218
    let rad = match kicker.kicker_type {
        KickerType::Williams | KickerType::Gottlieb => kicker.radius * 0.88,
        KickerType::Cup2 => kicker.radius * 0.87,
        KickerType::Cup => kicker.radius, // Cup uses full radius
        _ => kicker.radius * 0.82,        // Hole, HoleSimple, etc.
    };

    let num_vertices = KICKER_PLATE_NUM_VERTICES;
    let num_indices = KICKER_PLATE_NUM_INDICES;

    let mut vertices = Vec::with_capacity(num_vertices);

    // Transform vertices
    // From kicker.cpp lines 219-229
    for src in &KICKER_PLATE_VERTICES {
        let x = src.x * rad + kicker.center.x;
        let y = src.y * rad + kicker.center.y;
        let z = src.z * rad + base_height;

        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x,
                y,
                z,
                nx: src.nx,
                ny: src.ny,
                nz: src.nz,
                tu: 0.0, // Plate doesn't use texture coordinates
                tv: 0.0,
            },
        ));
    }

    // Convert indices to faces
    let mut faces = Vec::with_capacity(num_indices / 3);
    for i in (0..num_indices).step_by(3) {
        faces.push(VpxFace {
            i0: KICKER_PLATE_INDICES[i] as i64,
            i1: KICKER_PLATE_INDICES[i + 1] as i64,
            i2: KICKER_PLATE_INDICES[i + 2] as i64,
        });
    }

    (vertices, faces)
}

/// Generate the kicker body mesh
///
/// The mesh used depends on the kicker type. Each mesh is scaled by radius
/// and rotated by orientation.
///
/// Ported from VPinball kicker.cpp GenerateMesh()
fn generate_kicker_mesh(kicker: &Kicker, base_height: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    // Get mesh data and parameters based on kicker type
    // From kicker.cpp GenerateMesh() lines 470-526
    let (mesh_vertices, mesh_indices, z_offset, z_rot) = match kicker.kicker_type {
        KickerType::Cup => (
            &KICKER_CUP_VERTICES[..],
            &KICKER_CUP_INDICES[..],
            -0.18_f32,
            kicker.orientation,
        ),
        KickerType::Williams => (
            &KICKER_WILLIAMS_VERTICES[..],
            &KICKER_WILLIAMS_INDICES[..],
            0.0_f32,
            kicker.orientation + 90.0,
        ),
        KickerType::Gottlieb => (
            &KICKER_GOTTLIEB_VERTICES[..],
            &KICKER_GOTTLIEB_INDICES[..],
            0.0_f32,
            kicker.orientation,
        ),
        KickerType::Cup2 => (
            &KICKER_T1_VERTICES[..],
            &KICKER_T1_INDICES[..],
            0.0_f32,
            kicker.orientation,
        ),
        KickerType::Hole => (
            &KICKER_HOLE_VERTICES[..],
            &KICKER_HOLE_INDICES[..],
            0.0_f32,
            0.0_f32, // Hole type ignores orientation
        ),
        KickerType::HoleSimple | KickerType::Invisible => (
            &KICKER_SIMPLE_HOLE_VERTICES[..],
            &KICKER_SIMPLE_HOLE_INDICES[..],
            0.0_f32,
            0.0_f32, // HoleSimple type ignores orientation
        ),
    };

    let num_vertices = mesh_vertices.len();
    let num_indices = mesh_indices.len();

    // Build rotation matrix
    let full_matrix = Matrix3D::rotate_z(deg_to_rad(z_rot));

    let mut vertices = Vec::with_capacity(num_vertices);

    // Transform vertices
    // From kicker.cpp GenerateMesh() lines 528-541
    for src in mesh_vertices {
        // Apply z offset and rotate
        let vert = full_matrix.transform_vertex(crate::vpx::math::Vertex3D::new(
            src.x,
            src.y,
            src.z + z_offset,
        ));

        // Scale by radius and translate to position
        let x = vert.x * kicker.radius + kicker.center.x;
        let y = vert.y * kicker.radius + kicker.center.y;
        let z = vert.z * kicker.radius + base_height;

        // Rotate normals (no translation)
        let normal = full_matrix.transform_normal(src.nx, src.ny, src.nz);

        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x,
                y,
                z,
                nx: normal.x,
                ny: normal.y,
                nz: normal.z,
                tu: src.tu,
                tv: src.tv,
            },
        ));
    }

    // Convert indices to faces
    let mut faces = Vec::with_capacity(num_indices / 3);
    for i in (0..num_indices).step_by(3) {
        faces.push(VpxFace {
            i0: mesh_indices[i] as i64,
            i1: mesh_indices[i + 1] as i64,
            i2: mesh_indices[i + 2] as i64,
        });
    }

    (vertices, faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    fn create_test_kicker(kicker_type: KickerType) -> Kicker {
        let mut kicker = Kicker::default();
        kicker.center = Vertex2D { x: 100.0, y: 200.0 };
        kicker.radius = 25.0;
        kicker.orientation = 0.0;
        kicker.kicker_type = kicker_type;
        kicker.name = "TestKicker".to_string();
        kicker
    }

    #[test]
    fn test_invisible_kicker_has_no_mesh() {
        let kicker = create_test_kicker(KickerType::Invisible);
        let meshes = build_kicker_meshes(&kicker, 0.0);
        assert!(meshes.plate.is_none());
        assert!(meshes.kicker.is_none());
    }

    #[test]
    fn test_cup_kicker_has_meshes() {
        let kicker = create_test_kicker(KickerType::Cup);
        let meshes = build_kicker_meshes(&kicker, 0.0);
        assert!(meshes.plate.is_some());
        assert!(meshes.kicker.is_some());

        let (plate_verts, plate_faces) = meshes.plate.unwrap();
        assert_eq!(plate_verts.len(), KICKER_PLATE_NUM_VERTICES);
        assert_eq!(plate_faces.len(), KICKER_PLATE_NUM_INDICES / 3);

        let (kicker_verts, kicker_faces) = meshes.kicker.unwrap();
        assert_eq!(kicker_verts.len(), KICKER_CUP_NUM_VERTICES);
        assert_eq!(kicker_faces.len(), KICKER_CUP_NUM_INDICES / 3);
    }

    #[test]
    fn test_williams_kicker_rotation() {
        let kicker = create_test_kicker(KickerType::Williams);
        let meshes = build_kicker_meshes(&kicker, 0.0);
        assert!(meshes.kicker.is_some());

        let (kicker_verts, _) = meshes.kicker.unwrap();
        assert_eq!(kicker_verts.len(), KICKER_WILLIAMS_NUM_VERTICES);
    }

    #[test]
    fn test_all_kicker_types_generate_meshes() {
        for kicker_type in [
            KickerType::Cup,
            KickerType::Cup2,
            KickerType::Hole,
            KickerType::HoleSimple,
            KickerType::Williams,
            KickerType::Gottlieb,
        ] {
            let kicker = create_test_kicker(kicker_type.clone());
            let meshes = build_kicker_meshes(&kicker, 0.0);
            assert!(
                meshes.plate.is_some(),
                "Plate should exist for {:?}",
                kicker_type
            );
            assert!(
                meshes.kicker.is_some(),
                "Kicker should exist for {:?}",
                kicker_type
            );
        }
    }
}
