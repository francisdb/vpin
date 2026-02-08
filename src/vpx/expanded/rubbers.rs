//! Rubber mesh generation for expanded VPX export
//!
//! This module ports the rubber mesh generation from Visual Pinball's rubber.cpp.
//! Rubbers are rendered as tubular shapes that follow a spline curve defined by drag points.

use super::mesh_common::{
    RenderVertex2D, Vec2, Vec3, compute_normals, generated_mesh_file_name, get_rotated_axis,
    init_nonuniform_catmull_coeffs, write_mesh_to_file,
};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::dragpoint::DragPoint;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::rubber::Rubber;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

/// Catmull-Rom spline curve for 2D interpolation
struct CatmullCurve2D {
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
    fn new(
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

    fn get_point_at(&self, t: f32) -> (f32, f32) {
        let t2 = t * t;
        let t3 = t2 * t;

        let x = self.cx3 * t3 + self.cx2 * t2 + self.cx1 * t + self.cx0;
        let y = self.cy3 * t3 + self.cy2 * t2 + self.cy1 * t + self.cy0;

        (x, y)
    }
}

/// Check if three points are collinear within the given accuracy
fn flat_with_accuracy(
    v1: &RenderVertex2D,
    v2: &RenderVertex2D,
    vmid: &RenderVertex2D,
    accuracy: f32,
) -> bool {
    // Calculate perpendicular distance from vmid to line v1-v2
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;

    let line_len_sq = dx * dx + dy * dy;
    if line_len_sq < 1e-10 {
        return true;
    }

    // Vector from v1 to vmid
    let px = vmid.x - v1.x;
    let py = vmid.y - v1.y;

    // Cross product gives perpendicular distance * line_length
    let cross = dx * py - dy * px;
    let dist_sq = (cross * cross) / line_len_sq;

    dist_sq < accuracy * accuracy
}

/// Recursively subdivide a curve segment until it's flat enough
fn recurse_smooth_line(
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

    if flat_with_accuracy(vt1, vt2, &vmid, accuracy) {
        vv.push(*vt1);
    } else {
        recurse_smooth_line(cc, t1, t_mid, vt1, &vmid, vv, accuracy);
        recurse_smooth_line(cc, t_mid, t2, &vmid, vt2, vv, accuracy);
    }
}

/// Get the central curve of the rubber from drag points.
/// Unlike ramps, rubbers always loop.
fn get_central_curve(drag_points: &[DragPoint], accuracy: f32) -> Vec<RenderVertex2D> {
    let cpoint = drag_points.len();
    if cpoint < 2 {
        return vec![];
    }

    let mut vv = Vec::new();

    // Rubbers always loop
    for i in 0..cpoint {
        let pdp1 = &drag_points[i];
        let pdp2 = &drag_points[(i + 1) % cpoint];

        // Skip if two points coincide
        if (pdp1.x - pdp2.x).abs() < 1e-6 && (pdp1.y - pdp2.y).abs() < 1e-6 {
            continue;
        }

        // For rubbers, always loop
        let iprev = if pdp1.smooth {
            (i + cpoint - 1) % cpoint
        } else {
            i
        };
        let inext = if pdp2.smooth {
            (i + 2) % cpoint
        } else {
            (i + 1) % cpoint
        };

        let pdp0 = &drag_points[iprev];
        let pdp3 = &drag_points[inext];

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

        recurse_smooth_line(&cc, 0.0, 1.0, &rendv1, &rendv2, &mut vv, accuracy);
    }

    vv
}

/// Get the spline vertices for the rubber outline
/// Returns (outline_vertices, middle_points) where:
/// - outline_vertices: 2D outline of the rubber (right side forward, left side backward)
/// - middle_points: the center points of the curve
fn get_spline_vertex(
    drag_points: &[DragPoint],
    thickness: f32,
    accuracy: f32,
) -> (Vec<Vec2>, Vec<Vec2>) {
    let vvertex = get_central_curve(drag_points, accuracy);
    let cvertex = vvertex.len();

    if cvertex == 0 {
        return (vec![], vec![]);
    }

    let mut rgv_local: Vec<Vec2> = vec![Vec2::default(); (cvertex + 1) * 2];
    let mut middle_points: Vec<Vec2> = vec![Vec2::default(); cvertex + 1];

    for i in 0..cvertex {
        // prev and next wrap around as rubbers always loop
        let vprev = &vvertex[if i > 0 { i - 1 } else { cvertex - 1 }];
        let vnext = &vvertex[if i < cvertex - 1 { i + 1 } else { 0 }];
        let vmiddle = &vvertex[i];

        // Get normal at this point
        let v1normal = Vec2 {
            x: vprev.y - vmiddle.y,
            y: vmiddle.x - vprev.x,
        }; // vector vmiddle-vprev rotated RIGHT
        let v2normal = Vec2 {
            x: vmiddle.y - vnext.y,
            y: vnext.x - vmiddle.x,
        }; // vector vnext-vmiddle rotated RIGHT

        let vnormal = if cvertex == 2 && i == cvertex - 1 {
            v1normal.normalize()
        } else if cvertex == 2 && i == 0 {
            v2normal.normalize()
        } else {
            let v1n = v1normal.normalize();
            let v2n = v2normal.normalize();

            if (v1n.x - v2n.x).abs() < 0.0001 && (v1n.y - v2n.y).abs() < 0.0001 {
                // Two parallel segments
                v1n
            } else {
                // Find intersection of the two edges meeting at this point
                let a = vprev.y - vmiddle.y;
                let b = vmiddle.x - vprev.x;
                let c = a * (v1n.x - vprev.x) + b * (v1n.y - vprev.y);

                let d = vnext.y - vmiddle.y;
                let e = vmiddle.x - vnext.x;
                let f = d * (v2n.x - vnext.x) + e * (v2n.y - vnext.y);

                let det = a * e - b * d;
                let inv_det = if det != 0.0 { 1.0 / det } else { 0.0 };

                let intersectx = (b * f - e * c) * inv_det;
                let intersecty = (c * d - a * f) * inv_det;

                Vec2 {
                    x: vmiddle.x - intersectx,
                    y: vmiddle.y - intersecty,
                }
            }
        };

        let widthcur = thickness;
        middle_points[i] = Vec2 {
            x: vmiddle.x,
            y: vmiddle.y,
        };

        rgv_local[i] = Vec2 {
            x: vmiddle.x,
            y: vmiddle.y,
        } + vnormal * (widthcur * 0.5);
        rgv_local[(cvertex + 1) * 2 - i - 1] = Vec2 {
            x: vmiddle.x,
            y: vmiddle.y,
        } - vnormal * (widthcur * 0.5);

        if i == 0 {
            rgv_local[cvertex] = rgv_local[0];
            rgv_local[(cvertex + 1) * 2 - cvertex - 1] = rgv_local[(cvertex + 1) * 2 - 1];
        }
    }

    middle_points[cvertex] = middle_points[0];

    (rgv_local, middle_points)
}

/// Generate the rubber mesh
/// This is a port of Rubber::GenerateMesh from rubber.cpp
fn generate_mesh(rubber: &Rubber) -> Option<(Vec<Vertex3dNoTex2>, Vec<u32>)> {
    // Use a fixed accuracy for the highest detail level
    // This is 4.0 * 10^((10-10)/1.5) = 4.0 * 10^0 = 4.0
    let accuracy = 4.0f32;

    let (_, middle_points) =
        get_spline_vertex(&rubber.drag_points, rubber.thickness as f32, accuracy);

    let num_rings = middle_points.len() - 1; // splinePoints - 1
    if num_rings < 1 {
        return None;
    }

    // Use 8 segments for the circular cross-section (similar to wire ramps)
    let num_segments = 8;

    let num_vertices = num_rings * num_segments;
    let num_indices = 6 * num_vertices;

    let mut vertices: Vec<Vertex3dNoTex2> = vec![
        Vertex3dNoTex2 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            nx: 0.0,
            ny: 0.0,
            nz: 0.0,
            tu: 0.0,
            tv: 0.0,
        };
        num_vertices
    ];
    let mut indices: Vec<u32> = vec![0; num_indices];

    let height = rubber.hit_height.unwrap_or(rubber.height);
    let thickness = rubber.thickness as f32;

    let mut prev_binorm = Vec3::default();
    let inv_nr = 1.0 / num_rings as f32;
    let inv_ns = 1.0 / num_segments as f32;

    for i in 0..num_rings {
        let i2 = if i == num_rings - 1 { 0 } else { i + 1 };

        let tangent = Vec3 {
            x: middle_points[i2].x - middle_points[i].x,
            y: middle_points[i2].y - middle_points[i].y,
            z: 0.0,
        };

        let (normal, binorm) = if i == 0 {
            let up = Vec3 {
                x: middle_points[i2].x + middle_points[i].x,
                y: middle_points[i2].y + middle_points[i].y,
                z: height * 2.0,
            };
            // CrossProduct(tangent, up)
            let normal = Vec3 {
                x: tangent.y * up.z,
                y: -tangent.x * up.z,
                z: tangent.x * up.y - tangent.y * up.x,
            };
            // CrossProduct(tangent, normal)
            let binorm = Vec3 {
                x: tangent.y * normal.z,
                y: -tangent.x * normal.z,
                z: tangent.x * normal.y - tangent.y * normal.x,
            };
            (normal, binorm)
        } else {
            let normal = Vec3::cross(&prev_binorm, &tangent);
            let binorm = Vec3::cross(&tangent, &normal);
            (normal, binorm)
        };

        let binorm = binorm.normalize();
        let normal = normal.normalize();
        prev_binorm = binorm;

        let u = i as f32 * inv_nr;

        for j in 0..num_segments {
            let index = i * num_segments + j;
            let v = (j as f32 + u) * inv_ns;

            let angle = j as f32 * 360.0 * inv_ns;
            let tmp = get_rotated_axis(angle, &tangent, &normal) * (thickness * 0.5);

            vertices[index].x = middle_points[i].x + tmp.x;
            vertices[index].y = middle_points[i].y + tmp.y;
            vertices[index].z = height + tmp.z;

            // Texture coordinates
            vertices[index].tu = u;
            vertices[index].tv = v;
        }
    }

    // Calculate face indices
    for i in 0..num_rings {
        for j in 0..num_segments {
            let mut quad = [0u32; 4];
            quad[0] = (i * num_segments + j) as u32;

            quad[1] = if j != num_segments - 1 {
                (i * num_segments + j + 1) as u32
            } else {
                (i * num_segments) as u32
            };

            if i != num_rings - 1 {
                quad[2] = ((i + 1) * num_segments + j) as u32;
                quad[3] = if j != num_segments - 1 {
                    ((i + 1) * num_segments + j + 1) as u32
                } else {
                    ((i + 1) * num_segments) as u32
                };
            } else {
                quad[2] = j as u32;
                quad[3] = if j != num_segments - 1 {
                    (j + 1) as u32
                } else {
                    0
                };
            }

            let offs = (i * num_segments + j) * 6;
            indices[offs] = quad[0];
            indices[offs + 1] = quad[1];
            indices[offs + 2] = quad[2];
            indices[offs + 3] = quad[3];
            indices[offs + 4] = quad[2];
            indices[offs + 5] = quad[1];
        }
    }

    // Compute normals
    compute_normals(&mut vertices, &indices);

    Some((vertices, indices))
}

/// Apply rotation transformation to rubber mesh
/// This is a port of Rubber::UpdateRubber from rubber.cpp
fn apply_rotation(
    vertices: &mut [Vertex3dNoTex2],
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    height: f32,
) {
    if rot_x == 0.0 && rot_y == 0.0 && rot_z == 0.0 {
        return;
    }

    // Find the middle point of the mesh for rotation center
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for v in vertices.iter() {
        min_x = min_x.min(v.x);
        max_x = max_x.max(v.x);
        min_y = min_y.min(v.y);
        max_y = max_y.max(v.y);
        min_z = min_z.min(v.z);
        max_z = max_z.max(v.z);
    }

    let middle_x = (max_x + min_x) * 0.5;
    let middle_y = (max_y + min_y) * 0.5;
    let middle_z = (max_z + min_z) * 0.5;

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
        // Translate to origin
        let x = v.x - middle_x;
        let y = v.y - middle_y;
        let z = v.z - middle_z;

        // Apply rotation
        let new_x = m00 * x + m01 * y + m02 * z;
        let new_y = m10 * x + m11 * y + m12 * z;
        let new_z = m20 * x + m21 * y + m22 * z;

        // Translate back with height adjustment
        v.x = new_x + middle_x;
        v.y = new_y + middle_y;
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

/// Build the complete rubber mesh
fn build_rubber_mesh(rubber: &Rubber) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if rubber.thickness == 0 {
        return None;
    }

    if rubber.drag_points.len() < 2 {
        return None;
    }

    let (mut vertices, indices) = generate_mesh(rubber)?;

    // Apply rotation transformation
    apply_rotation(
        &mut vertices,
        rubber.rot_x,
        rubber.rot_y,
        rubber.rot_z,
        rubber.height,
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

/// Write rubber meshes to file
pub(super) fn write_rubber_meshes(
    gameitems_dir: &Path,
    rubber: &Rubber,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_rubber_mesh(rubber) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &rubber.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catmull_curve_2d() {
        let v0 = RenderVertex2D {
            x: 0.0,
            y: 0.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };
        let v1 = RenderVertex2D {
            x: 1.0,
            y: 0.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };
        let v2 = RenderVertex2D {
            x: 2.0,
            y: 1.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };
        let v3 = RenderVertex2D {
            x: 3.0,
            y: 1.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };

        let curve = CatmullCurve2D::new(&v0, &v1, &v2, &v3);
        let (x, y) = curve.get_point_at(0.0);
        assert!((x - 1.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);

        let (x, y) = curve.get_point_at(1.0);
        assert!((x - 2.0).abs() < 0.01);
        assert!((y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_simple_rubber() {
        let mut rubber = Rubber::default();
        rubber.drag_points = vec![
            DragPoint {
                x: 50.0,
                y: 0.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: 50.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: -50.0,
                y: 0.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: -50.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
        ];

        let result = build_rubber_mesh(&rubber);
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }

    #[test]
    fn test_rubber_with_rotation() {
        let mut rubber = Rubber::default();
        rubber.rot_x = 45.0;
        rubber.rot_y = 30.0;
        rubber.rot_z = 15.0;
        rubber.drag_points = vec![
            DragPoint {
                x: 50.0,
                y: 0.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: 50.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: -50.0,
                y: 0.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 0.0,
                y: -50.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
        ];

        let result = build_rubber_mesh(&rubber);
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }

    #[test]
    fn test_left_sling_rubber() {
        // This rubber data is from LeftSling.json and should generate a mesh
        let mut rubber = Rubber::default();
        rubber.height = 29.0;
        rubber.hit_height = Some(29.0);
        rubber.thickness = 11;
        rubber.is_visible = false; // Invisible, but should still generate mesh
        rubber.drag_points = vec![
            DragPoint {
                x: 149.5576,
                y: 1492.057,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 146.7396,
                y: 1496.274,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 145.75,
                y: 1501.25,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 145.75,
                y: 1653.0,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 146.7396,
                y: 1657.974,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 149.5576,
                y: 1662.192,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 228.775,
                y: 1717.51,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 233.75,
                y: 1718.5,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 238.725,
                y: 1717.51,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 242.9424,
                y: 1714.692,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 245.7604,
                y: 1710.474,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 246.75,
                y: 1705.5,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 234.22433,
                y: 1596.8163,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 231.13315,
                y: 1588.2709,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 227.76147,
                y: 1579.4148,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 167.9424,
                y: 1492.057,
                z: 0.0,
                smooth: false,
                ..Default::default()
            },
            DragPoint {
                x: 163.725,
                y: 1489.239,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 158.75,
                y: 1488.25,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 153.775,
                y: 1489.239,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
        ];

        let result = build_rubber_mesh(&rubber);
        assert!(
            result.is_some(),
            "Expected mesh to be generated but got None"
        );

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }
}
