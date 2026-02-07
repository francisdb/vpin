//! Wall mesh generation for expanded VPX export

use super::mesh_common::{Vec2, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::wall::Wall;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::path::Path;

struct RenderVertex {
    x: f32,
    y: f32,
    smooth: bool,
    has_auto_texture: bool,
    tex_coord: f32,
}

pub(super) fn write_wall_meshes(
    gameitems_dir: &Path,
    wall: &Wall,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_wall_mesh(wall) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(&mesh_path, &wall.name, &vertices, &indices, mesh_format, fs)
}

fn build_wall_mesh(wall: &Wall) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    let render_vertices = build_render_vertices(wall);
    if render_vertices.len() < 3 {
        return None;
    }

    let texture_coords = if wall.side_image.is_empty() {
        None
    } else {
        Some(compute_side_texture_coords(&render_vertices))
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
    build_top_mesh(&render_vertices, wall, &mut top_vertices, &mut top_indices);

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
        return None;
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

fn build_render_vertices(wall: &Wall) -> Vec<RenderVertex> {
    wall.drag_points
        .iter()
        .map(|point| RenderVertex {
            x: point.x,
            y: point.y,
            smooth: point.smooth,
            has_auto_texture: point.has_auto_texture,
            tex_coord: point.tex_coord,
        })
        .collect()
}

fn compute_side_texture_coords(points: &[RenderVertex]) -> Vec<f32> {
    let mut cumulative = Vec::with_capacity(points.len());
    let mut total = 0.0f32;
    cumulative.push(0.0);
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        total += (dx * dx + dy * dy).sqrt();
        cumulative.push(total);
    }

    if points.len() > 1 {
        let dx = points[0].x - points[points.len() - 1].x;
        let dy = points[0].y - points[points.len() - 1].y;
        total += (dx * dx + dy * dy).sqrt();
    }

    let inv_total = if total == 0.0 { 0.0 } else { 1.0 / total };

    points
        .iter()
        .zip(cumulative.into_iter())
        .map(|(point, dist)| {
            if point.has_auto_texture {
                dist * inv_total
            } else {
                point.tex_coord
            }
        })
        .collect()
}

fn build_side_mesh(
    wall: &Wall,
    points: &[RenderVertex],
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

fn build_top_mesh(
    points: &[RenderVertex],
    wall: &Wall,
    out_vertices: &mut Vec<Vertex3dNoTex2>,
    out_indices: &mut Vec<u32>,
) {
    let coords = points.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>();
    let triangles = triangulate_polygon(&coords);

    out_indices.clear();
    out_indices.extend(triangles.into_iter().flat_map(|tri| tri));

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

fn bounds_xy(points: &[RenderVertex]) -> (f32, f32, f32, f32) {
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
