//! Ramp mesh generation for expanded VPX export
//!
//! This module ports the ramp mesh generation from Visual Pinball's ramp.cpp.
//! Ramps can be either flat (with optional walls) or wire ramps (1-4 wire types).

use super::mesh_common::{
    RenderVertex3D, TableDimensions, Vec2, Vec3, compute_normals, detail_level_to_accuracy,
    generated_mesh_file_name, get_rotated_axis, init_nonuniform_catmull_coeffs, write_mesh_to_file,
};
use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::dragpoint::DragPoint;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::ramp::{Ramp, RampType};
use crate::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::path::Path;

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
        ..Default::default()
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
            ..Default::default()
        };
        let v1 = RenderVertex3D {
            x: pdp1.x,
            y: pdp1.y,
            z: pdp1.z,
            smooth: pdp1.smooth,
            control_point: true,
            ..Default::default()
        };
        let v2 = RenderVertex3D {
            x: pdp2.x,
            y: pdp2.y,
            z: pdp2.z,
            smooth: pdp2.smooth,
            control_point: true,
            ..Default::default()
        };
        let v3 = RenderVertex3D {
            x: pdp3.x,
            y: pdp3.y,
            z: pdp3.z,
            smooth: pdp3.smooth,
            control_point: true,
            ..Default::default()
        };

        let cc = CatmullCurve3D::new(&v0, &v1, &v2, &v3);

        let rendv1 = RenderVertex3D {
            x: v1.x,
            y: v1.y,
            z: v1.z,
            smooth: pdp1.smooth,
            control_point: true,
            ..Default::default()
        };

        let rendv2 = RenderVertex3D {
            x: v2.x,
            y: v2.y,
            z: v2.z,
            smooth: pdp2.smooth,
            control_point: true,
            ..Default::default()
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
            ..Default::default()
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
    table_dims: &TableDimensions,
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

    // For world-aligned textures, we need to normalize coordinates to table dimensions
    // VPinball: rgv3D[0].tu = rgv3D[0].x * inv_tablewidth
    //           rgv3D[0].tv = rgv3D[0].y * inv_tableheight
    let inv_table_width = 1.0 / (table_dims.right - table_dims.left);
    let inv_table_height = 1.0 / (table_dims.bottom - table_dims.top);
    let use_world_coords = ramp.image_alignment == RampImageAlignment::World;

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
            if use_world_coords {
                // World-aligned texture coordinates (VPinball ramp.cpp line 2175-2180)
                vertices[offset].tu = vertices[offset].x * inv_table_width;
                vertices[offset].tv = vertices[offset].y * inv_table_height;
                vertices[offset + 1].tu = vertices[offset + 1].x * inv_table_width;
                vertices[offset + 1].tv = vertices[offset + 1].y * inv_table_height;
            } else {
                // Ramp-aligned (wrap) texture coordinates
                vertices[offset].tu = 1.0;
                vertices[offset].tv = rgratio[i];
                vertices[offset + 1].tu = 0.0;
                vertices[offset + 1].tv = rgratio[i];
            }
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
pub(super) fn build_ramp_mesh(
    ramp: &Ramp,
    table_dims: &TableDimensions,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    // Generate meshes for all ramps, including invisible ones
    // This is useful for tools that need to visualize or process all geometry

    if ramp.width_bottom == 0.0 && ramp.width_top == 0.0 {
        return None;
    }

    // From VPinball mesh.h GetRgVertex: accuracy = 4.0 is highest detail level
    // detail_level_to_accuracy(10.0) = 4.0
    let accuracy = detail_level_to_accuracy(10.0);
    let vvertex = get_central_curve(&ramp.drag_points, accuracy);

    if vvertex.len() < 2 {
        return None;
    }

    if is_habitrail(ramp) {
        build_wire_ramp_mesh(ramp, &vvertex)
    } else {
        build_flat_ramp_mesh(ramp, &vvertex, table_dims)
    }
}

/// Write ramp meshes to file
pub(super) fn write_ramp_meshes(
    gameitems_dir: &Path,
    ramp: &Ramp,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    table_dims: &TableDimensions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_ramp_mesh(ramp, table_dims) else {
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
            ..Default::default()
        };
        let v1 = RenderVertex3D {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };
        let v2 = RenderVertex3D {
            x: 2.0,
            y: 1.0,
            z: 0.0,
            smooth: false,
            control_point: true,
            ..Default::default()
        };
        let v3 = RenderVertex3D {
            x: 3.0,
            y: 1.0,
            z: 0.0,
            smooth: false,
            control_point: true,
            ..Default::default()
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

        let result = build_ramp_mesh(&ramp, &TableDimensions::new(0.0, 0.0, 1000.0, 2000.0));
        assert!(result.is_some());

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
    }
}
