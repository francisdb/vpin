//! Light mesh generation for VPinball lights
//!
//! Lights can have bulb meshes when `show_bulb_mesh` is true.
//! The bulb mesh consists of two parts:
//! - Bulb (the glass dome)
//! - Socket (the base)
//!
//! Ported from VPinball light.cpp

mod bulb_light_mesh;
mod bulb_socket_mesh;

use crate::vpx::gameitem::light::Light;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

use bulb_light_mesh::{BULB_LIGHT_INDICES, BULB_LIGHT_VERTICES};
use bulb_socket_mesh::{BULB_SOCKET_INDICES, BULB_SOCKET_VERTICES};

/// Result of building light meshes
pub struct LightMeshes {
    /// The bulb (glass dome) mesh, if show_bulb_mesh is true
    pub bulb: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The socket (base) mesh, if show_bulb_mesh is true
    pub socket: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// Build meshes for a light
///
/// Returns None if show_bulb_mesh is false or the light is a backglass light
///
/// Based on VPinball light.cpp RenderSetup():
/// ```cpp
/// if (m_d.m_showBulbMesh) {
///     const float bulb_z = m_surfaceHeight;
///     for (unsigned int i = 0; i < bulbLightNumVertices; i++) {
///         buf[i].x = bulbLight[i].x * m_d.m_meshRadius + m_d.m_vCenter.x;
///         buf[i].y = bulbLight[i].y * m_d.m_meshRadius + m_d.m_vCenter.y;
///         buf[i].z = bulbLight[i].z * m_d.m_meshRadius + bulb_z;
///         // normals and UVs are copied directly
///     }
/// }
/// ```
pub fn build_light_meshes(light: &Light, surface_height: f32) -> Option<LightMeshes> {
    // Skip backglass lights
    if light.is_backglass {
        return None;
    }

    // Only generate meshes if show_bulb_mesh is true
    if !light.show_bulb_mesh {
        return None;
    }

    let center_x = light.center.x;
    let center_y = light.center.y;
    // VPinball places the bulb mesh at surface height only, NOT surface + light.height
    // The light.height is used for the light emission point (halo), not the physical mesh
    // See light.cpp line 385: const float bulb_z = m_surfaceHeight;
    let bulb_z = surface_height;
    let mesh_radius = light.mesh_radius;

    // Build bulb mesh
    let bulb = build_bulb_mesh(center_x, center_y, bulb_z, mesh_radius);

    // Build socket mesh
    let socket = build_socket_mesh(center_x, center_y, bulb_z, mesh_radius);

    Some(LightMeshes {
        bulb: Some(bulb),
        socket: Some(socket),
    })
}

/// Build the bulb (glass dome) mesh
fn build_bulb_mesh(
    center_x: f32,
    center_y: f32,
    bulb_z: f32,
    mesh_radius: f32,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let vertices: Vec<VertexWrapper> = BULB_LIGHT_VERTICES
        .iter()
        .map(|v| {
            let vertex = Vertex3dNoTex2 {
                x: v.x * mesh_radius + center_x,
                y: v.y * mesh_radius + center_y,
                z: v.z * mesh_radius + bulb_z,
                nx: v.nx,
                ny: v.ny,
                nz: v.nz,
                tu: v.tu,
                tv: v.tv,
            };
            VertexWrapper::new([0u8; 32], vertex)
        })
        .collect();

    let indices: Vec<VpxFace> = BULB_LIGHT_INDICES
        .chunks(3)
        .map(|chunk| VpxFace::new(chunk[0] as i64, chunk[1] as i64, chunk[2] as i64))
        .collect();

    (vertices, indices)
}

/// Build the socket (base) mesh
fn build_socket_mesh(
    center_x: f32,
    center_y: f32,
    bulb_z: f32,
    mesh_radius: f32,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let vertices: Vec<VertexWrapper> = BULB_SOCKET_VERTICES
        .iter()
        .map(|v| {
            let vertex = Vertex3dNoTex2 {
                x: v.x * mesh_radius + center_x,
                y: v.y * mesh_radius + center_y,
                z: v.z * mesh_radius + bulb_z,
                nx: v.nx,
                ny: v.ny,
                nz: v.nz,
                tu: v.tu,
                tv: v.tv,
            };
            VertexWrapper::new([0u8; 32], vertex)
        })
        .collect();

    let indices: Vec<VpxFace> = BULB_SOCKET_INDICES
        .chunks(3)
        .map(|chunk| VpxFace::new(chunk[0] as i64, chunk[1] as i64, chunk[2] as i64))
        .collect();

    (vertices, indices)
}

/// Write light meshes to files
#[allow(dead_code)]
pub fn write_light_meshes(
    gameitems_dir: &std::path::Path,
    light: &Light,
    json_file_name: &str,
    mesh_format: crate::vpx::expanded::PrimitiveMeshFormat,
    fs: &dyn crate::filesystem::FileSystem,
) -> Result<(), crate::vpx::expanded::WriteError> {
    use crate::vpx::expanded::WriteError;
    use crate::vpx::gltf::{GltfContainer, write_gltf};
    use crate::vpx::obj::write_obj;
    use std::io;

    // Use 0.0 as default surface height for individual mesh export
    // In full table export, the actual surface height is calculated
    let surface_height = 0.0;

    let Some(meshes) = build_light_meshes(light, surface_height) else {
        return Ok(());
    };

    let file_name_without_ext = json_file_name.trim_end_matches(".json");

    // Write bulb mesh
    if let Some((vertices, indices)) = meshes.bulb {
        let mesh_name = format!("{}-bulb", file_name_without_ext);
        match mesh_format {
            crate::vpx::expanded::PrimitiveMeshFormat::Obj => {
                let path = gameitems_dir.join(format!("{}.obj", mesh_name));
                write_obj(&mesh_name, &vertices, &indices, &path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            crate::vpx::expanded::PrimitiveMeshFormat::Glb => {
                let path = gameitems_dir.join(format!("{}.glb", mesh_name));
                write_gltf(
                    &mesh_name,
                    &vertices,
                    &indices,
                    &path,
                    GltfContainer::Glb,
                    fs,
                )
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            crate::vpx::expanded::PrimitiveMeshFormat::Gltf => {
                let path = gameitems_dir.join(format!("{}.gltf", mesh_name));
                write_gltf(
                    &mesh_name,
                    &vertices,
                    &indices,
                    &path,
                    GltfContainer::Gltf,
                    fs,
                )
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
        }
    }

    // Write socket mesh
    if let Some((vertices, indices)) = meshes.socket {
        let mesh_name = format!("{}-socket", file_name_without_ext);
        match mesh_format {
            crate::vpx::expanded::PrimitiveMeshFormat::Obj => {
                let path = gameitems_dir.join(format!("{}.obj", mesh_name));
                write_obj(&mesh_name, &vertices, &indices, &path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            crate::vpx::expanded::PrimitiveMeshFormat::Glb => {
                let path = gameitems_dir.join(format!("{}.glb", mesh_name));
                write_gltf(
                    &mesh_name,
                    &vertices,
                    &indices,
                    &path,
                    GltfContainer::Glb,
                    fs,
                )
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            crate::vpx::expanded::PrimitiveMeshFormat::Gltf => {
                let path = gameitems_dir.join(format!("{}.gltf", mesh_name));
                write_gltf(
                    &mesh_name,
                    &vertices,
                    &indices,
                    &path,
                    GltfContainer::Gltf,
                    fs,
                )
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::light::Light;

    fn create_test_light(show_bulb_mesh: bool) -> Light {
        let mut light = Light::default();
        light.center.x = 100.0;
        light.center.y = 200.0;
        light.height = Some(50.0);
        light.name = "TestLight".to_string();
        light.is_backglass = false;
        light.show_bulb_mesh = show_bulb_mesh;
        light.mesh_radius = 20.0;
        light
    }

    #[test]
    fn test_build_light_meshes_with_bulb() {
        let light = create_test_light(true);
        let meshes = build_light_meshes(&light, 0.0);

        assert!(meshes.is_some());
        let meshes = meshes.unwrap();

        // Check bulb mesh
        assert!(meshes.bulb.is_some());
        let (vertices, indices) = meshes.bulb.unwrap();
        assert_eq!(vertices.len(), 67);
        assert_eq!(indices.len(), 120); // 360 indices / 3

        // Check socket mesh
        assert!(meshes.socket.is_some());
        let (vertices, indices) = meshes.socket.unwrap();
        assert_eq!(vertices.len(), 592);
        assert_eq!(indices.len(), 1128); // 3384 indices / 3
    }

    #[test]
    fn test_build_light_meshes_without_bulb() {
        let light = create_test_light(false);
        let meshes = build_light_meshes(&light, 0.0);

        assert!(meshes.is_none());
    }

    #[test]
    fn test_build_light_meshes_backglass() {
        let mut light = create_test_light(true);
        light.is_backglass = true;
        let meshes = build_light_meshes(&light, 0.0);

        assert!(meshes.is_none());
    }

    #[test]
    fn test_mesh_transformation() {
        let light = create_test_light(true);
        let surface_height = 100.0;
        let meshes = build_light_meshes(&light, surface_height).unwrap();

        let (vertices, _) = meshes.bulb.unwrap();

        // All vertices should be centered around (100, 200) with height offset
        for v in &vertices {
            // Check that vertices are transformed
            // The original mesh is centered at origin, so after transformation
            // vertices should be near the light center
            assert!(
                (v.vertex.x - light.center.x).abs() < light.mesh_radius * 2.0,
                "x should be near center"
            );
            assert!(
                (v.vertex.y - light.center.y).abs() < light.mesh_radius * 2.0,
                "y should be near center"
            );
        }
    }
}
