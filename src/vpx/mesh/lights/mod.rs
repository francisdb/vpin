//! Light mesh generation for VPinball lights
//!
//! Lights can have bulb meshes when `show_bulb_mesh` is true.
//! The bulb mesh consists of two parts:
//! - Bulb (the glass dome)
//! - Socket (the base)
//!
//! Lights also define their playfield shape via drag points. The insert mesh
//! is a flat polygon generated from these drag points, useful for extruding
//! holes in the playfield mesh (e.g., via boolean difference in Blender).
//!
//! Ported from VPinball light.cpp

mod bulb_light_mesh;
mod bulb_socket_mesh;

use crate::vpx::TableDimensions;
use crate::vpx::gameitem::light::Light;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::math::Vec3;
use crate::vpx::mesh::{detail_level_to_accuracy, get_rg_vertex_2d, polygon_to_triangles};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use bulb_light_mesh::{BULB_LIGHT_INDICES, BULB_LIGHT_VERTICES};
use bulb_socket_mesh::{BULB_SOCKET_INDICES, BULB_SOCKET_VERTICES};

/// Result of building light meshes
///
/// Vertices are centered at origin.
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
pub fn build_light_meshes(light: &Light) -> Option<LightMeshes> {
    // Skip backglass lights
    if light.is_backglass {
        return None;
    }

    // Only generate meshes if show_bulb_mesh is true
    if !light.show_bulb_mesh {
        return None;
    }

    let mesh_radius = light.mesh_radius;

    // Build bulb mesh (centered at origin)
    let bulb = build_bulb_mesh(mesh_radius);

    // Build socket mesh (centered at origin)
    let socket = build_socket_mesh(mesh_radius);

    Some(LightMeshes {
        bulb: Some(bulb),
        socket: Some(socket),
    })
}

/// Build a flat polygon mesh from a light's drag points (the "insert" shape).
///
/// This generates the 2D shape that the light occupies on the playfield surface.
/// The mesh is useful for boolean-cutting holes in the playfield (e.g., for
/// light inserts in Blender using a boolean modifier with "Difference" operation).
///
/// UV mapping follows VPinball's light.cpp UpdateMeshBuffer():
/// - With image (`image` set): table-space UVs (`tu = x / table_width`, `tv = y / table_height`)
/// - Without image: center-based UVs (`tu = 0.5 + (x - center) * inv_maxdist`)
///
/// Vertices are centered at origin. The returned center position should be used
/// as a glTF node translation.
///
/// Returns None if the light has fewer than 3 drag points or is a backglass light.
pub fn build_light_insert_mesh(
    light: &Light,
    table_dims: &TableDimensions,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>, Vec3)> {
    if light.is_backglass || light.drag_points.len() < 3 {
        return None;
    }

    // Use highest detail for the spline interpolation
    let accuracy = detail_level_to_accuracy(10.0);
    // Lights are closed polygons (like flashers)
    let vvertex = get_rg_vertex_2d(&light.drag_points, accuracy, true);

    if vvertex.len() < 3 {
        return None;
    }

    // Calculate bounds for centering
    let mut minx = f32::MAX;
    let mut miny = f32::MAX;
    let mut maxx = f32::MIN;
    let mut maxy = f32::MIN;

    for v in &vvertex {
        minx = minx.min(v.x);
        maxx = maxx.max(v.x);
        miny = miny.min(v.y);
        maxy = maxy.max(v.y);
    }

    let center_x = (minx + maxx) * 0.5;
    let center_y = (miny + maxy) * 0.5;

    let has_image = !light.image.is_empty();

    // Precompute UV mapping values based on whether the light has an image
    // VPinball light.cpp UpdateMeshBuffer():
    //   if (pin != nullptr) { tu = pv0->x * inv_tablewidth; tv = pv0->y * inv_tableheight; }
    //   else { tu = 0.5 + (pv0->x - center.x) * inv_maxdist; tv = 0.5 + (pv0->y - center.y) * inv_maxdist; }
    let inv_tablewidth = {
        let w = table_dims.right - table_dims.left;
        if w.abs() > 1e-6 { 1.0 / w } else { 0.0 }
    };
    let inv_tableheight = {
        let h = table_dims.bottom - table_dims.top;
        if h.abs() > 1e-6 { 1.0 / h } else { 0.0 }
    };

    // max_dist: squared distance from center to farthest vertex (for default light UVs)
    let max_dist = vvertex.iter().fold(0.0_f32, |acc, v| {
        let dx = v.x - light.center.x;
        let dy = v.y - light.center.y;
        acc.max(dx * dx + dy * dy)
    });
    let inv_maxdist = if max_dist > 0.0 {
        0.5 / max_dist.sqrt()
    } else {
        0.0
    };

    // Create vertices centered at origin, flat at z=0
    let vertices: Vec<VertexWrapper> = vvertex
        .iter()
        .map(|v| {
            let (tu, tv) = if has_image {
                // Table-space UVs (matching VPinball when image is set)
                (v.x * inv_tablewidth, v.y * inv_tableheight)
            } else {
                // Center-based UVs (default light, radial from center)
                (
                    0.5 + (v.x - light.center.x) * inv_maxdist,
                    0.5 + (v.y - light.center.y) * inv_maxdist,
                )
            };
            let vertex = Vertex3dNoTex2 {
                x: v.x - center_x,
                y: v.y - center_y,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0, // Normal pointing up in VPX Z → glTF Y (0,1,0), consistent with geometric face normal after winding reversal
                tu,
                tv,
            };
            VertexWrapper::new(vertex.to_vpx_bytes(), vertex)
        })
        .collect();

    // Triangulate the polygon
    let indices = polygon_to_triangles(&vvertex);

    if indices.is_empty() {
        return None;
    }

    let faces: Vec<VpxFace> = indices
        .chunks_exact(3)
        .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
        .collect();

    Some((vertices, faces, Vec3::new(center_x, center_y, 0.0)))
}

/// Build the bulb (glass dome) mesh
///
/// Vertices are centered at origin, scaled by mesh_radius.
fn build_bulb_mesh(mesh_radius: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let vertices: Vec<VertexWrapper> = BULB_LIGHT_VERTICES
        .iter()
        .map(|v| {
            let vertex = Vertex3dNoTex2 {
                x: v.x * mesh_radius,
                y: v.y * mesh_radius,
                z: v.z * mesh_radius,
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
///
/// Vertices are centered at origin, scaled by mesh_radius.
fn build_socket_mesh(mesh_radius: f32) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let vertices: Vec<VertexWrapper> = BULB_SOCKET_VERTICES
        .iter()
        .map(|v| {
            let vertex = Vertex3dNoTex2 {
                x: v.x * mesh_radius,
                y: v.y * mesh_radius,
                z: v.z * mesh_radius,
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
pub(crate) fn write_light_meshes(
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

    let Some(meshes) = build_light_meshes(light) else {
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

    fn default_table_dims() -> TableDimensions {
        TableDimensions::new(0.0, 0.0, 952.0, 2162.0)
    }

    #[test]
    fn test_build_light_meshes_with_bulb() {
        let light = create_test_light(true);
        let meshes = build_light_meshes(&light);

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
        let meshes = build_light_meshes(&light);

        assert!(meshes.is_none());
    }

    #[test]
    fn test_build_light_meshes_backglass() {
        let mut light = create_test_light(true);
        light.is_backglass = true;
        let meshes = build_light_meshes(&light);

        assert!(meshes.is_none());
    }

    #[test]
    fn test_mesh_transformation() {
        let light = create_test_light(true);
        let meshes = build_light_meshes(&light).unwrap();

        let (vertices, _) = meshes.bulb.unwrap();

        // Vertices should be centered at origin, scaled by mesh_radius
        for v in &vertices {
            // Check that vertices are scaled but centered at origin
            assert!(
                v.vertex.x.abs() < light.mesh_radius * 2.0,
                "x should be centered at origin"
            );
            assert!(
                v.vertex.y.abs() < light.mesh_radius * 2.0,
                "y should be centered at origin"
            );
        }
    }

    #[test]
    fn test_build_light_insert_mesh_with_drag_points() {
        use crate::vpx::gameitem::dragpoint::DragPoint;

        let mut light = create_test_light(false);
        // Create a simple square light insert
        light.drag_points = vec![
            DragPoint {
                x: 90.0,
                y: 190.0,
                ..Default::default()
            },
            DragPoint {
                x: 110.0,
                y: 190.0,
                ..Default::default()
            },
            DragPoint {
                x: 110.0,
                y: 210.0,
                ..Default::default()
            },
            DragPoint {
                x: 90.0,
                y: 210.0,
                ..Default::default()
            },
        ];

        let result = build_light_insert_mesh(&light, &default_table_dims());
        assert!(result.is_some());

        let (vertices, indices, center) = result.unwrap();

        // Should have 4 vertices (no smoothing)
        assert_eq!(vertices.len(), 4);
        // Should triangulate into 2 triangles
        assert_eq!(indices.len(), 2);

        // Center should be at the middle of the drag points
        assert!((center.x - 100.0).abs() < 0.01);
        assert!((center.y - 200.0).abs() < 0.01);
        assert!((center.z - 0.0).abs() < 0.01);

        // All vertices should be centered at origin
        for v in &vertices {
            assert!(
                v.vertex.x.abs() <= 10.01,
                "x should be within +-10 of origin, got {}",
                v.vertex.x
            );
            assert!(
                v.vertex.y.abs() <= 10.01,
                "y should be within +-10 of origin, got {}",
                v.vertex.y
            );
            assert_eq!(v.vertex.z, 0.0, "z should be 0 for flat insert mesh");
        }
    }

    #[test]
    fn test_build_light_insert_mesh_no_drag_points() {
        let light = create_test_light(false);
        // No drag points
        let result = build_light_insert_mesh(&light, &default_table_dims());
        assert!(result.is_none());
    }

    #[test]
    fn test_build_light_insert_mesh_backglass() {
        use crate::vpx::gameitem::dragpoint::DragPoint;

        let mut light = create_test_light(false);
        light.is_backglass = true;
        light.drag_points = vec![
            DragPoint {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            DragPoint {
                x: 10.0,
                y: 0.0,
                ..Default::default()
            },
            DragPoint {
                x: 10.0,
                y: 10.0,
                ..Default::default()
            },
        ];

        let result = build_light_insert_mesh(&light, &default_table_dims());
        assert!(result.is_none());
    }

    /// Verify that vertex normals agree with geometric face normals after the
    /// glTF coordinate transform (VPX→glTF) and winding reversal.
    /// A mismatch causes Cycles to render the surface black.
    #[test]
    fn test_light_insert_vertex_normals_match_geometric_face_normals_after_gltf_transform() {
        use crate::vpx::gameitem::dragpoint::DragPoint;
        use crate::vpx::mesh::mesh_validation::check_normal_consistency_gltf;

        let mut light = create_test_light(false);
        light.drag_points = vec![
            DragPoint {
                x: 90.0,
                y: 190.0,
                ..Default::default()
            },
            DragPoint {
                x: 110.0,
                y: 190.0,
                ..Default::default()
            },
            DragPoint {
                x: 110.0,
                y: 210.0,
                ..Default::default()
            },
            DragPoint {
                x: 90.0,
                y: 210.0,
                ..Default::default()
            },
        ];

        let (vertices, faces, _center) =
            build_light_insert_mesh(&light, &default_table_dims()).unwrap();

        let inconsistent = check_normal_consistency_gltf(&vertices, &faces);
        assert!(
            inconsistent.is_empty(),
            "Light insert has {} faces where vertex normals disagree with geometric face normals after glTF transform (causes Blender Cycles black rendering): {:?}",
            inconsistent.len(),
            inconsistent
        );
    }
}
