//! Spinner mesh generation for expanded VPX export
//!
//! This module ports the spinner mesh generation from Visual Pinball's spinner.cpp.
//! Spinners use pre-defined base meshes that are scaled and transformed based on
//! the spinner's parameters (length, height, rotation, etc.).
//!
//! A spinner consists of 2 parts:
//! - Bracket: The fixed mounting bracket (optional, controlled by `show_bracket`)
//! - Plate: The rotating spinner plate
//!
//! Ported from: VPinball/src/parts/spinner.cpp

mod spinner_bracket_mesh;
mod spinner_plate_mesh;

use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::spinner::Spinner;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

use crate::vpx::math::{Mat3, Vec3};
pub use spinner_bracket_mesh::*;
pub use spinner_plate_mesh::*;

/// Result of spinner mesh generation with separate meshes for each part
///
/// Vertices are centered at origin.
pub struct SpinnerMeshes {
    /// The bracket mesh (uses a default metal material)
    /// Only present if `spinner.show_bracket` is true
    pub bracket: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The plate mesh (uses spinner.material and spinner.image)
    pub plate: (Vec<VertexWrapper>, Vec<VpxFace>),
}

/// Generate all spinner meshes based on the spinner parameters
///
/// # Arguments
/// * `spinner` - The spinner definition
/// * `base_height` - The height of the surface the spinner sits on (from table surface lookup)
///
/// # Returns
/// A SpinnerMeshes struct containing all spinner parts
pub fn build_spinner_meshes(spinner: &Spinner) -> SpinnerMeshes {
    let full_matrix = Mat3::rotate_z(spinner.rotation.to_radians());

    SpinnerMeshes {
        bracket: if spinner.show_bracket {
            Some(generate_bracket_mesh(spinner, &full_matrix))
        } else {
            None
        },
        plate: generate_plate_mesh(spinner, &full_matrix),
    }
}

/// Generate the bracket mesh
/// From VPinball Spinner::ExportMesh (bracket section)
fn generate_bracket_mesh(
    spinner: &Spinner,
    full_matrix: &Mat3,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let length = spinner.length;

    let vertices: Vec<VertexWrapper> = SPINNER_BRACKET_MESH
        .iter()
        .map(|v| {
            let vert = full_matrix.multiply_vector(Vec3 {
                x: v.x,
                y: v.y,
                z: v.z,
            });

            let norm = full_matrix.multiply_vector_no_translate(Vec3 {
                x: v.nx,
                y: v.ny,
                z: v.nz,
            });

            VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: vert.x * length,
                    y: vert.y * length,
                    z: vert.z * length,
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = SPINNER_BRACKET_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, indices)
}

/// Generate the plate mesh
/// From VPinball Spinner::UpdatePlate
///
/// Note: In VPinball, the plate rotates around X axis based on the current angle.
/// For static export, we export at angle 0 (upright position).
fn generate_plate_mesh(
    spinner: &Spinner,
    full_matrix: &Mat3,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let length = spinner.length;

    // For static export, we use angle 0 (no X rotation)
    // In VPinball: fullMatrix = MatrixRotateX(-angle) * MatrixRotateZ(rotation)
    // With angle = 0, this simplifies to just the Z rotation

    let vertices: Vec<VertexWrapper> = SPINNER_PLATE_MESH
        .iter()
        .map(|v| {
            let vert = full_matrix.multiply_vector(Vec3 {
                x: v.x,
                y: v.y,
                z: v.z,
            });

            let norm = full_matrix.multiply_vector_no_translate(Vec3 {
                x: v.nx,
                y: v.ny,
                z: v.nz,
            });

            VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: vert.x * length,
                    y: vert.y * length,
                    z: vert.z * length,
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = SPINNER_PLATE_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    #[test]
    fn test_build_spinner_meshes_with_bracket() {
        let spinner = Spinner {
            center: Vertex2D::new(500.0, 500.0),
            length: 80.0,
            height: 60.0,
            rotation: 0.0,
            show_bracket: true,
            is_visible: true,
            ..Default::default()
        };

        let meshes = build_spinner_meshes(&spinner);

        // Check bracket mesh
        assert!(meshes.bracket.is_some());
        let (bracket_verts, bracket_indices) = meshes.bracket.unwrap();
        assert_eq!(bracket_verts.len(), SPINNER_BRACKET_NUM_VERTICES);
        assert_eq!(bracket_indices.len(), SPINNER_BRACKET_NUM_FACES / 3);

        // Check plate mesh
        let (plate_verts, plate_indices) = meshes.plate;
        assert_eq!(plate_verts.len(), SPINNER_PLATE_NUM_VERTICES);
        assert_eq!(plate_indices.len(), SPINNER_PLATE_NUM_FACES / 3);
    }

    #[test]
    fn test_build_spinner_meshes_without_bracket() {
        let spinner = Spinner {
            center: Vertex2D::new(500.0, 500.0),
            length: 80.0,
            height: 60.0,
            rotation: 0.0,
            show_bracket: false,
            is_visible: true,
            ..Default::default()
        };

        let meshes = build_spinner_meshes(&spinner);

        // Bracket should not be present
        assert!(meshes.bracket.is_none());

        // Plate should still be present
        let (plate_verts, plate_indices) = meshes.plate;
        assert_eq!(plate_verts.len(), SPINNER_PLATE_NUM_VERTICES);
        assert_eq!(plate_indices.len(), SPINNER_PLATE_NUM_FACES / 3);
    }
}
