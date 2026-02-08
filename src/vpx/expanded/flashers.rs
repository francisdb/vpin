//! Flasher mesh generation for expanded VPX export
//!
//! This module ports the flasher mesh generation from Visual Pinball's flasher.cpp.
//! Flashers are flat polygons defined by drag points, with optional rotation and height.

use super::mesh_common::{
    RenderVertex2D, generated_mesh_file_name, get_rg_vertex_2d, write_mesh_to_file,
};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::flasher::Flasher;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

/// Find the corner vertex (minimum Y, in case of tie also minimum X)
/// This matches VPinball's FindCornerVertex function
fn find_corner_vertex(vertices: &[RenderVertex2D]) -> usize {
    let mut min_vertex = 0;
    let mut min_y = f32::MAX;
    let mut min_x_at_min_y = f32::MAX;

    for (i, vert) in vertices.iter().enumerate() {
        if vert.y > min_y {
            continue;
        }
        if vert.y == min_y && vert.x >= min_x_at_min_y {
            continue;
        }
        // Minimum so far
        min_vertex = i;
        min_y = vert.y;
        min_x_at_min_y = vert.x;
    }

    min_vertex
}

/// Determine the winding order of a polygon
/// Returns true if clockwise, false if counter-clockwise
/// This matches VPinball's DetermineWindingOrder function
fn is_clockwise(vertices: &[RenderVertex2D]) -> bool {
    let n = vertices.len();
    if n < 3 {
        return false;
    }

    let i_min = find_corner_vertex(vertices);

    // Get the three vertices around the corner
    let a = &vertices[(i_min + n - 1) % n];
    let b = &vertices[i_min];
    let c = &vertices[(i_min + 1) % n];

    // Orientation matrix determinant:
    // det(O) = (xb*yc + xa*yb + ya*xc) - (ya*xb + yb*xc + xa*yc)
    let det_orient = (b.x * c.y + a.x * b.y + a.y * c.x) - (a.y * b.x + b.y * c.x + a.x * c.y);

    // VPinball: detOrient > 0 means Clockwise
    det_orient > 0.0
}

/// Triangulate a polygon using ear clipping algorithm
/// Returns indices into the vertex array forming triangles
///
/// This follows VPinball's PolygonToTriangles with support_both_winding_orders=true
fn polygon_to_triangles(vertices: &[RenderVertex2D]) -> Vec<u32> {
    let n = vertices.len();
    if n < 3 {
        return vec![];
    }

    // Check winding order - if clockwise, we'll process in reverse order
    // This matches VPinball's approach of reversing the polygon for clockwise winding
    let clockwise = is_clockwise(vertices);

    // Simple ear clipping triangulation
    let mut indices = Vec::with_capacity((n - 2) * 3);
    let mut remaining: Vec<usize> = if clockwise {
        // Reverse the order for clockwise polygons (same as VPinball)
        (0..n).rev().collect()
    } else {
        (0..n).collect()
    };

    while remaining.len() > 3 {
        let len = remaining.len();
        let mut ear_found = false;

        for i in 0..len {
            let prev = remaining[(i + len - 1) % len];
            let curr = remaining[i];
            let next = remaining[(i + 1) % len];

            // After reversing for CW, we always check for CCW ears
            if is_ear(vertices, &remaining, prev, curr, next) {
                indices.push(prev as u32);
                indices.push(curr as u32);
                indices.push(next as u32);
                remaining.remove(i);
                ear_found = true;
                break;
            }
        }

        if !ear_found {
            // Fallback: just create a fan from first vertex
            // This handles degenerate cases
            for i in 1..remaining.len() - 1 {
                indices.push(remaining[0] as u32);
                indices.push(remaining[i] as u32);
                indices.push(remaining[i + 1] as u32);
            }
            break;
        }
    }

    // Add the last triangle
    if remaining.len() == 3 {
        indices.push(remaining[0] as u32);
        indices.push(remaining[1] as u32);
        indices.push(remaining[2] as u32);
    }

    indices
}

/// Check if vertex at index `curr` forms an ear (convex and no other vertices inside)
/// Since we normalize all polygons to CCW order, we always check for positive cross product
fn is_ear(
    vertices: &[RenderVertex2D],
    remaining: &[usize],
    prev: usize,
    curr: usize,
    next: usize,
) -> bool {
    let a = &vertices[prev];
    let b = &vertices[curr];
    let c = &vertices[next];

    // Check if the triangle is convex (CCW winding - positive cross product)
    let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if cross <= 0.0 {
        return false;
    }

    // Check if any other vertex is inside this triangle
    for &idx in remaining {
        if idx == prev || idx == curr || idx == next {
            continue;
        }
        if point_in_triangle(&vertices[idx], a, b, c) {
            return false;
        }
    }

    true
}

/// Check if point p is inside triangle abc
fn point_in_triangle(
    p: &RenderVertex2D,
    a: &RenderVertex2D,
    b: &RenderVertex2D,
    c: &RenderVertex2D,
) -> bool {
    let v0x = c.x - a.x;
    let v0y = c.y - a.y;
    let v1x = b.x - a.x;
    let v1y = b.y - a.y;
    let v2x = p.x - a.x;
    let v2y = p.y - a.y;

    let dot00 = v0x * v0x + v0y * v0y;
    let dot01 = v0x * v1x + v0y * v1y;
    let dot02 = v0x * v2x + v0y * v2y;
    let dot11 = v1x * v1x + v1y * v1y;
    let dot12 = v1x * v2x + v1y * v2y;

    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= 0.0 && v >= 0.0 && (u + v) < 1.0
}

/// Apply rotation transformation to flasher mesh
fn apply_rotation(
    vertices: &mut [Vertex3dNoTex2],
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    height: f32,
    center_x: f32,
    center_y: f32,
) {
    if rot_x == 0.0 && rot_y == 0.0 && rot_z == 0.0 {
        // Just apply height offset
        for v in vertices.iter_mut() {
            v.z = height;
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
        // Translate to center
        let x = v.x - center_x;
        let y = v.y - center_y;
        let z = 0.0; // Flashers start flat

        // Apply rotation
        let new_x = m00 * x + m01 * y + m02 * z;
        let new_y = m10 * x + m11 * y + m12 * z;
        let new_z = m20 * x + m21 * y + m22 * z;

        // Translate back and apply height
        v.x = new_x + center_x;
        v.y = new_y + center_y;
        v.z = new_z + height;

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
fn build_flasher_mesh(flasher: &Flasher) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if flasher.drag_points.len() < 3 {
        return None;
    }

    // Use accuracy = 4.0 (maximum precision)
    let accuracy = 4.0f32;
    // Flashers always loop (closed polygon)
    let vvertex = get_rg_vertex_2d(&flasher.drag_points, accuracy, true);

    if vvertex.len() < 3 {
        return None;
    }

    // Calculate bounds for texture coordinates
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

    let inv_width = if (maxx - minx).abs() > 1e-6 {
        1.0 / (maxx - minx)
    } else {
        1.0
    };
    let inv_height = if (maxy - miny).abs() > 1e-6 {
        1.0 / (maxy - miny)
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
                nz: 1.0, // Flat surface, normal pointing up
                tu: (v.x - minx) * inv_width,
                tv: (v.y - miny) * inv_height,
            }
        })
        .collect();

    // Triangulate the polygon
    let indices = polygon_to_triangles(&vvertex);

    if indices.is_empty() {
        return None;
    }

    // Calculate center for rotation
    let center_x = (minx + maxx) * 0.5;
    let center_y = (miny + maxy) * 0.5;

    // Apply rotation transformation
    apply_rotation(
        &mut vertices,
        flasher.rot_x,
        flasher.rot_y,
        flasher.rot_z,
        flasher.height,
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

    Some((wrapped, faces))
}

/// Write flasher meshes to file
pub(super) fn write_flasher_meshes(
    gameitems_dir: &Path,
    flasher: &Flasher,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_flasher_mesh(flasher) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &flasher.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
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

        let result = build_flasher_mesh(&flasher);
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 2); // 2 triangles for a quad
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

        let result = build_flasher_mesh(&flasher);
        assert!(result.is_some());

        let (vertices, _indices) = result.unwrap();
        // Verify height is not just flat at flasher.height due to rotation
        let has_varied_z = vertices
            .iter()
            .any(|v| (v.vertex.z - flasher.height).abs() > 0.01);
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

        let result = build_flasher_mesh(&flasher);
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
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

        let result = build_flasher_mesh(&flasher);
        assert!(
            result.is_some(),
            "Clockwise winding flasher should generate a mesh"
        );

        let (vertices, indices) = result.unwrap();
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 2); // 2 triangles for a quad
    }
}
