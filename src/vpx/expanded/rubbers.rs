//! Rubber mesh generation for expanded VPX export
//!
//! This module ports the rubber mesh generation from Visual Pinball's rubber.cpp.
//! Rubbers are rendered as tubular shapes that follow a spline curve defined by drag points.

use super::mesh_common::{
    Vec2, Vec3, compute_normals, detail_level_to_accuracy, generated_mesh_file_name,
    get_rg_vertex_2d, get_rotated_axis, write_mesh_to_file,
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

/// Get the spline vertices for the rubber outline
/// Returns (outline_vertices, middle_points) where:
/// - outline_vertices: 2D outline of the rubber (right side forward, left side backward)
/// - middle_points: the center points of the curve
fn get_spline_vertex(
    drag_points: &[DragPoint],
    thickness: f32,
    accuracy: f32,
) -> (Vec<Vec2>, Vec<Vec2>) {
    // Rubbers always loop
    let vvertex = get_rg_vertex_2d(drag_points, accuracy, true);
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
    // From VPinball rubber.cpp GetCentralCurve():
    // accuracy = 4.0f * powf(10.0f, (10.0f - accuracy) * (1.0f / 1.5f))
    // where detail_level=10 gives 4.0 (highest detail), detail_level=0 gives ~18,000,000 (lowest detail)
    // We use the highest detail level (10) which gives accuracy = 4.0
    let accuracy = detail_level_to_accuracy(10.0);

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
pub(super) fn build_rubber_mesh(rubber: &Rubber) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
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
    use crate::vpx::expanded::mesh_common::{CatmullCurve2D, RenderVertex2D};

    /// Generate octagon drag points for testing
    fn create_octagon_drag_points(radius: f32, smooth: bool) -> Vec<DragPoint> {
        let num_points = 8;
        (0..num_points)
            .map(|i| {
                let angle = (i as f32 / num_points as f32) * 2.0 * PI;
                DragPoint {
                    x: radius * angle.cos(),
                    y: radius * angle.sin(),
                    z: 0.0,
                    smooth,
                    ..Default::default()
                }
            })
            .collect()
    }

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
    fn test_rubber_octagon_smooth_corners() {
        // Create an octagon shape with all smooth points
        // This should produce a smooth, rounded rubber mesh
        let drag_points = create_octagon_drag_points(100.0, true);

        let mut rubber = Rubber::default();
        rubber.thickness = 8;
        rubber.height = 25.0;
        rubber.drag_points = drag_points;

        let result = build_rubber_mesh(&rubber);
        assert!(result.is_some(), "Octagon rubber mesh should be generated");

        let (vertices, indices) = result.unwrap();
        assert!(!vertices.is_empty(), "Should have vertices");
        assert!(!indices.is_empty(), "Should have faces");

        // Extract vertex data for analysis
        let verts: Vec<_> = vertices.iter().map(|v| &v.vertex).collect();

        // Check that all normals are normalized
        for (i, v) in verts.iter().enumerate() {
            let len = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();
            assert!(
                (len - 1.0).abs() < 0.01,
                "Normal at vertex {} should be normalized, got length {}",
                i,
                len
            );
        }

        // For a smooth rubber, adjacent vertices should have similar normals
        // (indicating smooth shading, not flat shading with hard edges)
        // We check vertices that are on the same ring (same position along the rubber)
        let num_segments = 8; // Cross-section segments

        // Check the first ring of vertices
        if verts.len() >= num_segments * 2 {
            for j in 0..num_segments {
                let v1 = &verts[j];
                let v2 = &verts[(j + 1) % num_segments];

                // Calculate dot product of normals
                let dot = v1.nx * v2.nx + v1.ny * v2.ny + v1.nz * v2.nz;

                // For a smooth circular cross-section, adjacent normals should have
                // a reasonable angle between them (dot product > 0.7 for ~45 degrees)
                assert!(
                    dot > 0.5,
                    "Adjacent normals on ring should be similar for smooth shading, got dot={}",
                    dot
                );
            }
        }

        // Verify seam continuity - the last ring should connect smoothly back to the first
        let num_rings = verts.len() / num_segments;
        if num_rings >= 2 {
            // Compare normals at the seam (last ring connects to first ring)
            let last_ring_start = (num_rings - 1) * num_segments;
            for j in 0..num_segments {
                let v_last = &verts[last_ring_start + j];
                let v_first = &verts[j];

                let dot = v_last.nx * v_first.nx + v_last.ny * v_first.ny + v_last.nz * v_first.nz;

                // The seam normals should be fairly similar for a smooth loop
                // Allow more tolerance here as the geometry changes around the curve
                assert!(
                    dot > 0.3,
                    "Seam normals should be continuous, got dot={} at segment {}",
                    dot,
                    j
                );
            }
            println!("Seam continuity check passed for {} rings", num_rings);
        }

        // Print some stats for debugging
        println!(
            "Octagon rubber: {} vertices, {} faces",
            vertices.len(),
            indices.len()
        );
    }

    #[test]
    fn test_rubber_octagon_sharp_corners() {
        // Create an octagon with NON-smooth points
        // This should produce sharper corners in the curve
        let drag_points = create_octagon_drag_points(100.0, false);

        let mut rubber = Rubber::default();
        rubber.thickness = 8;
        rubber.height = 25.0;
        rubber.drag_points = drag_points;

        let result = build_rubber_mesh(&rubber);
        assert!(
            result.is_some(),
            "Sharp octagon rubber mesh should be generated"
        );

        let (vertices, indices) = result.unwrap();

        // Sharp corners should generate fewer vertices since less subdivision
        println!(
            "Sharp octagon rubber: {} vertices, {} faces",
            vertices.len(),
            indices.len()
        );
    }

    #[test]
    fn test_rubber_ring003_from_table() {
        // This is the actual Ring003 rubber from example_with_balls.vpx
        // It has 8 drag points in a circle but should render as a smooth ring
        let mut rubber = Rubber::default();
        rubber.height = 102.0;
        rubber.hit_height = Some(25.0);
        rubber.thickness = 8;
        rubber.static_rendering = true;
        rubber.rot_x = 90.0;
        rubber.rot_y = 0.0;
        rubber.rot_z = 0.0;
        rubber.drag_points = vec![
            DragPoint {
                x: 50.00001,
                y: 910.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 21.715734,
                y: 921.7157,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 10.0,
                y: 950.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 21.715723,
                y: 978.2843,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 50.0,
                y: 990.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 78.28428,
                y: 978.2843,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 90.0,
                y: 950.0,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
            DragPoint {
                x: 78.28428,
                y: 921.7157,
                z: 0.0,
                smooth: true,
                ..Default::default()
            },
        ];

        let result = build_rubber_mesh(&rubber);
        assert!(result.is_some(), "Ring003 rubber mesh should be generated");

        let (vertices, indices) = result.unwrap();
        let num_segments = 8; // Cross-section segments
        let num_rings = vertices.len() / num_segments;

        println!("Ring003 rubber:");
        println!("  Drag points: {}", rubber.drag_points.len());
        println!("  Generated rings: {}", num_rings);
        println!("  Total vertices: {}", vertices.len());
        println!("  Total faces: {}", indices.len());

        // For a smooth ring with 8 drag points and high detail,
        // we should have MANY more rings than drag points due to spline subdivision
        // If we only get 8 rings, the spline is not subdividing
        assert!(
            num_rings > 8,
            "Should have more rings ({}) than drag points (8) for smooth subdivision",
            num_rings
        );

        // A truly smooth ring should have ~32+ rings for 8 control points
        // at the highest detail level
        assert!(
            num_rings >= 24,
            "Expected at least 24 rings for a smooth ring, got {}",
            num_rings
        );
    }
}
