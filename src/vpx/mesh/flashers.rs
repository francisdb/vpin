//! Flasher mesh generation for expanded VPX export
//!
//! This module ports the flasher mesh generation from Visual Pinball's flasher.cpp.
//! Flashers are flat polygons defined by drag points, with optional rotation and height.

use super::super::mesh::{detail_level_to_accuracy, get_rg_vertex_2d, polygon_to_triangles};
use crate::vpx::TableDimensions;
use crate::vpx::gameitem::flasher::Flasher;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
use crate::vpx::math::Vec3;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;

/// Apply rotation transformation to flasher mesh
///
/// Vertices are transformed relative to the center point and remain centered at origin.
/// Height is NOT applied here - it should be part of the glTF node translation.
fn apply_rotation(
    vertices: &mut [Vertex3dNoTex2],
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    center_x: f32,
    center_y: f32,
) {
    if rot_x == 0.0 && rot_y == 0.0 && rot_z == 0.0 {
        // Just center vertices (no rotation needed)
        for v in vertices.iter_mut() {
            v.x -= center_x;
            v.y -= center_y;
            v.z = 0.0;
        }
        return;
    }

    // Convert degrees to radians
    let rad_x = rot_x * PI / 180.0;
    let rad_y = rot_y * PI / 180.0;
    let rad_z = rot_z * PI / 180.0;

    let cos_x = rad_x.cos();
    let sin_x = rad_x.sin();
    let cos_y = rad_y.cos();
    let sin_y = rad_y.sin();
    let cos_z = rad_z.cos();
    let sin_z = rad_z.sin();

    // Combined rotation matrix (Z * Y * X)
    let m00 = cos_z * cos_y;
    let m01 = cos_z * sin_y * sin_x - sin_z * cos_x;
    let m02 = cos_z * sin_y * cos_x + sin_z * sin_x;
    let m10 = sin_z * cos_y;
    let m11 = sin_z * sin_y * sin_x + cos_z * cos_x;
    let m12 = sin_z * sin_y * cos_x - cos_z * sin_x;
    let m20 = -sin_y;
    let m21 = cos_y * sin_x;
    let m22 = cos_y * cos_x;

    for v in vertices.iter_mut() {
        // Translate to center (vertices will stay centered at origin)
        let x = v.x - center_x;
        let y = v.y - center_y;
        let z = 0.0; // Flashers start flat

        // Apply rotation
        let new_x = m00 * x + m01 * y + m02 * z;
        let new_y = m10 * x + m11 * y + m12 * z;
        let new_z = m20 * x + m21 * y + m22 * z;

        // Keep centered at origin (height will be in node translation)
        v.x = new_x;
        v.y = new_y;
        v.z = new_z;

        // Also rotate normals
        let nx = v.nx;
        let ny = v.ny;
        let nz = v.nz;

        v.nx = m00 * nx + m01 * ny + m02 * nz;
        v.ny = m10 * nx + m11 * ny + m12 * nz;
        v.nz = m20 * nx + m21 * ny + m22 * nz;
    }
}

/// Build the complete flasher mesh
///
/// Returns vertices centered at origin, along with the center position in VPX coordinates.
/// The center position should be used as a glTF node transform.
///
/// # Returns
/// A tuple of (vertices, faces, position) or None if invalid.
/// Position is (x, y, z) in VPX coordinates (center_x, center_y, height).
pub(crate) fn build_flasher_mesh(
    flasher: &Flasher,
    table_dims: &TableDimensions,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>, Vec3)> {
    if flasher.drag_points.len() < 3 {
        return None;
    }

    // From VPinball mesh.h GetRgVertex: detail_level=10 gives accuracy=4.0 (highest detail)
    let accuracy = detail_level_to_accuracy(10.0);
    // Flashers always loop (closed polygon)
    let vvertex = get_rg_vertex_2d(&flasher.drag_points, accuracy, true);

    if vvertex.len() < 3 {
        return None;
    }

    // Calculate bounds for texture coordinates (used for Wrap mode)
    let mut minx = f32::MAX;
    let mut miny = f32::MAX;
    let mut maxx = f32::MIN;
    let mut maxy = f32::MIN;

    for v in &vvertex {
        if v.x < minx {
            minx = v.x;
        }
        if v.x > maxx {
            maxx = v.x;
        }
        if v.y < miny {
            miny = v.y;
        }
        if v.y > maxy {
            maxy = v.y;
        }
    }

    // Determine UV mapping based on image_alignment
    // VPinball flasher.cpp lines 150-169:
    // - ImageModeWrap: uses local bounding box (minx, miny, maxx, maxy)
    // - ImageModeWorld: uses table coordinates (m_left, m_top, m_right, m_bottom)
    let use_world_coords = flasher.image_alignment == RampImageAlignment::World;

    let (uv_minx, uv_miny, uv_maxx, uv_maxy) = if use_world_coords {
        (
            table_dims.left,
            table_dims.top,
            table_dims.right,
            table_dims.bottom,
        )
    } else {
        (minx, miny, maxx, maxy)
    };

    let inv_width = if (uv_maxx - uv_minx).abs() > 1e-6 {
        1.0 / (uv_maxx - uv_minx)
    } else {
        1.0
    };
    let inv_height = if (uv_maxy - uv_miny).abs() > 1e-6 {
        1.0 / (uv_maxy - uv_miny)
    } else {
        1.0
    };

    // Create vertices with texture coordinates
    let mut vertices: Vec<Vertex3dNoTex2> = vvertex
        .iter()
        .map(|v| {
            Vertex3dNoTex2 {
                x: v.x,
                y: v.y,
                z: 0.0, // Will be set by rotation
                nx: 0.0,
                ny: 0.0,
                nz: -1.0, // Flat surface, normal pointing down (visible from above after winding reversal)
                tu: (v.x - uv_minx) * inv_width,
                tv: (v.y - uv_miny) * inv_height,
            }
        })
        .collect();

    // Triangulate the polygon
    let indices = polygon_to_triangles(&vvertex);

    if indices.is_empty() {
        return None;
    }

    // Calculate center for rotation and node transform
    let center_x = (minx + maxx) * 0.5;
    let center_y = (miny + maxy) * 0.5;

    // Apply rotation transformation (vertices will be centered at origin)
    apply_rotation(
        &mut vertices,
        flasher.rot_x,
        flasher.rot_y,
        flasher.rot_z,
        center_x,
        center_y,
    );

    let wrapped = vertices
        .into_iter()
        .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
        .collect();

    let faces = indices
        .chunks_exact(3)
        .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
        .collect();

    Some((
        wrapped,
        faces,
        Vec3::new(center_x, center_y, flasher.height),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::dragpoint::DragPoint;

    #[test]
    fn test_simple_flasher() {
        let mut flasher = Flasher::default();
        flasher.height = 50.0;
        flasher.drag_points = vec![
            DragPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 100.0,
                y: 0.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 100.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
        ];

        let result = build_flasher_mesh(&flasher, &TableDimensions::new(0.0, 0.0, 1000.0, 2000.0));
        assert!(result.is_some());

        let (vertices, indices, position) = result.unwrap();
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 2); // 2 triangles for a quad

        // Center should be at (50, 50) for a 0-100 x 0-100 quad, height = 50
        assert!((position.x - 50.0).abs() < 0.01);
        assert!((position.y - 50.0).abs() < 0.01);
        assert!((position.z - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_flasher_with_rotation() {
        let mut flasher = Flasher::default();
        flasher.height = 50.0;
        flasher.rot_x = 45.0;
        flasher.rot_y = 30.0;
        flasher.rot_z = 15.0;
        flasher.drag_points = vec![
            DragPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 100.0,
                y: 0.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 100.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
        ];

        let result = build_flasher_mesh(&flasher, &TableDimensions::new(0.0, 0.0, 1000.0, 2000.0));
        assert!(result.is_some());

        let (vertices, _indices, _position) = result.unwrap();
        // Verify z values vary due to rotation (not all at 0)
        let has_varied_z = vertices.iter().any(|v| v.vertex.z.abs() > 0.01);
        assert!(has_varied_z, "Rotation should cause varied z values");
    }

    #[test]
    fn test_triangle_flasher() {
        let mut flasher = Flasher::default();
        flasher.height = 25.0;
        flasher.drag_points = vec![
            DragPoint {
                x: 50.0,
                y: 0.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 100.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: 100.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
        ];

        let result = build_flasher_mesh(&flasher, &TableDimensions::new(0.0, 0.0, 1000.0, 2000.0));
        assert!(result.is_some());

        let (vertices, indices, _position) = result.unwrap();
        assert_eq!(vertices.len(), 3);
        assert_eq!(indices.len(), 1); // 1 triangle
    }

    #[test]
    fn test_clockwise_flasher() {
        // This matches the Flasher.4Points001.json example which has clockwise winding
        let mut flasher = Flasher::default();
        flasher.height = 50.0;
        flasher.drag_points = vec![
            DragPoint {
                x: 810.0,
                y: 1350.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 883.0,
                y: 1930.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 952.0,
                y: 1930.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 952.0,
                y: 1350.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
        ];

        let result = build_flasher_mesh(&flasher, &TableDimensions::new(0.0, 0.0, 1000.0, 2000.0));
        assert!(
            result.is_some(),
            "Clockwise winding flasher should generate a mesh"
        );

        let (vertices, indices, _position) = result.unwrap();
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 2); // 2 triangles for a quad
    }

    #[test]
    fn test_concave_l_shaped_polygon() {
        // L-shaped concave polygon (6 vertices):
        //   (0,0)---(100,0)
        //     |         |
        //     |    (100,50)---(200,50)
        //     |                  |
        //   (0,100)----------(200,100)
        //
        // This is concave at (100,50) — the old ear-clipping fallback
        // would fill it as convex, creating incorrect triangles.
        use crate::vpx::mesh::RenderVertex2D;
        let vertices = vec![
            RenderVertex2D {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            RenderVertex2D {
                x: 100.0,
                y: 0.0,
                ..Default::default()
            },
            RenderVertex2D {
                x: 100.0,
                y: 50.0,
                ..Default::default()
            },
            RenderVertex2D {
                x: 200.0,
                y: 50.0,
                ..Default::default()
            },
            RenderVertex2D {
                x: 200.0,
                y: 100.0,
                ..Default::default()
            },
            RenderVertex2D {
                x: 0.0,
                y: 100.0,
                ..Default::default()
            },
        ];

        let indices = polygon_to_triangles(&vertices);

        // 6 vertices should produce 4 triangles (n-2 = 4)
        assert_eq!(
            indices.len(),
            4 * 3,
            "L-shape (6 vertices) should produce 4 triangles, got {} indices",
            indices.len()
        );

        // Verify no triangle covers the concave "notch" area
        // The point (150, 25) is inside the bounding box but outside the L-shape
        // (it's in the cut-out rectangle between (100,0)→(200,50))
        // None of the triangles should contain this point.
        let test_point = (150.0_f32, 25.0_f32);
        for tri in indices.chunks(3) {
            let a = &vertices[tri[0] as usize];
            let b = &vertices[tri[1] as usize];
            let c = &vertices[tri[2] as usize];
            // Barycentric point-in-triangle test
            let v0x = c.x - a.x;
            let v0y = c.y - a.y;
            let v1x = b.x - a.x;
            let v1y = b.y - a.y;
            let v2x = test_point.0 - a.x;
            let v2y = test_point.1 - a.y;
            let dot00 = v0x * v0x + v0y * v0y;
            let dot01 = v0x * v1x + v0y * v1y;
            let dot02 = v0x * v2x + v0y * v2y;
            let dot11 = v1x * v1x + v1y * v1y;
            let dot12 = v1x * v2x + v1y * v2y;
            let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
            let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
            let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
            assert!(
                !(u >= 0.0 && v >= 0.0 && (u + v) < 1.0),
                "Point (150, 25) should NOT be inside any triangle of the L-shape, \
                 but was found inside triangle ({},{}) ({},{}) ({},{})",
                a.x,
                a.y,
                b.x,
                b.y,
                c.x,
                c.y
            );
        }
    }
}
