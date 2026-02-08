//! Ramp mesh generation for expanded VPX export
//!
//! This module ports the ramp mesh generation from Visual Pinball's ramp.cpp.
//! Ramps can be either flat (with optional walls) or wire ramps (1-4 wire types).

use super::mesh_common::{Vec2, Vec3, generated_mesh_file_name, write_mesh_to_file};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::dragpoint::DragPoint;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::ramp::{Ramp, RampType};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;
use std::path::Path;

/// A 3D point used during curve generation
#[derive(Debug, Clone, Copy)]
struct RenderVertex3D {
    x: f32,
    y: f32,
    z: f32,
    #[allow(dead_code)]
    smooth: bool,
    #[allow(dead_code)]
    control_point: bool,
}

impl RenderVertex3D {
    #[allow(dead_code)]
    fn set(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }
}

/// Catmull-Rom spline coefficients for cubic interpolation
struct CatmullCurve3D {
    cx0: f32,
    cx1: f32,
    cx2: f32,
    cx3: f32,
    cy0: f32,
    cy1: f32,
    cy2: f32,
    cy3: f32,
    cz0: f32,
    cz1: f32,
    cz2: f32,
    cz3: f32,
}

impl CatmullCurve3D {
    fn new(
        v0: &RenderVertex3D,
        v1: &RenderVertex3D,
        v2: &RenderVertex3D,
        v3: &RenderVertex3D,
    ) -> Self {
        let p0 = Vec3 {
            x: v0.x,
            y: v0.y,
            z: v0.z,
        };
        let p1 = Vec3 {
            x: v1.x,
            y: v1.y,
            z: v1.z,
        };
        let p2 = Vec3 {
            x: v2.x,
            y: v2.y,
            z: v2.z,
        };
        let p3 = Vec3 {
            x: v3.x,
            y: v3.y,
            z: v3.z,
        };

        let mut dt0 = (p1 - p0).length().sqrt();
        let mut dt1 = (p2 - p1).length().sqrt();
        let mut dt2 = (p3 - p2).length().sqrt();

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
        let (cz0, cz1, cz2, cz3) =
            init_nonuniform_catmull_coeffs(p0.z, p1.z, p2.z, p3.z, dt0, dt1, dt2);

        Self {
            cx0,
            cx1,
            cx2,
            cx3,
            cy0,
            cy1,
            cy2,
            cy3,
            cz0,
            cz1,
            cz2,
            cz3,
        }
    }

    fn get_point_at(&self, t: f32) -> (f32, f32, f32) {
        let t2 = t * t;
        let t3 = t2 * t;

        let x = self.cx3 * t3 + self.cx2 * t2 + self.cx1 * t + self.cx0;
        let y = self.cy3 * t3 + self.cy2 * t2 + self.cy1 * t + self.cy0;
        let z = self.cz3 * t3 + self.cz2 * t2 + self.cz1 * t + self.cz0;

        (x, y, z)
    }
}

/// Initialize cubic spline coefficients for p(s) = c0 + c1*s + c2*s^2 + c3*s^3
fn init_cubic_spline_coeffs(x0: f32, x1: f32, t0: f32, t1: f32) -> (f32, f32, f32, f32) {
    let c0 = x0;
    let c1 = t0;
    let c2 = -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1;
    let c3 = 2.0 * x0 - 2.0 * x1 + t0 + t1;
    (c0, c1, c2, c3)
}

/// Initialize non-uniform Catmull-Rom spline coefficients
fn init_nonuniform_catmull_coeffs(
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

/// Check if three points are collinear within the given accuracy
fn flat_with_accuracy(
    v1: &RenderVertex3D,
    v2: &RenderVertex3D,
    vmid: &RenderVertex3D,
    accuracy: f32,
) -> bool {
    // Calculate perpendicular distance from vmid to line v1-v2
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;
    let dz = v2.z - v1.z;

    let line_len_sq = dx * dx + dy * dy + dz * dz;
    if line_len_sq < 1e-10 {
        return true;
    }

    // Vector from v1 to vmid
    let px = vmid.x - v1.x;
    let py = vmid.y - v1.y;
    let pz = vmid.z - v1.z;

    // Cross product gives perpendicular distance * line_length
    let cross_x = dy * pz - dz * py;
    let cross_y = dz * px - dx * pz;
    let cross_z = dx * py - dy * px;

    let cross_len_sq = cross_x * cross_x + cross_y * cross_y + cross_z * cross_z;
    let dist_sq = cross_len_sq / line_len_sq;

    dist_sq < accuracy * accuracy
}

/// Recursively subdivide a curve segment until it's flat enough
fn recurse_smooth_line(
    cc: &CatmullCurve3D,
    t1: f32,
    t2: f32,
    vt1: &RenderVertex3D,
    vt2: &RenderVertex3D,
    vv: &mut Vec<RenderVertex3D>,
    accuracy: f32,
) {
    let t_mid = (t1 + t2) * 0.5;
    let (x, y, z) = cc.get_point_at(t_mid);
    let vmid = RenderVertex3D {
        x,
        y,
        z,
        smooth: true,
        control_point: false,
    };

    if flat_with_accuracy(vt1, vt2, &vmid, accuracy) {
        vv.push(*vt1);
    } else {
        recurse_smooth_line(cc, t1, t_mid, vt1, &vmid, vv, accuracy);
        recurse_smooth_line(cc, t_mid, t2, &vmid, vt2, vv, accuracy);
    }
}

/// Get the interpolated central curve of the ramp from drag points
fn get_central_curve(drag_points: &[DragPoint], accuracy: f32) -> Vec<RenderVertex3D> {
    let cpoint = drag_points.len();
    if cpoint < 2 {
        return vec![];
    }

    let mut vv = Vec::new();

    // Ramps don't loop, so we go from 0 to cpoint-1
    let endpoint = cpoint - 1;

    for i in 0..endpoint {
        let pdp1 = &drag_points[i];
        let pdp2 = &drag_points[i + 1];

        // Skip if two points coincide
        if (pdp1.x - pdp2.x).abs() < 1e-6
            && (pdp1.y - pdp2.y).abs() < 1e-6
            && (pdp1.z - pdp2.z).abs() < 1e-6
        {
            continue;
        }

        // Ramps don't loop
        let iprev = if pdp1.smooth && i > 0 { i - 1 } else { i };
        let inext = if pdp2.smooth && i + 2 < cpoint {
            i + 2
        } else {
            i + 1
        };

        let pdp0 = &drag_points[iprev];
        let pdp3 = &drag_points[inext];

        let v0 = RenderVertex3D {
            x: pdp0.x,
            y: pdp0.y,
            z: pdp0.z,
            smooth: pdp0.smooth,
            control_point: true,
        };
        let v1 = RenderVertex3D {
            x: pdp1.x,
            y: pdp1.y,
            z: pdp1.z,
            smooth: pdp1.smooth,
            control_point: true,
        };
        let v2 = RenderVertex3D {
            x: pdp2.x,
            y: pdp2.y,
            z: pdp2.z,
            smooth: pdp2.smooth,
            control_point: true,
        };
        let v3 = RenderVertex3D {
            x: pdp3.x,
            y: pdp3.y,
            z: pdp3.z,
            smooth: pdp3.smooth,
            control_point: true,
        };

        let cc = CatmullCurve3D::new(&v0, &v1, &v2, &v3);

        let rendv1 = RenderVertex3D {
            x: v1.x,
            y: v1.y,
            z: v1.z,
            smooth: pdp1.smooth,
            control_point: true,
        };

        let rendv2 = RenderVertex3D {
            x: v2.x,
            y: v2.y,
            z: v2.z,
            smooth: pdp2.smooth,
            control_point: true,
        };

        recurse_smooth_line(&cc, 0.0, 1.0, &rendv1, &rendv2, &mut vv, accuracy);
    }

    // Add the very last point
    if let Some(last) = drag_points.last() {
        vv.push(RenderVertex3D {
            x: last.x,
            y: last.y,
            z: last.z,
            smooth: true,
            control_point: true,
        });
    }

    vv
}

/// Compute the 2D outline vertices of the ramp along with heights and ratios
fn get_ramp_vertex(
    ramp: &Ramp,
    vvertex: &[RenderVertex3D],
    inc_width: bool,
) -> (Vec<Vec2>, Vec<f32>, Vec<f32>) {
    let cvertex = vvertex.len();
    if cvertex == 0 {
        return (vec![], vec![], vec![]);
    }

    let mut rgv_local: Vec<Vec2> = vec![Vec2 { x: 0.0, y: 0.0 }; cvertex * 2];
    let mut rgheight: Vec<f32> = vec![0.0; cvertex];
    let mut rgratio: Vec<f32> = vec![0.0; cvertex];

    // Compute an approximation to the length of the central curve
    let mut totallength = 0.0f32;
    for i in 0..(cvertex - 1) {
        let v1 = &vvertex[i];
        let v2 = &vvertex[i + 1];

        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;
        totallength += (dx * dx + dy * dy).sqrt();
    }

    let bottom_height = ramp.height_bottom;
    let top_height = ramp.height_top;

    let mut currentlength = 0.0f32;

    for i in 0..cvertex {
        // Clamp next and prev as ramps do not loop
        let vprev = &vvertex[if i > 0 { i - 1 } else { i }];
        let vnext = &vvertex[if i < cvertex - 1 { i + 1 } else { i }];
        let vmiddle = &vvertex[i];

        // Get normal at this point
        let v1normal = Vec2 {
            x: vprev.y - vmiddle.y,
            y: vmiddle.x - vprev.x,
        };
        let v2normal = Vec2 {
            x: vmiddle.y - vnext.y,
            y: vnext.x - vmiddle.x,
        };

        let vnormal = if i == cvertex - 1 {
            v1normal.normalize()
        } else if i == 0 {
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

        // Update current length along the ramp
        {
            let dx = vprev.x - vmiddle.x;
            let dy = vprev.y - vmiddle.y;
            currentlength += (dx * dx + dy * dy).sqrt();
        }

        let percentage = if totallength > 0.0 {
            currentlength / totallength
        } else {
            0.0
        };
        let mut widthcur = percentage * (ramp.width_top - ramp.width_bottom) + ramp.width_bottom;

        rgheight[i] = vmiddle.z + percentage * (top_height - bottom_height) + bottom_height;
        rgratio[i] = 1.0 - percentage;

        // Handle wire ramp widths
        if is_habitrail(ramp) && ramp.ramp_type != RampType::OneWire {
            widthcur = ramp.wire_distance_x;
            if inc_width {
                widthcur += 20.0;
            }
        } else if ramp.ramp_type == RampType::OneWire {
            widthcur = ramp.wire_diameter;
        }

        let vmid = Vec2 {
            x: vmiddle.x,
            y: vmiddle.y,
        };
        rgv_local[i] = vmid + vnormal * (widthcur * 0.5);
        rgv_local[cvertex * 2 - i - 1] = vmid - vnormal * (widthcur * 0.5);
    }

    (rgv_local, rgheight, rgratio)
}

/// Check if the ramp is a wire ramp (habitrail)
fn is_habitrail(ramp: &Ramp) -> bool {
    matches!(
        ramp.ramp_type,
        RampType::FourWire
            | RampType::OneWire
            | RampType::TwoWire
            | RampType::ThreeWireLeft
            | RampType::ThreeWireRight
    )
}

/// Generate the flat ramp mesh (floor and walls)
fn build_flat_ramp_mesh(
    ramp: &Ramp,
    vvertex: &[RenderVertex3D],
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    let (rgv_local, rgheight, rgratio) = get_ramp_vertex(ramp, vvertex, true);
    let ramp_vertex = vvertex.len();

    if ramp_vertex < 2 {
        return None;
    }

    let num_vertices = ramp_vertex * 2;
    let rgi_offset = (ramp_vertex - 1) * 6;
    let num_indices = rgi_offset * 3; // floor + right wall + left wall

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
        num_vertices * 3
    ];
    let mut indices: Vec<u32> = vec![0; num_indices];

    let has_image = !ramp.image.is_empty();

    // Generate floor vertices
    for i in 0..ramp_vertex {
        let offset = i * 2;
        vertices[offset].x = rgv_local[i].x;
        vertices[offset].y = rgv_local[i].y;
        vertices[offset].z = rgheight[i];

        vertices[offset + 1].x = rgv_local[ramp_vertex * 2 - i - 1].x;
        vertices[offset + 1].y = rgv_local[ramp_vertex * 2 - i - 1].y;
        vertices[offset + 1].z = rgheight[i];

        if has_image {
            // Use ramp-aligned texture coordinates
            vertices[offset].tu = 1.0;
            vertices[offset].tv = rgratio[i];
            vertices[offset + 1].tu = 0.0;
            vertices[offset + 1].tv = rgratio[i];
        }

        if i < ramp_vertex - 1 {
            // Floor indices
            let idx_offset = i * 6;
            indices[idx_offset] = (i * 2) as u32;
            indices[idx_offset + 1] = (i * 2 + 1) as u32;
            indices[idx_offset + 2] = (i * 2 + 3) as u32;
            indices[idx_offset + 3] = (i * 2) as u32;
            indices[idx_offset + 4] = (i * 2 + 3) as u32;
            indices[idx_offset + 5] = (i * 2 + 2) as u32;

            // Right wall indices
            let idx_offset_right = rgi_offset + i * 6;
            indices[idx_offset_right] = (i * 2 + num_vertices) as u32;
            indices[idx_offset_right + 1] = (i * 2 + num_vertices + 1) as u32;
            indices[idx_offset_right + 2] = (i * 2 + num_vertices + 3) as u32;
            indices[idx_offset_right + 3] = (i * 2 + num_vertices) as u32;
            indices[idx_offset_right + 4] = (i * 2 + num_vertices + 3) as u32;
            indices[idx_offset_right + 5] = (i * 2 + num_vertices + 2) as u32;

            // Left wall indices
            let idx_offset_left = rgi_offset * 2 + i * 6;
            indices[idx_offset_left] = (i * 2 + num_vertices * 2) as u32;
            indices[idx_offset_left + 1] = (i * 2 + num_vertices * 2 + 1) as u32;
            indices[idx_offset_left + 2] = (i * 2 + num_vertices * 2 + 3) as u32;
            indices[idx_offset_left + 3] = (i * 2 + num_vertices * 2) as u32;
            indices[idx_offset_left + 4] = (i * 2 + num_vertices * 2 + 3) as u32;
            indices[idx_offset_left + 5] = (i * 2 + num_vertices * 2 + 2) as u32;
        }
    }

    // Compute normals for floor
    compute_normals(&mut vertices[..num_vertices], &indices[..rgi_offset]);

    // Copy floor vertices to output buffer (offset 0)
    // Vertices are already in place

    // Generate right wall vertices (if visible)
    if ramp.right_wall_height_visible > 0.0 || ramp.left_wall_height_visible > 0.0 {
        // Right wall
        for i in 0..ramp_vertex {
            let offset = num_vertices + i * 2;
            vertices[offset].x = rgv_local[i].x;
            vertices[offset].y = rgv_local[i].y;
            vertices[offset].z = rgheight[i];

            vertices[offset + 1].x = rgv_local[i].x;
            vertices[offset + 1].y = rgv_local[i].y;
            vertices[offset + 1].z = rgheight[i] + ramp.right_wall_height_visible;

            if has_image && ramp.image_walls {
                vertices[offset].tu = 0.0;
                vertices[offset].tv = rgratio[i];
                vertices[offset + 1].tu = 0.0;
                vertices[offset + 1].tv = rgratio[i];
            }
        }
        compute_normals(
            &mut vertices[num_vertices..num_vertices * 2],
            &indices[..rgi_offset],
        );

        // Left wall
        for i in 0..ramp_vertex {
            let offset = num_vertices * 2 + i * 2;
            vertices[offset].x = rgv_local[ramp_vertex * 2 - i - 1].x;
            vertices[offset].y = rgv_local[ramp_vertex * 2 - i - 1].y;
            vertices[offset].z = rgheight[i];

            vertices[offset + 1].x = rgv_local[ramp_vertex * 2 - i - 1].x;
            vertices[offset + 1].y = rgv_local[ramp_vertex * 2 - i - 1].y;
            vertices[offset + 1].z = rgheight[i] + ramp.left_wall_height_visible;

            if has_image && ramp.image_walls {
                vertices[offset].tu = 0.0;
                vertices[offset].tv = rgratio[i];
                vertices[offset + 1].tu = 0.0;
                vertices[offset + 1].tv = rgratio[i];
            }
        }
        compute_normals(
            &mut vertices[num_vertices * 2..num_vertices * 3],
            &indices[..rgi_offset],
        );
    }

    // Determine which parts to include based on visibility
    let include_floor = true; // Floor is always included for flat ramps
    let include_right = ramp.right_wall_height_visible > 0.0;
    let include_left = ramp.left_wall_height_visible > 0.0;

    // Build final vertex and index lists
    let mut final_vertices = Vec::new();
    let mut final_indices = Vec::new();

    if include_floor {
        let base = final_vertices.len() as u32;
        for v in &vertices[..num_vertices] {
            final_vertices.push((*v).clone());
        }
        for &idx in &indices[..rgi_offset] {
            final_indices.push(base + idx);
        }
    }

    if include_right && include_left {
        let base = final_vertices.len() as u32;
        for v in &vertices[num_vertices..num_vertices * 2] {
            final_vertices.push((*v).clone());
        }
        for i in 0..rgi_offset {
            final_indices.push(base + indices[rgi_offset + i] - num_vertices as u32);
        }

        let base = final_vertices.len() as u32;
        for v in &vertices[num_vertices * 2..num_vertices * 3] {
            final_vertices.push((*v).clone());
        }
        for i in 0..rgi_offset {
            final_indices.push(base + indices[rgi_offset * 2 + i] - (num_vertices * 2) as u32);
        }
    } else if include_right {
        let base = final_vertices.len() as u32;
        for v in &vertices[num_vertices..num_vertices * 2] {
            final_vertices.push((*v).clone());
        }
        for i in 0..rgi_offset {
            final_indices.push(base + indices[rgi_offset + i] - num_vertices as u32);
        }
    } else if include_left {
        let base = final_vertices.len() as u32;
        for v in &vertices[num_vertices * 2..num_vertices * 3] {
            final_vertices.push((*v).clone());
        }
        for i in 0..rgi_offset {
            final_indices.push(base + indices[rgi_offset * 2 + i] - (num_vertices * 2) as u32);
        }
    }

    if final_vertices.is_empty() || final_indices.is_empty() {
        return None;
    }

    let wrapped = final_vertices
        .into_iter()
        .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
        .collect();

    let faces = final_indices
        .chunks_exact(3)
        .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
        .collect();

    Some((wrapped, faces))
}

/// Compute normals for a mesh
fn compute_normals(vertices: &mut [Vertex3dNoTex2], indices: &[u32]) {
    // Reset all normals
    for v in vertices.iter_mut() {
        v.nx = 0.0;
        v.ny = 0.0;
        v.nz = 0.0;
    }

    // Accumulate face normals
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

    // Normalize
    for v in vertices.iter_mut() {
        let len = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();
        if len > 0.0 {
            v.nx /= len;
            v.ny /= len;
            v.nz /= len;
        }
    }
}

/// Create a wire mesh for wire ramps
fn create_wire(
    ramp: &Ramp,
    num_rings: usize,
    num_segments: usize,
    mid_points: &[Vec2],
    heights: &[f32],
) -> Vec<Vertex3dNoTex2> {
    let mut vertices = vec![
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
        num_rings * num_segments
    ];

    let mut prev_binorm = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    let inv_num_rings = 1.0 / num_rings as f32;
    let inv_num_segments = 1.0 / num_segments as f32;

    for i in 0..num_rings {
        let i2 = if i == num_rings - 1 { i } else { i + 1 };
        let height = heights[i];

        let tangent = if i == num_rings - 1 && i > 0 {
            Vec3 {
                x: mid_points[i].x - mid_points[i - 1].x,
                y: mid_points[i].y - mid_points[i - 1].y,
                z: heights[i] - heights[i - 1],
            }
        } else {
            Vec3 {
                x: mid_points[i2].x - mid_points[i].x,
                y: mid_points[i2].y - mid_points[i].y,
                z: heights[i2] - height,
            }
        };

        let (normal, binorm) = if i == 0 {
            let up = Vec3 {
                x: mid_points[i2].x + mid_points[i].x,
                y: mid_points[i2].y + mid_points[i].y,
                z: heights[i2] - height,
            };
            let normal = Vec3::cross(&tangent, &up);
            let binorm = Vec3::cross(&tangent, &normal);
            (normal, binorm)
        } else {
            let normal = Vec3::cross(&prev_binorm, &tangent);
            let binorm = Vec3::cross(&tangent, &normal);
            (normal, binorm)
        };

        let binorm = binorm.normalize();
        let normal = normal.normalize();
        prev_binorm = binorm;

        let u = i as f32 * inv_num_rings;
        for j in 0..num_segments {
            let index = i * num_segments + j;
            let v = (j as f32 + u) * inv_num_segments;
            let angle = j as f32 * 360.0 * inv_num_segments;

            let tmp = get_rotated_axis(angle, &tangent, &normal) * (ramp.wire_diameter * 0.5);

            vertices[index].x = mid_points[i].x + tmp.x;
            vertices[index].y = mid_points[i].y + tmp.y;
            vertices[index].z = height + tmp.z;
            vertices[index].tu = u;
            vertices[index].tv = v;

            // Normal points outward from center
            let n = Vec3 {
                x: vertices[index].x - mid_points[i].x,
                y: vertices[index].y - mid_points[i].y,
                z: vertices[index].z - height,
            };
            let len = n.length();
            if len > 0.0 {
                vertices[index].nx = n.x / len;
                vertices[index].ny = n.y / len;
                vertices[index].nz = n.z / len;
            }
        }
    }

    vertices
}

/// Rotate a vector around an axis
fn get_rotated_axis(angle_degrees: f32, axis: &Vec3, temp: &Vec3) -> Vec3 {
    let u = axis.normalize();
    let angle_rad = angle_degrees * PI / 180.0;
    let sin_angle = angle_rad.sin();
    let cos_angle = angle_rad.cos();
    let one_minus_cos = 1.0 - cos_angle;

    let rot_row0 = Vec3 {
        x: u.x * u.x + cos_angle * (1.0 - u.x * u.x),
        y: u.x * u.y * one_minus_cos - sin_angle * u.z,
        z: u.x * u.z * one_minus_cos + sin_angle * u.y,
    };
    let rot_row1 = Vec3 {
        x: u.x * u.y * one_minus_cos + sin_angle * u.z,
        y: u.y * u.y + cos_angle * (1.0 - u.y * u.y),
        z: u.y * u.z * one_minus_cos - sin_angle * u.x,
    };
    let rot_row2 = Vec3 {
        x: u.x * u.z * one_minus_cos - sin_angle * u.y,
        y: u.y * u.z * one_minus_cos + sin_angle * u.x,
        z: u.z * u.z + cos_angle * (1.0 - u.z * u.z),
    };

    Vec3 {
        x: temp.x * rot_row0.x + temp.y * rot_row0.y + temp.z * rot_row0.z,
        y: temp.x * rot_row1.x + temp.y * rot_row1.y + temp.z * rot_row1.z,
        z: temp.x * rot_row2.x + temp.y * rot_row2.y + temp.z * rot_row2.z,
    }
}

/// Generate wire ramp mesh
fn build_wire_ramp_mesh(
    ramp: &Ramp,
    vvertex: &[RenderVertex3D],
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    let (rgv_local, rgheight, _) = get_ramp_vertex(ramp, vvertex, false);
    let num_rings = vvertex.len();

    if num_rings < 2 {
        return None;
    }

    // Determine accuracy/segments based on detail level (use 8 as default)
    let num_segments = 8;

    // Get middle points (center of ramp)
    let mut mid_points: Vec<Vec2> = Vec::with_capacity(num_rings);
    for i in 0..num_rings {
        let left_idx = num_rings * 2 - i - 1;
        mid_points.push(Vec2 {
            x: (rgv_local[i].x + rgv_local[left_idx].x) * 0.5,
            y: (rgv_local[i].y + rgv_local[left_idx].y) * 0.5,
        });
    }

    // Get left side points (reversed)
    let mut left_points: Vec<Vec2> = Vec::with_capacity(num_rings);
    for i in 0..num_rings {
        left_points.push(rgv_local[num_rings * 2 - i - 1]);
    }

    let num_vertices_per_wire = num_rings * num_segments;
    let num_indices_per_wire = 6 * (num_rings - 1) * num_segments;

    // Generate wire indices (same for all wires)
    let mut wire_indices: Vec<u32> = vec![0; num_indices_per_wire];
    for i in 0..(num_rings - 1) {
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
            wire_indices[offs] = quad[0];
            wire_indices[offs + 1] = quad[1];
            wire_indices[offs + 2] = quad[2];
            wire_indices[offs + 3] = quad[3];
            wire_indices[offs + 4] = quad[2];
            wire_indices[offs + 5] = quad[1];
        }
    }

    // Build mesh based on ramp type
    let (final_vertices, final_indices) = match ramp.ramp_type {
        RampType::OneWire => {
            let vertices = create_wire(ramp, num_rings, num_segments, &mid_points, &rgheight);
            (vertices, wire_indices)
        }
        RampType::TwoWire => {
            let right_wire = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );
            let left_wire = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);

            let mut vertices = Vec::with_capacity(num_vertices_per_wire * 2);
            for mut v in right_wire {
                v.z += 3.0; // Raise wire
                vertices.push(v);
            }
            for mut v in left_wire {
                v.z += 3.0;
                vertices.push(v);
            }

            let mut indices = Vec::with_capacity(num_indices_per_wire * 2);
            indices.extend_from_slice(&wire_indices);
            for &idx in &wire_indices {
                indices.push(idx + num_vertices_per_wire as u32);
            }

            (vertices, indices)
        }
        RampType::ThreeWireLeft => {
            let right_wire = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );
            let left_wire = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);
            let upper_left = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);

            let mut vertices = Vec::with_capacity(num_vertices_per_wire * 3);
            for mut v in right_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in left_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in upper_left {
                v.z += ramp.wire_distance_y * 0.5;
                vertices.push(v);
            }

            let mut indices = Vec::with_capacity(num_indices_per_wire * 3);
            indices.extend_from_slice(&wire_indices);
            for &idx in &wire_indices {
                indices.push(idx + num_vertices_per_wire as u32);
            }
            for &idx in &wire_indices {
                indices.push(idx + (num_vertices_per_wire * 2) as u32);
            }

            (vertices, indices)
        }
        RampType::ThreeWireRight => {
            let right_wire = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );
            let left_wire = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);
            let upper_right = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );

            let mut vertices = Vec::with_capacity(num_vertices_per_wire * 3);
            for mut v in right_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in left_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in upper_right {
                v.z += ramp.wire_distance_y * 0.5;
                vertices.push(v);
            }

            let mut indices = Vec::with_capacity(num_indices_per_wire * 3);
            indices.extend_from_slice(&wire_indices);
            for &idx in &wire_indices {
                indices.push(idx + num_vertices_per_wire as u32);
            }
            for &idx in &wire_indices {
                indices.push(idx + (num_vertices_per_wire * 2) as u32);
            }

            (vertices, indices)
        }
        RampType::FourWire => {
            let right_wire = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );
            let left_wire = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);
            let upper_right = create_wire(
                ramp,
                num_rings,
                num_segments,
                &rgv_local[..num_rings],
                &rgheight,
            );
            let upper_left = create_wire(ramp, num_rings, num_segments, &left_points, &rgheight);

            let mut vertices = Vec::with_capacity(num_vertices_per_wire * 4);
            for mut v in right_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in left_wire {
                v.z += 3.0;
                vertices.push(v);
            }
            for mut v in upper_right {
                v.z += ramp.wire_distance_y * 0.5;
                vertices.push(v);
            }
            for mut v in upper_left {
                v.z += ramp.wire_distance_y * 0.5;
                vertices.push(v);
            }

            let mut indices = Vec::with_capacity(num_indices_per_wire * 4);
            indices.extend_from_slice(&wire_indices);
            for &idx in &wire_indices {
                indices.push(idx + num_vertices_per_wire as u32);
            }
            for &idx in &wire_indices {
                indices.push(idx + (num_vertices_per_wire * 2) as u32);
            }
            for &idx in &wire_indices {
                indices.push(idx + (num_vertices_per_wire * 3) as u32);
            }

            (vertices, indices)
        }
        RampType::Flat => {
            // This shouldn't happen as we handle flat ramps separately
            return None;
        }
    };

    if final_vertices.is_empty() || final_indices.is_empty() {
        return None;
    }

    let wrapped = final_vertices
        .into_iter()
        .map(|vertex| VertexWrapper::new(vertex.to_vpx_bytes(), vertex))
        .collect();

    let faces = final_indices
        .chunks_exact(3)
        .map(|tri| VpxFace::new(tri[0] as i64, tri[1] as i64, tri[2] as i64))
        .collect();

    Some((wrapped, faces))
}

/// Build the complete ramp mesh
fn build_ramp_mesh(ramp: &Ramp) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    // Generate meshes for all ramps, including invisible ones
    // This is useful for tools that need to visualize or process all geometry

    if ramp.width_bottom == 0.0 && ramp.width_top == 0.0 {
        return None;
    }

    // Use accuracy = 4.0 (highest detail)
    let accuracy = 4.0;
    let vvertex = get_central_curve(&ramp.drag_points, accuracy);

    if vvertex.len() < 2 {
        return None;
    }

    if is_habitrail(ramp) {
        build_wire_ramp_mesh(ramp, &vvertex)
    } else {
        build_flat_ramp_mesh(ramp, &vvertex)
    }
}

/// Write ramp meshes to file
pub(super) fn write_ramp_meshes(
    gameitems_dir: &Path,
    ramp: &Ramp,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_ramp_mesh(ramp) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(&mesh_path, &ramp.name, &vertices, &indices, mesh_format, fs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catmull_curve() {
        let v0 = RenderVertex3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            smooth: false,
            control_point: true,
        };
        let v1 = RenderVertex3D {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            smooth: false,
            control_point: true,
        };
        let v2 = RenderVertex3D {
            x: 2.0,
            y: 1.0,
            z: 0.0,
            smooth: false,
            control_point: true,
        };
        let v3 = RenderVertex3D {
            x: 3.0,
            y: 1.0,
            z: 0.0,
            smooth: false,
            control_point: true,
        };

        let curve = CatmullCurve3D::new(&v0, &v1, &v2, &v3);
        let (x, y, z) = curve.get_point_at(0.0);
        assert!((x - 1.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
        assert!((z - 0.0).abs() < 0.01);

        let (x, y, z) = curve.get_point_at(1.0);
        assert!((x - 2.0).abs() < 0.01);
        assert!((y - 1.0).abs() < 0.01);
        assert!((z - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_simple_ramp() {
        let mut ramp = Ramp::default();
        ramp.drag_points = vec![
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
        ];

        let result = build_ramp_mesh(&ramp);
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }
}
