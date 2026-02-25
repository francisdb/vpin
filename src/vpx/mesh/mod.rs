//! Mesh generation

pub(crate) mod balls;
pub(crate) mod bumpers;
pub(crate) mod decals;
pub(crate) mod flashers;
pub(crate) mod flippers;
pub(crate) mod gates;
pub(crate) mod hittargets;
pub(crate) mod kickers;
pub(crate) mod lights;
pub(crate) mod mesh_validation;
pub(crate) mod playfields;
pub(crate) mod plungers;
pub(crate) mod ramps;
pub(crate) mod rubbers;
pub(crate) mod spinners;
pub(crate) mod triggers;
pub(crate) mod walls;

/// Static detail level used by VPinball to approximate ramps and rubbers for physics/collision code.
/// From VPinball physconst.h: `#define HIT_SHAPE_DETAIL_LEVEL 7.0f`
///
/// This is a lower detail level than visual rendering (which uses 10.0) to improve
/// physics performance while maintaining adequate collision accuracy.
#[allow(dead_code)]
pub const HIT_SHAPE_DETAIL_LEVEL: f32 = 7.0;

/// Convert a detail level (0-10) to an accuracy value for spline subdivision.
///
/// From VPinball rubber.cpp GetCentralCurve():
/// `accuracy = 4.0f * powf(10.0f, (10.0f - detail_level) * (1.0f / 1.5f))`
///
/// - detail_level = 10 → accuracy = 4.0 (highest detail, most subdivision)
/// - detail_level = 7  → accuracy ≈ 63.5 (HIT_SHAPE_DETAIL_LEVEL)
/// - detail_level = 0  → accuracy ≈ 18,000,000 (lowest detail, least subdivision)
///
/// The accuracy value is used as a threshold in FlatWithAccuracy - smaller values
/// mean more curve subdivision (higher visual detail).
pub(super) fn detail_level_to_accuracy(detail_level: f32) -> f32 {
    4.0 * 10.0_f32.powf((10.0 - detail_level) / 1.5)
}

/// A 2D render vertex used during spline generation
/// Mirrors VPinball's RenderVertex from mesh.h
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderVertex2D {
    pub x: f32,
    pub y: f32,
    #[allow(dead_code)]
    pub smooth: bool,
    #[allow(dead_code)]
    pub slingshot: bool,
    #[allow(dead_code)]
    pub control_point: bool,
}

/// A 3D render vertex used during curve generation
/// Mirrors VPinball's RenderVertex3D from mesh.h
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderVertex3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[allow(dead_code)]
    pub smooth: bool,
    #[allow(dead_code)]
    pub slingshot: bool,
    #[allow(dead_code)]
    pub control_point: bool,
}

use crate::vpx::math::{Vec2, Vec3};
use crate::vpx::model::Vertex3dNoTex2;

/// Compute normals for a mesh by accumulating face normals
/// This matches VPinball's ComputeNormals from mesh.h
pub(super) fn compute_normals(vertices: &mut [Vertex3dNoTex2], indices: &[u32]) {
    // Reset all normals
    for v in vertices.iter_mut() {
        v.nx = 0.0;
        v.ny = 0.0;
        v.nz = 0.0;
    }

    // Accumulate face normals (normalized so each face contributes equally)
    for tri in indices.chunks_exact(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() {
            continue;
        }

        let v0 = &vertices[i0];
        let v1 = &vertices[i1];
        let v2 = &vertices[i2];

        let e1 = Vec3 {
            x: v1.x - v0.x,
            y: v1.y - v0.y,
            z: v1.z - v0.z,
        };
        let e2 = Vec3 {
            x: v2.x - v0.x,
            y: v2.y - v0.y,
            z: v2.z - v0.z,
        };
        let n = Vec3::cross(&e1, &e2);

        // Normalize face normal so each face contributes equally (like VPinball)
        let n = n.normalize();

        vertices[i0].nx += n.x;
        vertices[i0].ny += n.y;
        vertices[i0].nz += n.z;
        vertices[i1].nx += n.x;
        vertices[i1].ny += n.y;
        vertices[i1].nz += n.z;
        vertices[i2].nx += n.x;
        vertices[i2].ny += n.y;
        vertices[i2].nz += n.z;
    }

    // Normalize final vertex normals
    for v in vertices.iter_mut() {
        let len = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();
        if len > 0.0 {
            v.nx /= len;
            v.ny /= len;
            v.nz /= len;
        }
    }
}

/// Initialize cubic spline coefficients for p(s) = c0 + c1*s + c2*s^2 + c3*s^3
pub(super) fn init_cubic_spline_coeffs(x0: f32, x1: f32, t0: f32, t1: f32) -> (f32, f32, f32, f32) {
    let c0 = x0;
    let c1 = t0;
    let c2 = -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1;
    let c3 = 2.0 * x0 - 2.0 * x1 + t0 + t1;
    (c0, c1, c2, c3)
}

/// Initialize non-uniform Catmull-Rom spline coefficients
pub(super) fn init_nonuniform_catmull_coeffs(
    x0: f32,
    x1: f32,
    x2: f32,
    x3: f32,
    dt0: f32,
    dt1: f32,
    dt2: f32,
) -> (f32, f32, f32, f32) {
    // Compute tangents when parameterized in [t1,t2]
    let mut t1_tang = (x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1;
    let mut t2_tang = (x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2;

    // Rescale tangents for parametrization in [0,1]
    t1_tang *= dt1;
    t2_tang *= dt1;

    init_cubic_spline_coeffs(x1, x2, t1_tang, t2_tang)
}

/// Catmull-Rom spline curve for 2D interpolation
///
/// https://en.wikipedia.org/wiki/Catmull%E2%80%93Rom_spline
pub(super) struct CatmullCurve2D {
    cx0: f32,
    cx1: f32,
    cx2: f32,
    cx3: f32,
    cy0: f32,
    cy1: f32,
    cy2: f32,
    cy3: f32,
}

impl CatmullCurve2D {
    pub fn new(
        v0: &RenderVertex2D,
        v1: &RenderVertex2D,
        v2: &RenderVertex2D,
        v3: &RenderVertex2D,
    ) -> Self {
        let p0 = Vec2 { x: v0.x, y: v0.y };
        let p1 = Vec2 { x: v1.x, y: v1.y };
        let p2 = Vec2 { x: v2.x, y: v2.y };
        let p3 = Vec2 { x: v3.x, y: v3.y };

        let mut dt0 = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2))
            .sqrt()
            .sqrt();
        let mut dt1 = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2))
            .sqrt()
            .sqrt();
        let mut dt2 = ((p3.x - p2.x).powi(2) + (p3.y - p2.y).powi(2))
            .sqrt()
            .sqrt();

        // Check for repeated control points
        if dt1 < 1e-4 {
            dt1 = 1.0;
        }
        if dt0 < 1e-4 {
            dt0 = dt1;
        }
        if dt2 < 1e-4 {
            dt2 = dt1;
        }

        let (cx0, cx1, cx2, cx3) =
            init_nonuniform_catmull_coeffs(p0.x, p1.x, p2.x, p3.x, dt0, dt1, dt2);
        let (cy0, cy1, cy2, cy3) =
            init_nonuniform_catmull_coeffs(p0.y, p1.y, p2.y, p3.y, dt0, dt1, dt2);

        Self {
            cx0,
            cx1,
            cx2,
            cx3,
            cy0,
            cy1,
            cy2,
            cy3,
        }
    }

    pub fn get_point_at(&self, t: f32) -> (f32, f32) {
        let t2 = t * t;
        let t3 = t2 * t;

        let x = self.cx3 * t3 + self.cx2 * t2 + self.cx1 * t + self.cx0;
        let y = self.cy3 * t3 + self.cy2 * t2 + self.cy1 * t + self.cy0;

        (x, y)
    }
}

/// Check if three 2D points are collinear within the given accuracy
/// Matches VPinball's FlatWithAccuracy from mesh.h
pub(super) fn flat_with_accuracy_2d(
    v1: &RenderVertex2D,
    v2: &RenderVertex2D,
    vmid: &RenderVertex2D,
    accuracy: f32,
) -> bool {
    // Compute double the signed area of the triangle (v1, vmid, v2)
    // This is equivalent to the cross product of (vmid-v1) and (v2-v1)
    let dblarea = (vmid.x - v1.x) * (v2.y - v1.y) - (v2.x - v1.x) * (vmid.y - v1.y);

    // VPinball compares area squared directly against accuracy (not accuracy squared!)
    dblarea * dblarea < accuracy
}

/// Recursively subdivide a 2D curve segment until it's flat enough
pub(super) fn recurse_smooth_line_2d(
    cc: &CatmullCurve2D,
    t1: f32,
    t2: f32,
    vt1: &RenderVertex2D,
    vt2: &RenderVertex2D,
    vv: &mut Vec<RenderVertex2D>,
    accuracy: f32,
) {
    let t_mid = (t1 + t2) * 0.5;
    let (x, y) = cc.get_point_at(t_mid);
    let vmid = RenderVertex2D {
        x,
        y,
        smooth: true,
        ..Default::default()
    };

    if flat_with_accuracy_2d(vt1, vt2, &vmid, accuracy) {
        vv.push(*vt1);
    } else {
        recurse_smooth_line_2d(cc, t1, t_mid, vt1, &vmid, vv, accuracy);
        recurse_smooth_line_2d(cc, t_mid, t2, &vmid, vt2, vv, accuracy);
    }
}

/// Get the 2D vertices from drag points using spline interpolation.
/// If `loop_curve` is true, the curve is closed (for rubbers, flashers).
/// If false, the curve is open (for ramps).
pub(super) fn get_rg_vertex_2d(
    drag_points: &[crate::vpx::gameitem::dragpoint::DragPoint],
    accuracy: f32,
    loop_curve: bool,
) -> Vec<RenderVertex2D> {
    let cpoint = drag_points.len();
    if cpoint < 2 {
        return vec![];
    }

    let mut vv = Vec::new();
    let endpoint = if loop_curve { cpoint } else { cpoint - 1 };

    for i in 0..endpoint {
        let pdp1 = &drag_points[i];
        let pdp2 = &drag_points[(i + 1) % cpoint];

        // Skip if two points coincide
        if (pdp1.x - pdp2.x).abs() < 1e-6 && (pdp1.y - pdp2.y).abs() < 1e-6 {
            continue;
        }

        let iprev = if pdp1.smooth {
            if loop_curve {
                (i + cpoint - 1) % cpoint
            } else if i > 0 {
                i - 1
            } else {
                i
            }
        } else {
            i
        };

        let inext = if pdp2.smooth {
            if loop_curve {
                (i + 2) % cpoint
            } else if i + 2 < cpoint {
                i + 2
            } else {
                i + 1
            }
        } else {
            (i + 1) % cpoint
        };

        let pdp0 = &drag_points[iprev];
        let pdp3 = &drag_points[if loop_curve {
            inext
        } else {
            inext.min(cpoint - 1)
        }];

        let v0 = RenderVertex2D {
            x: pdp0.x,
            y: pdp0.y,
            smooth: pdp0.smooth,
            control_point: true,
            ..Default::default()
        };
        let v1 = RenderVertex2D {
            x: pdp1.x,
            y: pdp1.y,
            smooth: pdp1.smooth,
            control_point: true,
            ..Default::default()
        };
        let v2 = RenderVertex2D {
            x: pdp2.x,
            y: pdp2.y,
            smooth: pdp2.smooth,
            control_point: true,
            ..Default::default()
        };
        let v3 = RenderVertex2D {
            x: pdp3.x,
            y: pdp3.y,
            smooth: pdp3.smooth,
            control_point: true,
            ..Default::default()
        };

        let cc = CatmullCurve2D::new(&v0, &v1, &v2, &v3);

        let rendv1 = RenderVertex2D {
            x: v1.x,
            y: v1.y,
            smooth: pdp1.smooth,
            control_point: true,
            ..Default::default()
        };

        let rendv2 = RenderVertex2D {
            x: v2.x,
            y: v2.y,
            smooth: pdp2.smooth,
            control_point: true,
            ..Default::default()
        };

        recurse_smooth_line_2d(&cc, 0.0, 1.0, &rendv1, &rendv2, &mut vv, accuracy);
    }

    vv
}

// ---- Polygon triangulation ----
// Ported from VPinball's math/mesh.h: PolygonToTriangles, AdvancePoint, GetDot, FLinesIntersect,
// FindCornerVertex, DetermineWindingOrder

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

    let a = &vertices[(i_min + n - 1) % n];
    let b = &vertices[i_min];
    let c = &vertices[(i_min + 1) % n];

    let det_orient = (b.x * c.y + a.x * b.y + a.y * c.x) - (a.y * b.x + b.y * c.x + a.x * c.y);
    det_orient > 0.0
}

/// Cross product of two vectors defined by (pvEnd1→pvJoint) and (pvEnd2→pvJoint)
/// This matches VPinball's GetDot function from mesh.h
/// Returns positive for CCW turn, negative for CW turn
fn get_dot(end1: &RenderVertex2D, joint: &RenderVertex2D, end2: &RenderVertex2D) -> f32 {
    (joint.x - end1.x) * (joint.y - end2.y) - (joint.y - end1.y) * (joint.x - end2.x)
}

/// Check if two line segments intersect
/// This matches VPinball's FLinesIntersect function from mesh.h
fn lines_intersect(
    start1: &RenderVertex2D,
    start2: &RenderVertex2D,
    end1: &RenderVertex2D,
    end2: &RenderVertex2D,
) -> bool {
    let x1 = start1.x;
    let y1 = start1.y;
    let x2 = start2.x;
    let y2 = start2.y;
    let x3 = end1.x;
    let y3 = end1.y;
    let x4 = end2.x;
    let y4 = end2.y;

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

/// Check if vertex b can be removed to form triangle (a, c, b)
/// This matches VPinball's AdvancePoint function from mesh.h
fn advance_point(
    vertices: &[RenderVertex2D],
    pvpoly: &[u32],
    a: u32,
    b: u32,
    c: u32,
    pre: u32,
    post: u32,
) -> bool {
    let pv1 = &vertices[a as usize];
    let pv2 = &vertices[b as usize];
    let pv3 = &vertices[c as usize];
    let pv_pre = &vertices[pre as usize];
    let pv_post = &vertices[post as usize];

    if get_dot(pv1, pv2, pv3) < 0.0 {
        return false;
    }
    // Make sure angle created by new triangle line falls inside existing angles
    if (get_dot(pv_pre, pv1, pv2) > 0.0) && (get_dot(pv_pre, pv1, pv3) < 0.0) {
        return false;
    }
    if (get_dot(pv2, pv3, pv_post) > 0.0) && (get_dot(pv1, pv3, pv_post) < 0.0) {
        return false;
    }

    // Now make sure the interior segment of this triangle (line a→c) does not
    // intersect the polygon anywhere
    let minx = pv1.x.min(pv3.x);
    let maxx = pv1.x.max(pv3.x);
    let miny = pv1.y.min(pv3.y);
    let maxy = pv1.y.max(pv3.y);

    for i in 0..pvpoly.len() {
        let pv_cross1 = &vertices[pvpoly[i] as usize];
        let next_i = if i < pvpoly.len() - 1 { i + 1 } else { 0 };
        let pv_cross2 = &vertices[pvpoly[next_i] as usize];

        // Skip edges that share a vertex with the diagonal
        if std::ptr::eq(pv_cross1, pv1)
            || std::ptr::eq(pv_cross2, pv1)
            || std::ptr::eq(pv_cross1, pv3)
            || std::ptr::eq(pv_cross2, pv3)
        {
            continue;
        }

        // Bounding box early-out (matching VPinball's checks)
        if (pv_cross1.y < miny && pv_cross2.y < miny)
            || (pv_cross1.y > maxy && pv_cross2.y > maxy)
            || (pv_cross1.x < minx && pv_cross2.x < minx)
            || (pv_cross1.x > maxx && pv_cross2.y > maxx)
        {
            continue;
        }

        if lines_intersect(pv1, pv3, pv_cross1, pv_cross2) {
            return false;
        }
    }

    true
}

/// Triangulate a polygon using VPinball's PolygonToTriangles algorithm
/// Returns indices into the vertex array forming triangles
///
/// This is a direct port of VPinball's PolygonToTriangles from mesh.h
/// with support_both_winding_orders=true
pub(super) fn polygon_to_triangles(vertices: &[RenderVertex2D]) -> Vec<u32> {
    let n = vertices.len();
    if n < 3 {
        return vec![];
    }

    let mut pvpoly: Vec<u32> = (0..n as u32).collect();
    if is_clockwise(vertices) {
        pvpoly.reverse();
    }

    let tricount = pvpoly.len() - 2;
    let mut pvtri: Vec<u32> = Vec::with_capacity(tricount * 3);

    for _ in 0..tricount {
        let s = pvpoly.len();
        let mut found = false;

        for i in 0..s {
            let pre = pvpoly[if i == 0 { s - 1 } else { i - 1 }];
            let a = pvpoly[i];
            let b = pvpoly[if i < s - 1 { i + 1 } else { 0 }];
            let c = pvpoly[if i < s - 2 { i + 2 } else { (i + 2) - s }];
            let post = pvpoly[if i < s - 3 { i + 3 } else { (i + 3) - s }];

            if advance_point(vertices, &pvpoly, a, b, c, pre, post) {
                pvtri.push(a);
                pvtri.push(c);
                pvtri.push(b);
                let remove_idx = if i < s - 1 { i + 1 } else { 0 };
                pvpoly.remove(remove_idx);
                found = true;
                break;
            }
        }

        if !found {
            break;
        }
    }

    pvtri
}

#[cfg(test)]
pub mod test_utils {
    use crate::vpx::gameitem::primitive::compress_mesh_data;
    use crate::vpx::model::Vertex3dNoTex2;

    /// Creates minimal compressed mesh data for a single triangle.
    ///
    /// Returns (compressed_vertices, compressed_indices, num_vertices, num_indices)
    ///
    /// This is useful for creating test primitives that have valid mesh data
    /// without needing to load from a file.
    pub fn create_minimal_mesh_data() -> (Vec<u8>, Vec<u8>, u32, u32) {
        // Create 3 vertices (a simple triangle)
        let vertices = vec![
            Vertex3dNoTex2 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 0.0,
            },
            Vertex3dNoTex2 {
                x: 100.0,
                y: 0.0,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 1.0,
                tv: 0.0,
            },
            Vertex3dNoTex2 {
                x: 50.0,
                y: 100.0,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.5,
                tv: 1.0,
            },
        ];

        // Convert vertices to raw bytes (32 bytes per vertex)
        let mut raw_vertices = Vec::new();
        for v in &vertices {
            raw_vertices.extend_from_slice(&v.x.to_le_bytes());
            raw_vertices.extend_from_slice(&v.y.to_le_bytes());
            raw_vertices.extend_from_slice(&v.z.to_le_bytes());
            raw_vertices.extend_from_slice(&v.nx.to_le_bytes());
            raw_vertices.extend_from_slice(&v.ny.to_le_bytes());
            raw_vertices.extend_from_slice(&v.nz.to_le_bytes());
            raw_vertices.extend_from_slice(&v.tu.to_le_bytes());
            raw_vertices.extend_from_slice(&v.tv.to_le_bytes());
        }

        // Create indices for a single triangle (2 bytes per index since < 65535 vertices)
        let indices: Vec<u16> = vec![0, 1, 2];
        let mut raw_indices = Vec::new();
        for i in &indices {
            raw_indices.extend_from_slice(&i.to_le_bytes());
        }

        // Compress the data
        let compressed_vertices = compress_mesh_data(&raw_vertices).unwrap();
        let compressed_indices = compress_mesh_data(&raw_indices).unwrap();

        (
            compressed_vertices,
            compressed_indices,
            vertices.len() as u32,
            indices.len() as u32,
        )
    }
}
