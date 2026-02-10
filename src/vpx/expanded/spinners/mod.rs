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

use super::mesh_common::{Matrix3D, Vec3, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::spinner::Spinner;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

pub use spinner_bracket_mesh::*;
pub use spinner_plate_mesh::*;

/// Result of spinner mesh generation with separate meshes for each part
pub struct SpinnerMeshes {
    /// The bracket mesh (uses a default metal material)
    /// Only present if `spinner.show_bracket` is true
    pub bracket: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The plate mesh (uses spinner.material and spinner.image)
    pub plate: (Vec<VertexWrapper>, Vec<VpxFace>),
}

/// Degrees to radians conversion
fn deg_to_rad(deg: f32) -> f32 {
    deg * PI / 180.0
}

/// Generate all spinner meshes based on the spinner parameters
///
/// # Arguments
/// * `spinner` - The spinner definition
/// * `base_height` - The height of the surface the spinner sits on (from table surface lookup)
///
/// # Returns
/// A SpinnerMeshes struct containing all spinner parts
pub fn build_spinner_meshes(spinner: &Spinner, base_height: f32) -> SpinnerMeshes {
    let pos_z = base_height + spinner.height;
    let full_matrix = Matrix3D::rotate_z(deg_to_rad(spinner.rotation));

    SpinnerMeshes {
        bracket: if spinner.show_bracket {
            Some(generate_bracket_mesh(spinner, pos_z, &full_matrix))
        } else {
            None
        },
        plate: generate_plate_mesh(spinner, pos_z, &full_matrix),
    }
}

/// Generate the bracket mesh
/// From VPinball Spinner::ExportMesh (bracket section)
fn generate_bracket_mesh(
    spinner: &Spinner,
    pos_z: f32,
    full_matrix: &Matrix3D,
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
                    x: vert.x * length + spinner.center.x,
                    y: vert.y * length + spinner.center.y,
                    z: vert.z * length + pos_z,
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
    pos_z: f32,
    full_matrix: &Matrix3D,
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
                    x: vert.x * length + spinner.center.x,
                    y: vert.y * length + spinner.center.y,
                    z: vert.z * length + pos_z,
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

/// Write spinner meshes to file
pub(super) fn write_spinner_meshes(
    gameitems_dir: &Path,
    spinner: &Spinner,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    // TODO: get surface height from the table
    let meshes = build_spinner_meshes(spinner, 0.0);

    // Write bracket mesh if present
    if let Some((bracket_vertices, bracket_indices)) = meshes.bracket {
        let bracket_mesh_name = format!("{}-bracket", json_file_name.trim_end_matches(".json"));
        let bracket_mesh_path =
            gameitems_dir.join(generated_mesh_file_name(&bracket_mesh_name, mesh_format));
        write_mesh_to_file(
            &bracket_mesh_path,
            &format!("{}Bracket", spinner.name),
            &bracket_vertices,
            &bracket_indices,
            mesh_format,
            fs,
        )?;
    }

    // Write plate mesh
    let (plate_vertices, plate_indices) = meshes.plate;
    let plate_mesh_name = format!("{}-plate", json_file_name.trim_end_matches(".json"));
    let plate_mesh_path =
        gameitems_dir.join(generated_mesh_file_name(&plate_mesh_name, mesh_format));
    write_mesh_to_file(
        &plate_mesh_path,
        &format!("{}Plate", spinner.name),
        &plate_vertices,
        &plate_indices,
        mesh_format,
        fs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    #[test]
    fn test_build_spinner_meshes_with_bracket() {
        let mut spinner = Spinner::default();
        spinner.center = Vertex2D::new(500.0, 500.0);
        spinner.length = 80.0;
        spinner.height = 60.0;
        spinner.rotation = 0.0;
        spinner.show_bracket = true;
        spinner.is_visible = true;

        let meshes = build_spinner_meshes(&spinner, 0.0);

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
        let mut spinner = Spinner::default();
        spinner.center = Vertex2D::new(500.0, 500.0);
        spinner.length = 80.0;
        spinner.height = 60.0;
        spinner.rotation = 0.0;
        spinner.show_bracket = false;
        spinner.is_visible = true;

        let meshes = build_spinner_meshes(&spinner, 0.0);

        // Bracket should not be present
        assert!(meshes.bracket.is_none());

        // Plate should still be present
        let (plate_verts, plate_indices) = meshes.plate;
        assert_eq!(plate_verts.len(), SPINNER_PLATE_NUM_VERTICES);
        assert_eq!(plate_indices.len(), SPINNER_PLATE_NUM_FACES / 3);
    }
}
