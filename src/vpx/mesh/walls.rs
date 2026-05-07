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

/// Triangulate a simple 2D polygon, mirroring vpinball's
/// `PolygonToTriangles` + `AdvancePoint` (`MeshUtils.h:496`).
///
/// The algorithm is ear-clipping with two extra checks per candidate ear:
///
/// 1. The new triangle's interior angle (`a`-`b`-`c`) must not be
///    concave (`GetDot(pv1, pv2, pv3) >= 0`).
/// 2. The new diagonal `a`-`c` must not intersect any remaining
///    polygon edge.
///
/// The standard "convex vertex + no point in triangle" check used by
/// the previous implementation rejects valid ears on some polygons
/// where an adjacent vertex sits exactly on a triangle edge or where
/// the polygon has near-collinear edges. The diagonal-intersection
/// test is the more robust criterion vpinball uses.
///
/// Mirrors `surface.cpp:622`'s call (`support_both_winding_orders =
/// false`) - we don't pre-flip the winding because vpin's wall render
/// vertices come from the same `IHaveDragPoints::GetRgVertex` port and
/// are produced in the expected CCW orientation.
fn triangulate_polygon(points: &[(f32, f32)]) -> Vec<[u32; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }

    // VPinball's algorithm expects polygons in its conventional vertex
    // order (vpinball-CCW, which is math-CW because the table Y axis
    // points down). If the input is in math-CCW order, reverse it -
    // this mirrors `support_both_winding_orders = true` in vpinball's
    // `PolygonToTriangles`.
    let mut vpoly: Vec<u32> = (0..points.len() as u32).collect();
    if signed_area(points) > 0.0 {
        vpoly.reverse();
    }
    let tricount = points.len() - 2;
    let mut triangles = Vec::with_capacity(tricount);

    for _ in 0..tricount {
        let mut found = false;
        for i in 0..vpoly.len() {
            let s = vpoly.len();
            let pre = vpoly[if i == 0 { s - 1 } else { i - 1 }];
            let a = vpoly[i];
            let b = vpoly[if i < s - 1 { i + 1 } else { 0 }];
            let c = vpoly[if i < s - 2 { i + 2 } else { i + 2 - s }];
            let post = vpoly[if i < s - 3 { i + 3 } else { i + 3 - s }];

            if !advance_point(points, &vpoly, a, b, c, pre, post) {
                continue;
            }

            // VPinball emits the triangle as (a, c, b).
            triangles.push([a, c, b]);
            vpoly.remove(if i < s - 1 { i + 1 } else { 0 });
            found = true;
            break;
        }
        if !found {
            // No valid ear in this sweep - matches vpinball, which also
            // silently skips this iteration rather than recovering.
            break;
        }
    }

    triangles
}

/// Mirrors vpinball's `AdvancePoint` (`MeshUtils.h`).
fn advance_point(
    rgv: &[(f32, f32)],
    vpoly: &[u32],
    a: u32,
    b: u32,
    c: u32,
    pre: u32,
    post: u32,
) -> bool {
    let pv1 = rgv[a as usize];
    let pv2 = rgv[b as usize];
    let pv3 = rgv[c as usize];
    let pv_pre = rgv[pre as usize];
    let pv_post = rgv[post as usize];

    if dot_2d(pv1, pv2, pv3) < 0.0
        || (dot_2d(pv_pre, pv1, pv2) > 0.0 && dot_2d(pv_pre, pv1, pv3) < 0.0)
        || (dot_2d(pv2, pv3, pv_post) > 0.0 && dot_2d(pv1, pv3, pv_post) < 0.0)
    {
        return false;
    }

    // Bounding-box of the diagonal a-c, used by vpinball as a fast
    // reject before the full segment-intersection test.
    let minx = pv1.0.min(pv3.0);
    let maxx = pv1.0.max(pv3.0);
    let miny = pv1.1.min(pv3.1);
    let maxy = pv1.1.max(pv3.1);

    for i in 0..vpoly.len() {
        let v1 = rgv[vpoly[i] as usize];
        let v2 = rgv[vpoly[if i < vpoly.len() - 1 { i + 1 } else { 0 }] as usize];

        // Skip edges that share a vertex with the diagonal.
        if v1 == pv1 || v2 == pv1 || v1 == pv3 || v2 == pv3 {
            continue;
        }
        // VPinball's bounding-box reject (with the original `pvCross2->y
        // <= maxx` typo carried verbatim - see MeshUtils.h).
        if !((v1.1 >= miny || v2.1 >= miny)
            && (v1.1 <= maxy || v2.1 <= maxy)
            && (v1.0 >= minx || v2.0 >= minx)
            && (v1.0 <= maxx || v2.1 <= maxx))
        {
            continue;
        }
        if lines_intersect(pv1, pv3, v1, v2) {
            return false;
        }
    }
    true
}

/// Mirrors vpinball's `GetDot` (`MeshUtils.h`):
/// `(joint.x - end1.x) * (joint.y - end2.y) - (joint.y - end1.y) * (joint.x - end2.x)`.
fn dot_2d(end1: (f32, f32), joint: (f32, f32), end2: (f32, f32)) -> f32 {
    (joint.0 - end1.0) * (joint.1 - end2.1) - (joint.1 - end1.1) * (joint.0 - end2.0)
}

/// Twice the signed polygon area; positive = math-CCW orientation.
fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area
}

/// Mirrors vpinball's `FLinesIntersect` (`MeshUtils.h`). Returns true
/// iff segment `(s1, s2)` intersects segment `(e1, e2)`. The collinear
/// branches mirror vpinball's choice to compare only the X coordinate.
fn lines_intersect(s1: (f32, f32), s2: (f32, f32), e1: (f32, f32), e2: (f32, f32)) -> bool {
    let (x1, y1) = s1;
    let (x2, y2) = s2;
    let (x3, y3) = e1;
    let (x4, y4) = e2;

    let d123 = (x2 - x1) * (y3 - y1) - (x3 - x1) * (y2 - y1);
    if d123 == 0.0 {
        return x3 >= x1.min(x2) && x3 <= x2.max(x1);
    }

    let d124 = (x2 - x1) * (y4 - y1) - (x4 - x1) * (y2 - y1);
    if d124 == 0.0 {
        return x4 >= x1.min(x2) && x4 <= x2.max(x1);
    }

    if d123 * d124 >= 0.0 {
        return false;
    }

    let d341 = (x3 - x1) * (y4 - y1) - (x4 - x1) * (y3 - y1);
    if d341 == 0.0 {
        return x1 >= x3.min(x4) && x1 <= x3.max(x4);
    }

    let d342 = d123 - d124 + d341;
    if d342 == 0.0 {
        return x2 >= x3.min(x4) && x2 <= x3.max(x4);
    }

    d341 * d342 < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: walk all triangles, accumulate signed area, return its
    /// absolute value. Should equal the absolute polygon area when the
    /// triangulation is complete and non-overlapping.
    fn triangulated_area(points: &[(f32, f32)], tris: &[[u32; 3]]) -> f32 {
        let mut a = 0.0_f32;
        for tri in tris {
            let p0 = points[tri[0] as usize];
            let p1 = points[tri[1] as usize];
            let p2 = points[tri[2] as usize];
            a += ((p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1)).abs() * 0.5;
        }
        a
    }

    fn polygon_area(points: &[(f32, f32)]) -> f32 {
        signed_area(points).abs() * 0.5
    }

    #[test]
    fn triangulates_square() {
        // CCW square (math convention). The algorithm auto-flips to
        // vpinball-CCW (= math-CW) and produces 2 triangles.
        let points = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let tris = triangulate_polygon(&points);
        assert_eq!(tris.len(), 2);
        assert!((triangulated_area(&points, &tris) - polygon_area(&points)).abs() < 1e-5);
    }

    #[test]
    fn triangulates_clockwise_square_too() {
        // CW square (vpinball-native winding). No flip applied; should
        // still produce 2 triangles.
        let points = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];
        let tris = triangulate_polygon(&points);
        assert_eq!(tris.len(), 2);
        assert!((triangulated_area(&points, &tris) - polygon_area(&points)).abs() < 1e-5);
    }

    #[test]
    fn triangulates_concave_l_shape() {
        // L-shape (concave). The previous "convex ear + point in
        // triangle" algorithm could mis-classify ears here; the
        // vpinball-faithful AdvancePoint port should always produce
        // n - 2 = 4 triangles covering the full area.
        //
        //   (0,2)---(1,2)
        //     |       |
        //     |       |   (CCW)
        //     |       (1,1)----(2,1)
        //     |                  |
        //   (0,0)--------------(2,0)
        let points = vec![
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ];
        let tris = triangulate_polygon(&points);
        assert_eq!(tris.len(), points.len() - 2);
        assert!(
            (triangulated_area(&points, &tris) - polygon_area(&points)).abs() < 1e-5,
            "triangulated area {} != polygon area {}",
            triangulated_area(&points, &tris),
            polygon_area(&points)
        );
    }

    #[test]
    fn triangulates_convex_n_gon() {
        // Regular 13-gon - same vertex count as the Wall64 top mesh
        // that surfaced the gap originally. Fan-triangulation should
        // produce n - 2 = 11 triangles.
        const N: usize = 13;
        let points: Vec<(f32, f32)> = (0..N)
            .map(|i| {
                let t = (i as f32 / N as f32) * std::f32::consts::TAU;
                (t.cos(), t.sin())
            })
            .collect();
        let tris = triangulate_polygon(&points);
        assert_eq!(tris.len(), N - 2);
        assert!((triangulated_area(&points, &tris) - polygon_area(&points)).abs() < 1e-3);
    }

    #[test]
    fn rejects_degenerate_polygons() {
        assert!(triangulate_polygon(&[]).is_empty());
        assert!(triangulate_polygon(&[(0.0, 0.0)]).is_empty());
        assert!(triangulate_polygon(&[(0.0, 0.0), (1.0, 0.0)]).is_empty());
    }

    #[test]
    fn lines_intersect_basic_cross() {
        // Two segments that clearly cross.
        assert!(lines_intersect(
            (0.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (1.0, 0.0)
        ));
        // Parallel non-overlapping.
        assert!(!lines_intersect(
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (1.0, 1.0)
        ));
        // Note: vpinball's `FLinesIntersect` does NOT special-case
        // shared endpoints - the caller (`AdvancePoint`) skips edges
        // sharing a vertex with the diagonal before invoking this. We
        // mirror that, so behavior on shared endpoints is unspecified.
    }
}
