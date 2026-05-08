//! Procedural mesh for vpinball primitives that don't load a `.obj`
//! file (`use_3d_mesh = false`). Driven by two fields on the
//! primitive: `sides` (number of polygon sides on the top/bottom caps)
//! and `draw_textures_inside` (whether to emit back-faces too).
//!
//! Ported 1:1 from VPinball's `Primitive::CalculateBuiltinOriginal`
//! (`src/parts/primitive.cpp:648-836`). The generated mesh fits in the
//! unit cube `[-r, r] x [-r, r] x [-0.5, 0.5]` with `r` derived from
//! the sides count, ready to be scaled / rotated / translated by the
//! primitive's transform.
//!
//! Vertex layout (`4 * sides + 2` total):
//!
//! ```text
//! 0                   : top centre        (z=+0.5, n=+Z)
//! 1 .. sides+1        : top edge ring     (z=+0.5, n=+Z)
//! sides+1             : bottom centre     (z=-0.5, n=-Z)
//! sides+2 .. 2*sides+2: bottom edge ring  (z=-0.5, n=-Z)
//! 2*sides+2 .. 3*sides+2: side top ring   (z=+0.5, n=outward)
//! 3*sides+2 .. 4*sides+2: side bottom ring (z=-0.5, n=outward)
//! ```
//!
//! Indices are `12 * sides` for outside-only (`draw_textures_inside =
//! false`) or `24 * sides` when both cullings are emitted.

use crate::vpx::expanded::WriteError;
use crate::vpx::gameitem::primitive::{Primitive, ReadMesh, VertexWrapper};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use log::warn;
use std::f32::consts::PI;

/// Resolve the mesh to render/export for a primitive, choosing
/// between the loaded mesh ([`Primitive::read_mesh`]) and the
/// procedural builtin form ([`build_builtin_primitive_mesh`]) based
/// on [`Primitive::use_3d_mesh`].
///
/// Lives here rather than on `impl Primitive` so the dependency
/// stays one-way (`mesh` -> `gameitem`), matching the other
/// `mesh::*` builders that take `&Bumper` / `&Gate` / etc.
///
/// Logs warnings for the two inconsistent states:
///
/// - `use_3d_mesh = true` but no `compressed_vertices_data` present
///   (corrupt VPX, M3CX chunk missing) - returns `None`.
/// - `use_3d_mesh = false` but stale mesh data is still in the
///   M3CX chunk - emits the builtin mesh and ignores the stale
///   data (matches what vpinball does at runtime).
///
/// Returns `None` only when no mesh can be produced; primitives
/// with `sides < 3` also return `None` since the builtin generator
/// declines those (vpinball's editor clamps to 3+).
pub fn effective_primitive_mesh(primitive: &Primitive) -> Result<Option<ReadMesh>, WriteError> {
    if primitive.use_3d_mesh {
        match primitive.read_mesh()? {
            Some(mesh) => Ok(Some(mesh)),
            None => {
                warn!(
                    "Primitive '{}' has use_3d_mesh=true but no mesh data; skipping",
                    primitive.name
                );
                Ok(None)
            }
        }
    } else {
        if primitive.compressed_vertices_data.is_some() {
            warn!(
                "Primitive '{}' has use_3d_mesh=false but stale mesh data is present; using builtin",
                primitive.name
            );
        }
        match build_builtin_primitive_mesh(primitive.sides, primitive.draw_textures_inside) {
            Some((vertices, indices)) => Ok(Some(ReadMesh { vertices, indices })),
            None => {
                warn!(
                    "Primitive '{}' has sides={} (< 3); cannot generate builtin mesh",
                    primitive.name, primitive.sides
                );
                Ok(None)
            }
        }
    }
}

/// Build the procedural mesh used when a primitive has
/// `use_3d_mesh = false`. Returns `None` if `sides < 3` (degenerate -
/// vpinball clamps to 3 in the editor; the function declines rather
/// than producing a malformed mesh).
pub fn build_builtin_primitive_mesh(
    sides: u32,
    draw_textures_inside: bool,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if sides < 3 {
        return None;
    }
    let n = sides as usize;

    let outer_radius = -0.5 / (PI / sides as f32).cos();
    let add_angle = 2.0 * PI / sides as f32;
    let offs_angle = PI / sides as f32;

    // 4 * sides + 2 vertices: top centre, top ring, bottom centre,
    // bottom ring, side top ring, side bottom ring.
    let zero = Vertex3dNoTex2 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        nx: 0.0,
        ny: 0.0,
        nz: 0.0,
        tu: 0.0,
        tv: 0.0,
    };
    let mut verts: Vec<Vertex3dNoTex2> = vec![zero; 4 * n + 2];

    // Top centre.
    verts[0] = Vertex3dNoTex2 {
        x: 0.0,
        y: 0.0,
        z: 0.5,
        nx: 0.0,
        ny: 0.0,
        nz: 1.0,
        tu: 0.25,
        tv: 0.25,
    };
    // Bottom centre at index `n + 1`.
    verts[n + 1] = Vertex3dNoTex2 {
        x: 0.0,
        y: 0.0,
        z: -0.5,
        nx: 0.0,
        ny: 0.0,
        nz: -1.0,
        tu: 0.75,
        tv: 0.25,
    };

    // First pass: positions + normals on the four rings, plus bbox
    // for top/bottom UV mapping.
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for i in 0..n {
        let current_angle = add_angle * i as f32 + offs_angle;
        let sx = current_angle.sin() * outer_radius;
        let cy = current_angle.cos() * outer_radius;

        // Top ring point (cap normal = +Z).
        verts[i + 1] = Vertex3dNoTex2 {
            x: sx,
            y: cy,
            z: 0.5,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
            tu: 0.0,
            tv: 0.0,
        };
        // Bottom ring point (cap normal = -Z).
        verts[i + 1 + n + 1] = Vertex3dNoTex2 {
            x: sx,
            y: cy,
            z: -0.5,
            nx: 0.0,
            ny: 0.0,
            nz: -1.0,
            tu: 0.0,
            tv: 0.0,
        };
        // Side top ring (outward-facing normal in XY plane).
        verts[2 * n + 2 + i] = Vertex3dNoTex2 {
            x: sx,
            y: cy,
            z: 0.5,
            nx: current_angle.sin(),
            ny: current_angle.cos(),
            nz: 0.0,
            tu: 0.0,
            tv: 0.0,
        };
        // Side bottom ring.
        verts[3 * n + 2 + i] = Vertex3dNoTex2 {
            x: sx,
            y: cy,
            z: -0.5,
            nx: current_angle.sin(),
            ny: current_angle.cos(),
            nz: 0.0,
            tu: 0.0,
            tv: 0.0,
        };

        if sx < min_x {
            min_x = sx;
        }
        if sx > max_x {
            max_x = sx;
        }
        if cy < min_y {
            min_y = cy;
        }
        if cy > max_y {
            max_y = cy;
        }
    }

    // Second pass: UVs on the four rings now that we have the bbox.
    let inv_x = 0.5 / (max_x - min_x);
    let inv_y = 0.5 / (max_y - min_y);
    let inv_s = 1.0 / sides as f32;
    for i in 0..n {
        let top_tu = (verts[i + 1].x - min_x) * inv_x;
        let top_tv = (verts[i + 1].y - min_y) * inv_y;
        verts[i + 1].tu = top_tu;
        verts[i + 1].tv = top_tv;
        verts[i + 1 + n + 1].tu = top_tu + 0.5;
        verts[i + 1 + n + 1].tv = top_tv;

        let side_tu = i as f32 * inv_s;
        verts[2 * n + 2 + i].tu = side_tu;
        verts[2 * n + 2 + i].tv = 0.5;
        verts[3 * n + 2 + i].tu = side_tu;
        verts[3 * n + 2 + i].tv = 1.0;
    }

    // Wrap the raw vertices into VertexWrapper. The encoded-vertex
    // bytes are only used to round-trip floats to/from the on-disk
    // VPX exactly; for a synthesised mesh we leave them zeroed.
    let vertices: Vec<VertexWrapper> = verts
        .into_iter()
        .map(|v| VertexWrapper::new([0u8; 32], v))
        .collect();

    let mut indices: Vec<VpxFace> = if draw_textures_inside {
        Vec::with_capacity(8 * n)
    } else {
        Vec::with_capacity(4 * n)
    };

    // Indices follow vpinball's `CalculateBuiltinOriginal` layout. The
    // C++ writes raw `unsigned int`s; we group by triangle since our
    // face type is already `(i0, i1, i2)`.
    for i in 0..n {
        // `tmp` wraps the ring back to vertex 1 when we're on the last
        // side; matches vpinball's `(i == m_Sides - 1) ? 1 : (i + 2)`.
        let tmp = if i == n - 1 { 1 } else { i + 2 };
        let tmp2 = tmp + 1;

        // Top cap triangle: centre -> tmp -> i+1 (CCW from above).
        indices.push(VpxFace::new(0, tmp as i64, (i + 1) as i64));
        // Bottom cap triangle: centre -> i+sides+2 -> tmp2+sides
        // (CCW from below, i.e. outward = -Z).
        indices.push(VpxFace::new(
            (n + 1) as i64,
            (n + 2 + i) as i64,
            (n + tmp2) as i64,
        ));
        // Side quad (two triangles, outward-facing).
        let a = (2 * n + tmp2) as i64;
        let b = (3 * n + 2 + i) as i64;
        let c = (2 * n + 2 + i) as i64;
        let d = (3 * n + tmp2) as i64;
        indices.push(VpxFace::new(a, b, c));
        indices.push(VpxFace::new(a, d, b));

        if draw_textures_inside {
            // Mirrored back-faces with reversed winding.
            indices.push(VpxFace::new(0, (i + 1) as i64, tmp as i64));
            indices.push(VpxFace::new(
                (n + 1) as i64,
                (n + tmp2) as i64,
                (n + 2 + i) as i64,
            ));
            indices.push(VpxFace::new(a, c, b));
            indices.push(VpxFace::new(a, b, d));
        }
    }

    Some((vertices, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fewer_than_three_sides() {
        assert!(build_builtin_primitive_mesh(0, false).is_none());
        assert!(build_builtin_primitive_mesh(1, false).is_none());
        assert!(build_builtin_primitive_mesh(2, false).is_none());
    }

    #[test]
    fn vertex_and_face_counts_match_vpinball_layout() {
        // Outside-only: 4 triangles per side (top + bottom + 2 side
        // quad halves) = 4 faces * sides.
        let (verts, faces) = build_builtin_primitive_mesh(4, false).unwrap();
        assert_eq!(verts.len(), 4 * 4 + 2);
        assert_eq!(faces.len(), 4 * 4);

        // draw_textures_inside doubles the face count (back-faces too).
        let (verts, faces) = build_builtin_primitive_mesh(4, true).unwrap();
        assert_eq!(verts.len(), 4 * 4 + 2);
        assert_eq!(faces.len(), 8 * 4);

        // 8-sided with no back-faces: 18 verts, 32 faces.
        let (verts, faces) = build_builtin_primitive_mesh(8, false).unwrap();
        assert_eq!(verts.len(), 4 * 8 + 2);
        assert_eq!(faces.len(), 4 * 8);
    }

    #[test]
    fn caps_and_centres_have_expected_positions_and_normals() {
        let (verts, _) = build_builtin_primitive_mesh(4, false).unwrap();
        let n = 4;

        // Top centre: (0, 0, +0.5), normal +Z, UV (0.25, 0.25).
        assert_eq!(verts[0].vertex.x, 0.0);
        assert_eq!(verts[0].vertex.y, 0.0);
        assert_eq!(verts[0].vertex.z, 0.5);
        assert_eq!(verts[0].vertex.nz, 1.0);
        assert_eq!(verts[0].vertex.tu, 0.25);
        assert_eq!(verts[0].vertex.tv, 0.25);

        // Bottom centre at n+1: (0, 0, -0.5), normal -Z, UV (0.75, 0.25).
        let bc = &verts[n + 1].vertex;
        assert_eq!(bc.x, 0.0);
        assert_eq!(bc.y, 0.0);
        assert_eq!(bc.z, -0.5);
        assert_eq!(bc.nz, -1.0);
        assert_eq!(bc.tu, 0.75);
        assert_eq!(bc.tv, 0.25);
    }

    #[test]
    fn ring_points_lie_on_outer_radius_circle() {
        // Vpinball's `outerRadius = -0.5 / cos(pi / sides)`. The 4
        // top ring points should all share `|(x, y)| == |outerRadius|`.
        let sides = 6;
        let (verts, _) = build_builtin_primitive_mesh(sides, false).unwrap();
        let expected_r = (-0.5 / (PI / sides as f32).cos()).abs();
        for i in 0..sides as usize {
            let v = &verts[i + 1].vertex;
            let r = (v.x * v.x + v.y * v.y).sqrt();
            assert!(
                (r - expected_r).abs() < 1e-5,
                "ring vertex {i}: r={r}, expected {expected_r}",
            );
            assert_eq!(v.z, 0.5);
        }
    }

    #[test]
    fn side_normals_point_outward_in_xy_plane() {
        // For each side ring point, the normal's XY projection should
        // align with the position's XY projection (both roughly
        // pointing away from origin) and Z should be 0.
        let sides = 8;
        let (verts, _) = build_builtin_primitive_mesh(sides, false).unwrap();
        let n = sides as usize;
        for i in 0..n {
            let v = &verts[2 * n + 2 + i].vertex;
            assert_eq!(v.nz, 0.0, "side normal {i} should have nz=0");
            // Normal magnitude should be 1 (sin^2 + cos^2 = 1).
            let nlen = (v.nx * v.nx + v.ny * v.ny).sqrt();
            assert!(
                (nlen - 1.0).abs() < 1e-5,
                "side normal {i} magnitude {nlen}",
            );
        }
    }

    #[test]
    fn triangle_indices_reference_valid_vertices() {
        for &(sides, inside) in &[(3u32, false), (4, false), (8, false), (4, true)] {
            let (verts, faces) = build_builtin_primitive_mesh(sides, inside).unwrap();
            let n_verts = verts.len() as i64;
            for (idx, face) in faces.iter().enumerate() {
                for &i in &[face.i0, face.i1, face.i2] {
                    assert!(
                        i >= 0 && i < n_verts,
                        "sides={sides} inside={inside} face {idx}: index {i} out of [0,{n_verts})",
                    );
                }
            }
        }
    }
}
