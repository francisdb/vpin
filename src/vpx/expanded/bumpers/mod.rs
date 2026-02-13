//! Bumper mesh generation for expanded VPX export
//!
//! This module ports the bumper mesh generation from Visual Pinball's bumper.cpp.
//! Bumpers use pre-defined base meshes that are scaled and transformed based on
//! the bumper's parameters (radius, height_scale, orientation, etc.).
//!
//! A bumper consists of 4 parts:
//! - Base: The fixed base of the bumper
//! - Socket (Skirt): The flexible skirt around the base
//! - Ring: The animated ring that moves up/down when hit
//! - Cap: The top cap of the bumper
//!
//! Ported from: VPinball/src/parts/bumper.cpp

mod bumper_base_mesh;
mod bumper_cap_mesh;
mod bumper_ring_mesh;
mod bumper_socket_mesh;

use super::mesh_common::{Matrix3D, Vec3, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::bumper::Bumper;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::path::Path;

pub use bumper_base_mesh::*;
pub use bumper_cap_mesh::*;
pub use bumper_ring_mesh::*;
pub use bumper_socket_mesh::*;

/// Result of bumper mesh generation with separate meshes for each part
pub struct BumperMeshes {
    /// The base mesh (uses bumper.base_material)
    pub base: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The socket/skirt mesh (uses bumper.socket_material)
    pub socket: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The ring mesh (uses bumper.ring_material)
    pub ring: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The cap mesh (uses bumper.cap_material)
    pub cap: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// Generate all bumper meshes based on the bumper parameters
///
/// # Arguments
/// * `bumper` - The bumper definition
/// * `base_height` - The height of the surface the bumper sits on (from table surface lookup)
///
/// # Returns
/// A BumperMeshes struct containing all visible bumper parts
pub fn build_bumper_meshes(bumper: &Bumper, base_height: f32) -> BumperMeshes {
    let full_matrix = Matrix3D::rotate_z(bumper.orientation.to_radians());

    BumperMeshes {
        base: if bumper.is_base_visible {
            Some(generate_base_mesh(bumper, base_height, &full_matrix))
        } else {
            None
        },
        socket: if bumper.is_socket_visible.unwrap_or(true) {
            Some(generate_socket_mesh(bumper, base_height, &full_matrix))
        } else {
            None
        },
        ring: if bumper.is_ring_visible.unwrap_or(true) {
            Some(generate_ring_mesh(bumper, base_height, &full_matrix))
        } else {
            None
        },
        cap: if bumper.is_cap_visible {
            Some(generate_cap_mesh(bumper, base_height, &full_matrix))
        } else {
            None
        },
    }
}

/// Write bumper meshes to individual files
pub(super) fn write_bumper_meshes(
    gameitems_dir: &Path,
    bumper: &Bumper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let bumper_meshes = build_bumper_meshes(bumper, 0.0);
    let file_name_base = json_file_name.trim_end_matches(".json");

    // Write base mesh
    if let Some((vertices, indices)) = bumper_meshes.base {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-base.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Base", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write socket mesh
    if let Some((vertices, indices)) = bumper_meshes.socket {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-socket.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Socket", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write ring mesh
    if let Some((vertices, indices)) = bumper_meshes.ring {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-ring.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Ring", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write cap mesh
    if let Some((vertices, indices)) = bumper_meshes.cap {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-cap.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Cap", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    Ok(())
}

/// Generate the base mesh
/// From VPinball Bumper::GenerateBaseMesh
fn generate_base_mesh(
    bumper: &Bumper,
    base_height: f32,
    full_matrix: &Matrix3D,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let scale_xy = bumper.radius;

    let vertices: Vec<VertexWrapper> = BUMPER_BASE_MESH
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
                    x: vert.x * scale_xy + bumper.center.x,
                    y: vert.y * scale_xy + bumper.center.y,
                    z: vert.z * bumper.height_scale + base_height,
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = BUMPER_BASE_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, indices)
}

/// Generate the socket/skirt mesh
/// From VPinball Bumper::GenerateSocketMesh
fn generate_socket_mesh(
    bumper: &Bumper,
    base_height: f32,
    full_matrix: &Matrix3D,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let scale_xy = bumper.radius;

    let vertices: Vec<VertexWrapper> = BUMPER_SOCKET_MESH
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
                    x: vert.x * scale_xy + bumper.center.x,
                    y: vert.y * scale_xy + bumper.center.y,
                    // Socket is offset by 5.0 from base height
                    z: vert.z * bumper.height_scale + (base_height + 5.0),
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = BUMPER_SOCKET_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, indices)
}

/// Generate the ring mesh
/// From VPinball Bumper::GenerateRingMesh
fn generate_ring_mesh(
    bumper: &Bumper,
    base_height: f32,
    full_matrix: &Matrix3D,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let scale_xy = bumper.radius;

    let vertices: Vec<VertexWrapper> = BUMPER_RING_MESH
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
                    x: vert.x * scale_xy + bumper.center.x,
                    y: vert.y * scale_xy + bumper.center.y,
                    z: vert.z * bumper.height_scale + base_height,
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = BUMPER_RING_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, indices)
}

/// Generate the cap mesh
/// From VPinball Bumper::GenerateCapMesh
fn generate_cap_mesh(
    bumper: &Bumper,
    base_height: f32,
    full_matrix: &Matrix3D,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    // Cap uses 2x the radius for scaling
    let scale_xy = bumper.radius * 2.0;

    let vertices: Vec<VertexWrapper> = BUMPER_CAP_MESH
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
                    x: vert.x * scale_xy + bumper.center.x,
                    y: vert.y * scale_xy + bumper.center.y,
                    // Cap Z is offset by height_scale from base + height_scale
                    z: vert.z * bumper.height_scale + (bumper.height_scale + base_height),
                    nx: norm.x,
                    ny: norm.y,
                    nz: norm.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            )
        })
        .collect();

    let indices: Vec<VpxFace> = BUMPER_CAP_INDICES
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
    fn test_build_bumper_meshes() {
        let mut bumper = Bumper::default();
        bumper.center = Vertex2D::new(500.0, 500.0);
        bumper.radius = 45.0;
        bumper.height_scale = 90.0;
        bumper.orientation = 0.0;
        bumper.is_base_visible = true;
        bumper.is_cap_visible = true;
        bumper.is_ring_visible = Some(true);
        bumper.is_socket_visible = Some(true);

        let meshes = build_bumper_meshes(&bumper, 0.0);

        // Check base mesh
        assert!(meshes.base.is_some());
        let (base_verts, base_indices) = meshes.base.unwrap();
        assert_eq!(base_verts.len(), BUMPER_BASE_NUM_VERTICES);
        assert_eq!(base_indices.len(), BUMPER_BASE_NUM_INDICES / 3);

        // Check socket mesh
        assert!(meshes.socket.is_some());
        let (socket_verts, socket_indices) = meshes.socket.unwrap();
        assert_eq!(socket_verts.len(), BUMPER_SOCKET_NUM_VERTICES);
        assert_eq!(socket_indices.len(), BUMPER_SOCKET_NUM_INDICES / 3);

        // Check ring mesh
        assert!(meshes.ring.is_some());
        let (ring_verts, ring_indices) = meshes.ring.unwrap();
        assert_eq!(ring_verts.len(), BUMPER_RING_NUM_VERTICES);
        assert_eq!(ring_indices.len(), BUMPER_RING_NUM_INDICES / 3);

        // Check cap mesh
        assert!(meshes.cap.is_some());
        let (cap_verts, cap_indices) = meshes.cap.unwrap();
        assert_eq!(cap_verts.len(), BUMPER_CAP_NUM_VERTICES);
        assert_eq!(cap_indices.len(), BUMPER_CAP_NUM_INDICES / 3);
    }

    #[test]
    fn test_bumper_visibility_flags() {
        let mut bumper = Bumper::default();
        bumper.center = Vertex2D::new(500.0, 500.0);
        bumper.radius = 45.0;
        bumper.height_scale = 90.0;
        bumper.orientation = 0.0;
        bumper.is_base_visible = false;
        bumper.is_cap_visible = false;
        bumper.is_ring_visible = Some(false);
        bumper.is_socket_visible = Some(false);

        let meshes = build_bumper_meshes(&bumper, 0.0);

        assert!(meshes.base.is_none());
        assert!(meshes.socket.is_none());
        assert!(meshes.ring.is_none());
        assert!(meshes.cap.is_none());
    }
}
