//! Wall mesh generation for expanded VPX export
//!
//! This module ports the wall mesh generation from Visual Pinball's surface.cpp.
//! Walls are represented as extruded polygons with optional smoothing and texture coordinates.

use crate::vpx::TableDimensions;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::wall::Wall;
use crate::vpx::math::Vec2;
use crate::vpx::mesh::{RenderVertex2D, get_rg_vertex_2d};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

/// Separate wall meshes for top and sides, each with their own material/texture
pub(crate) struct WallMeshes {
    /// Top surface mesh (may be None if not visible)
    pub top: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// Side surface mesh (may be None if not visible)
    pub side: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
}

/// Build separate wall meshes for top and sides
/// This is useful for GLTF export where each surface can have its own material/texture
///
/// `table_dims` is required for proper UV calculation - wall top textures use table-space UVs
pub(crate) fn build_wall_meshes(wall: &Wall, table_dims: &TableDimensions) -> Option<WallMeshes> {
    let render_vertices = build_render_vertices(wall);
    if render_vertices.len() < 3 {
        return None;
    }

    let texture_coords = if wall.side_image.is_empty() && wall.image.is_empty() {
        None
    } else {
        Some(compute_side_texture_coords(
            &render_vertices,
            &wall.drag_points,
        ))
    };

    // Build side mesh
    let side_mesh = {
        let mut side_vertices = Vec::new();
        let mut side_indices = Vec::new();
        build_side_mesh(
            wall,
            &render_vertices,
            texture_coords.as_ref(),
            &mut side_vertices,
            &mut side_indices,
        );

        if !side_vertices.is_empty() {
            let wrapped = side_vertices
                .into_iter()
                .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
                .collect::<Vec<_>>();
            let faces = side_indices
                .chunks_exact(3)
                .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
                .collect::<Vec<_>>();
            Some((wrapped, faces))
        } else {
            None
        }
    };

    // Build top mesh
    let top_mesh = {
        let mut top_vertices = Vec::new();
        let mut top_indices = Vec::new();
        build_top_mesh(
            &render_vertices,
            wall,
            table_dims,
            &mut top_vertices,
            &mut top_indices,
        );

        if !top_indices.is_empty() {
            let wrapped = top_vertices
                .into_iter()
                .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
                .collect::<Vec<_>>();
            let faces = top_indices
                .chunks_exact(3)
                .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
                .collect::<Vec<_>>();
            Some((wrapped, faces))
        } else {
            None
        }
    };

    // Return None only if both meshes are empty
    if side_mesh.is_none() && top_mesh.is_none() {
        return None;
    }

    Some(WallMeshes {
        top: top_mesh,
        side: side_mesh,
    })
}

pub(crate) fn build_wall_mesh(wall: &Wall) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    let render_vertices = build_render_vertices(wall);
    if render_vertices.len() < 3 {
        return None;
    }

    let texture_coords = if wall.side_image.is_empty() {
        None
    } else {
        Some(compute_side_texture_coords(
            &render_vertices,
            &wall.drag_points,
        ))
    };

    let mut side_vertices = Vec::new();
    let mut side_indices = Vec::new();
    build_side_mesh(
        wall,
        &render_vertices,
        texture_coords.as_ref(),
        &mut side_vertices,
        &mut side_indices,
    );

    let mut top_vertices = Vec::new();
    let mut top_indices = Vec::new();
    build_top_mesh_item_space(&render_vertices, wall, &mut top_vertices, &mut top_indices);

    if top_indices.is_empty() {
        return None;
    }

    let (vertices, indices) = if wall.is_top_bottom_visible && wall.is_side_visible {
        let side_count = side_vertices.len();
        let mut vertices = side_vertices;
        vertices.extend(top_vertices);

        let mut indices = side_indices;
        indices.extend(top_indices.into_iter().map(|idx| idx + side_count as u32));
        (vertices, indices)
    } else if wall.is_top_bottom_visible {
        (top_vertices, top_indices)
    } else if wall.is_side_visible {
        (side_vertices, side_indices)
    } else {
        // Generate mesh even when invisible (useful for tools that need to process all geometry)
        // Include both top and sides
        let side_count = side_vertices.len();
        let mut vertices = side_vertices;
        vertices.extend(top_vertices);

        let mut indices = side_indices;
        indices.extend(top_indices.into_iter().map(|idx| idx + side_count as u32));
        (vertices, indices)
    };

    let wrapped = vertices
        .into_iter()
        .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
        .collect::<Vec<_>>();

    let faces = indices
        .chunks_exact(3)
        .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
        .collect::<Vec<_>>();

    Some((wrapped, faces))
}

/// Build smoothed render vertices from wall drag points using Catmull-Rom spline interpolation.
/// Walls are always closed loops (like rubbers) with maximum accuracy (4.0).
/// This matches VPinball's `GetRgVertex(vvertex)` call in surface.cpp.
fn build_render_vertices(wall: &Wall) -> Vec<RenderVertex2D> {
    // Walls always loop, accuracy = 4.0 (maximum precision, matching VPinball default)
    get_rg_vertex_2d(&wall.drag_points, 4.0, true)
}

/// Compute side texture coordinates for smoothed wall vertices.
///
/// This matches VPinball's `IHaveDragPoints::GetTextureCoords` from dragpoint.cpp.
/// It finds original drag points within the smoothed vertex list using the
/// `control_point` flag, then interpolates texture coordinates between them
/// proportionally by edge length.
fn compute_side_texture_coords(
    points: &[RenderVertex2D],
    drag_points: &[crate::vpx::gameitem::dragpoint::DragPoint],
) -> Vec<f32> {
    let cpoints = points.len();
    let mut coords = vec![0.0f32; cpoints];

    // Find control points that have manual texture coordinates
    let mut tex_points: Vec<usize> = Vec::new(); // indices into drag_points
    let mut render_points: Vec<usize> = Vec::new(); // corresponding indices into points
    let mut icontrol = 0usize;
    let mut no_coords = false;

    for (i, rv) in points.iter().enumerate() {
        if rv.control_point {
            if icontrol < drag_points.len() && !drag_points[icontrol].has_auto_texture {
                tex_points.push(icontrol);
                render_points.push(i);
            }
            icontrol += 1;
        }
    }

    if tex_points.is_empty() {
        // No manual texture coordinates — auto-generate from point 0
        tex_points.push(0);
        render_points.push(0);
        no_coords = true;
    }

    // Wrap around to cover the last section
    tex_points.push(tex_points[0] + drag_points.len());
    render_points.push(render_points[0] + cpoints);

    for i in 0..tex_points.len() - 1 {
        let start_render = render_points[i] % cpoints;
        let mut end_render = render_points[i + 1] % cpoints;

        let (start_tex, end_tex) = if no_coords {
            (0.0f32, 1.0f32)
        } else {
            let st = drag_points[tex_points[i] % drag_points.len()].tex_coord;
            let et = drag_points[tex_points[i + 1] % drag_points.len()].tex_coord;
            (st, et)
        };

        let delta_coord = end_tex - start_tex;

        if end_render <= start_render {
            end_render += cpoints;
        }

        // Compute total length of this section
        let mut total_length = 0.0f32;
        for l in start_render..end_render {
            let p1 = &points[l % cpoints];
            let p2 = &points[(l + 1) % cpoints];
            let dx = p2.x - p1.x;
            let dy = p2.y - p1.y;
            total_length += (dx * dx + dy * dy).sqrt();
        }

        // Assign texture coordinates proportionally by edge length
        let mut cur_length = 0.0f32;
        for l in start_render..end_render {
            let frac = if total_length > 0.0 {
                cur_length / total_length
            } else {
                0.0
            };
            coords[l % cpoints] = start_tex + frac * delta_coord;

            let p1 = &points[l % cpoints];
            let p2 = &points[(l + 1) % cpoints];
            let dx = p2.x - p1.x;
            let dy = p2.y - p1.y;
            cur_length += (dx * dx + dy * dy).sqrt();
        }
    }

    coords
}

fn build_side_mesh(
    wall: &Wall,
    points: &[RenderVertex2D],
    texture_coords: Option<&Vec<f32>>,
    out_vertices: &mut Vec<Vertex3dNoTex2>,
    out_indices: &mut Vec<u32>,
) {
    let count = points.len();
    let mut edge_normals = Vec::with_capacity(count);

    for i in 0..count {
        let next = if i + 1 < count { i + 1 } else { 0 };
        let dx = points[i].x - points[next].x;
        let dy = points[i].y - points[next].y;
        let inv_len = (dx * dx + dy * dy).sqrt();
        if inv_len == 0.0 {
            edge_normals.push(Vec2 { x: 0.0, y: 0.0 });
        } else {
            edge_normals.push(Vec2 {
                x: dy / inv_len,
                y: dx / inv_len,
            });
        }
    }

    out_vertices.clear();
    out_vertices.resize(
        count * 4,
        Vertex3dNoTex2 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            nx: 0.0,
            ny: 0.0,
            nz: 0.0,
            tu: 0.0,
            tv: 0.0,
        },
    );

    let bottom = wall.height_bottom;
    let top = wall.height_top;

    for i in 0..count {
        let next = if i + 1 < count { i + 1 } else { 0 };
        let prev = if i == 0 { count - 1 } else { i - 1 };

        let vnormal0 = if points[i].smooth {
            Vec2 {
                x: edge_normals[prev].x + edge_normals[i].x,
                y: edge_normals[prev].y + edge_normals[i].y,
            }
        } else {
            edge_normals[i]
        }
        .normalize();

        let vnormal1 = if points[next].smooth {
            Vec2 {
                x: edge_normals[i].x + edge_normals[next].x,
                y: edge_normals[i].y + edge_normals[next].y,
            }
        } else {
            edge_normals[i]
        }
        .normalize();

        let offset = i * 4;
        let pv1 = &points[i];
        let pv2 = &points[next];

        out_vertices[offset].x = pv1.x;
        out_vertices[offset].y = pv1.y;
        out_vertices[offset].z = bottom;

        out_vertices[offset + 1].x = pv1.x;
        out_vertices[offset + 1].y = pv1.y;
        out_vertices[offset + 1].z = top;

        out_vertices[offset + 2].x = pv2.x;
        out_vertices[offset + 2].y = pv2.y;
        out_vertices[offset + 2].z = top;

        out_vertices[offset + 3].x = pv2.x;
        out_vertices[offset + 3].y = pv2.y;
        out_vertices[offset + 3].z = bottom;

        if let Some(coords) = texture_coords {
            out_vertices[offset].tu = coords[i];
            out_vertices[offset].tv = 1.0;

            out_vertices[offset + 1].tu = coords[i];
            out_vertices[offset + 1].tv = 0.0;

            out_vertices[offset + 2].tu = coords[next];
            out_vertices[offset + 2].tv = 0.0;

            out_vertices[offset + 3].tu = coords[next];
            out_vertices[offset + 3].tv = 1.0;
        }

        out_vertices[offset].nx = vnormal0.x;
        out_vertices[offset].ny = -vnormal0.y;
        out_vertices[offset].nz = 0.0;

        out_vertices[offset + 1].nx = vnormal0.x;
        out_vertices[offset + 1].ny = -vnormal0.y;
        out_vertices[offset + 1].nz = 0.0;

        out_vertices[offset + 2].nx = vnormal1.x;
        out_vertices[offset + 2].ny = -vnormal1.y;
        out_vertices[offset + 2].nz = 0.0;

        out_vertices[offset + 3].nx = vnormal1.x;
        out_vertices[offset + 3].ny = -vnormal1.y;
        out_vertices[offset + 3].nz = 0.0;
    }

    out_indices.clear();
    out_indices.reserve(count * 6);
    for i in 0..count {
        let offset = (i * 4) as u32;
        out_indices.extend_from_slice(&[
            offset,
            offset + 1,
            offset + 2,
            offset,
            offset + 2,
            offset + 3,
        ]);
    }
}

/// Build top mesh with item-space UVs (normalized to wall bounding box)
/// Used for single item mesh export
fn build_top_mesh_item_space(
    points: &[RenderVertex2D],
    wall: &Wall,
    out_vertices: &mut Vec<Vertex3dNoTex2>,
    out_indices: &mut Vec<u32>,
) {
    let coords = points.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>();
    let triangles = triangulate_polygon(&coords);

    out_indices.clear();
    out_indices.extend(triangles.into_iter().flatten());

    let (min_x, max_x, min_y, max_y) = bounds_xy(points);
    let inv_w = if max_x == min_x {
        0.0
    } else {
        1.0 / (max_x - min_x)
    };
    let inv_h = if max_y == min_y {
        0.0
    } else {
        1.0 / (max_y - min_y)
    };

    out_vertices.clear();
    out_vertices.reserve(points.len());
    for point in points {
        out_vertices.push(Vertex3dNoTex2 {
            x: point.x,
            y: point.y,
            z: wall.height_top,
            tu: (point.x - min_x) * inv_w,
            tv: (point.y - min_y) * inv_h,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
        });
    }
}

fn bounds_xy(points: &[RenderVertex2D]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    (min_x, max_x, min_y, max_y)
}

/// Build top mesh with table-space UVs (normalized to table dimensions)
/// Used for full table GLTF export where textures span the entire playfield
fn build_top_mesh(
    points: &[RenderVertex2D],
    wall: &Wall,
    table_dims: &TableDimensions,
    out_vertices: &mut Vec<Vertex3dNoTex2>,
    out_indices: &mut Vec<u32>,
) {
    let coords = points.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>();
    let triangles = triangulate_polygon(&coords);

    out_indices.clear();
    out_indices.extend(triangles.into_iter().flatten());

    // VPinball uses table-space UV coordinates for wall tops
    // See surface.cpp: tu = pv0->x * inv_tablewidth, tv = pv0->y * inv_tableheight
    let table_width = table_dims.right - table_dims.left;
    let table_height = table_dims.bottom - table_dims.top;
    let inv_table_width = if table_width == 0.0 {
        0.0
    } else {
        1.0 / table_width
    };
    let inv_table_height = if table_height == 0.0 {
        0.0
    } else {
        1.0 / table_height
    };

    out_vertices.clear();
    out_vertices.reserve(points.len());
    for point in points {
        out_vertices.push(Vertex3dNoTex2 {
            x: point.x,
            y: point.y,
            z: wall.height_top,
            // Table-space UV coordinates (matching VPinball behavior)
            tu: (point.x - table_dims.left) * inv_table_width,
            tv: (point.y - table_dims.top) * inv_table_height,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
        });
    }
}

fn triangulate_polygon(points: &[(f32, f32)]) -> Vec<[u32; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }

    let mut indices = (0..points.len()).collect::<Vec<_>>();
    if signed_area(points) < 0.0 {
        indices.reverse();
    }

    let mut triangles = Vec::new();
    let mut guard = 0usize;
    while indices.len() >= 3 && guard < points.len() * points.len() {
        guard += 1;
        let mut ear_found = false;
        for i in 0..indices.len() {
            let prev = indices[(i + indices.len() - 1) % indices.len()];
            let curr = indices[i];
            let next = indices[(i + 1) % indices.len()];

            if !is_convex(points[prev], points[curr], points[next]) {
                continue;
            }

            let mut contains = false;
            for &other in &indices {
                if other == prev || other == curr || other == next {
                    continue;
                }
                if point_in_triangle(points[other], points[prev], points[curr], points[next]) {
                    contains = true;
                    break;
                }
            }

            if contains {
                continue;
            }

            triangles.push([prev as u32, curr as u32, next as u32]);
            indices.remove(i);
            ear_found = true;
            break;
        }

        if !ear_found {
            break;
        }
    }

    triangles
}

fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area * 0.5
}

fn is_convex(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let bcx = c.0 - b.0;
    let bcy = c.1 - b.1;
    (abx * bcy - aby * bcx) > 0.0
}

fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let v0x = c.0 - a.0;
    let v0y = c.1 - a.1;
    let v1x = b.0 - a.0;
    let v1y = b.1 - a.1;
    let v2x = p.0 - a.0;
    let v2y = p.1 - a.1;

    let dot00 = v0x * v0x + v0y * v0y;
    let dot01 = v0x * v1x + v0y * v1y;
    let dot02 = v0x * v2x + v0y * v2y;
    let dot11 = v1x * v1x + v1y * v1y;
    let dot12 = v1x * v2x + v1y * v2y;

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom == 0.0 {
        return false;
    }
    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= 0.0 && v >= 0.0 && (u + v) <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulates_square() {
        let points = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let tris = triangulate_polygon(&points);
        assert_eq!(tris.len(), 2);
    }
}
