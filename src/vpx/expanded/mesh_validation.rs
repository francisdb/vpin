//! Mesh validation utilities
//!
//! This module provides functions to validate generated meshes for common issues:
//! - Consistent winding order
//! - Valid normals (non-zero, normalized)
//! - Watertight meshes (no holes/non-manifold edges)
//! - Valid indices (within bounds)

use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::obj::VpxFace;
use std::collections::HashMap;

/// Result of mesh validation
#[derive(Debug, Default)]
pub struct MeshValidationResult {
    /// Total number of vertices
    pub vertex_count: usize,
    /// Total number of faces (triangles)
    pub face_count: usize,
    /// Indices that are out of bounds
    pub invalid_indices: Vec<(usize, i64)>,
    /// Vertices with zero-length normals
    pub zero_normals: Vec<usize>,
    /// Vertices with non-normalized normals (length != 1.0)
    pub non_unit_normals: Vec<(usize, f32)>,
    /// Degenerate triangles (zero area)
    pub degenerate_faces: Vec<usize>,
    /// Edges that appear only once (holes in the mesh)
    pub boundary_edges: Vec<(i64, i64)>,
    /// Edges that appear more than twice (non-manifold)
    pub non_manifold_edges: Vec<(i64, i64)>,
    /// Faces with inconsistent winding (based on edge direction analysis)
    pub inconsistent_winding_faces: Vec<usize>,
}

impl MeshValidationResult {
    /// Returns true if the mesh has no critical issues (valid indices and no degenerate faces)
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.invalid_indices.is_empty()
            && self.zero_normals.is_empty()
            && self.degenerate_faces.is_empty()
    }

    /// Returns true if the mesh is watertight (no boundary edges)
    #[allow(dead_code)]
    pub fn is_watertight(&self) -> bool {
        self.boundary_edges.is_empty() && self.non_manifold_edges.is_empty()
    }

    /// Returns true if all normals are valid (non-zero and approximately unit length)
    #[allow(dead_code)]
    pub fn has_valid_normals(&self) -> bool {
        self.zero_normals.is_empty() && self.non_unit_normals.is_empty()
    }

    /// Returns a human-readable summary of validation issues
    #[allow(dead_code)]
    pub fn summary(&self) -> String {
        let mut issues = Vec::new();

        if !self.invalid_indices.is_empty() {
            issues.push(format!(
                "{} invalid indices (out of bounds)",
                self.invalid_indices.len()
            ));
        }
        if !self.zero_normals.is_empty() {
            issues.push(format!("{} zero-length normals", self.zero_normals.len()));
        }
        if !self.non_unit_normals.is_empty() {
            issues.push(format!("{} non-unit normals", self.non_unit_normals.len()));
        }
        if !self.degenerate_faces.is_empty() {
            issues.push(format!(
                "{} degenerate triangles",
                self.degenerate_faces.len()
            ));
        }
        if !self.boundary_edges.is_empty() {
            issues.push(format!(
                "{} boundary edges (holes)",
                self.boundary_edges.len()
            ));
        }
        if !self.non_manifold_edges.is_empty() {
            issues.push(format!(
                "{} non-manifold edges",
                self.non_manifold_edges.len()
            ));
        }
        if !self.inconsistent_winding_faces.is_empty() {
            issues.push(format!(
                "{} faces with inconsistent winding",
                self.inconsistent_winding_faces.len()
            ));
        }

        if issues.is_empty() {
            format!(
                "Mesh valid: {} vertices, {} faces",
                self.vertex_count, self.face_count
            )
        } else {
            format!(
                "Mesh issues ({} vertices, {} faces): {}",
                self.vertex_count,
                self.face_count,
                issues.join(", ")
            )
        }
    }
}

/// Validate a mesh for common issues
///
/// # Arguments
/// * `vertices` - The mesh vertices
/// * `faces` - The mesh faces (triangles)
///
/// # Returns
/// A `MeshValidationResult` containing any issues found
#[allow(dead_code)]
pub fn validate_mesh(vertices: &[VertexWrapper], faces: &[VpxFace]) -> MeshValidationResult {
    let mut result = MeshValidationResult {
        vertex_count: vertices.len(),
        face_count: faces.len(),
        ..Default::default()
    };

    // Check for invalid indices
    for (face_idx, face) in faces.iter().enumerate() {
        let vertex_count = vertices.len() as i64;
        if face.i0 < 0 || face.i0 >= vertex_count {
            result.invalid_indices.push((face_idx, face.i0));
        }
        if face.i1 < 0 || face.i1 >= vertex_count {
            result.invalid_indices.push((face_idx, face.i1));
        }
        if face.i2 < 0 || face.i2 >= vertex_count {
            result.invalid_indices.push((face_idx, face.i2));
        }
    }

    // Check normals
    const EPSILON: f32 = 0.0001;
    const UNIT_TOLERANCE: f32 = 0.01;

    for (idx, wrapper) in vertices.iter().enumerate() {
        let v = &wrapper.vertex;
        let normal_length = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();

        if normal_length < EPSILON {
            result.zero_normals.push(idx);
        } else if (normal_length - 1.0).abs() > UNIT_TOLERANCE {
            result.non_unit_normals.push((idx, normal_length));
        }
    }

    // Check for degenerate triangles (if indices are valid)
    if result.invalid_indices.is_empty() {
        for (face_idx, face) in faces.iter().enumerate() {
            let v0 = &vertices[face.i0 as usize].vertex;
            let v1 = &vertices[face.i1 as usize].vertex;
            let v2 = &vertices[face.i2 as usize].vertex;

            // Check if triangle has zero area using cross product
            let e1 = (v1.x - v0.x, v1.y - v0.y, v1.z - v0.z);
            let e2 = (v2.x - v0.x, v2.y - v0.y, v2.z - v0.z);

            let cross = (
                e1.1 * e2.2 - e1.2 * e2.1,
                e1.2 * e2.0 - e1.0 * e2.2,
                e1.0 * e2.1 - e1.1 * e2.0,
            );

            let area_squared = cross.0 * cross.0 + cross.1 * cross.1 + cross.2 * cross.2;
            if area_squared < EPSILON * EPSILON {
                result.degenerate_faces.push(face_idx);
            }
        }
    }

    // Check edge topology (for watertightness and manifold property)
    if result.invalid_indices.is_empty() {
        let mut edge_counts: HashMap<(i64, i64), i32> = HashMap::new();

        for face in faces {
            // Add edges with consistent ordering (smaller index first for undirected edge)
            let edges = [
                (face.i0.min(face.i1), face.i0.max(face.i1)),
                (face.i1.min(face.i2), face.i1.max(face.i2)),
                (face.i2.min(face.i0), face.i2.max(face.i0)),
            ];

            for edge in edges {
                *edge_counts.entry(edge).or_insert(0) += 1;
            }
        }

        for (edge, count) in edge_counts {
            if count == 1 {
                result.boundary_edges.push(edge);
            } else if count > 2 {
                result.non_manifold_edges.push(edge);
            }
        }
    }

    // Check winding consistency using directed edges
    if result.invalid_indices.is_empty() && result.boundary_edges.is_empty() {
        let mut directed_edge_faces: HashMap<(i64, i64), Vec<usize>> = HashMap::new();

        for (face_idx, face) in faces.iter().enumerate() {
            // Store directed edges (order matters for winding)
            let edges = [(face.i0, face.i1), (face.i1, face.i2), (face.i2, face.i0)];

            for edge in edges {
                directed_edge_faces.entry(edge).or_default().push(face_idx);
            }
        }

        // For a consistent mesh, each directed edge should appear exactly once,
        // and its reverse should also appear exactly once (for closed meshes)
        for ((v0, v1), face_indices) in &directed_edge_faces {
            if face_indices.len() > 1 {
                // Multiple faces share the same directed edge - inconsistent winding
                for &face_idx in face_indices {
                    if !result.inconsistent_winding_faces.contains(&face_idx) {
                        result.inconsistent_winding_faces.push(face_idx);
                    }
                }
            }

            // Check if reverse edge exists (for closed mesh)
            let reverse = (*v1, *v0);
            if !directed_edge_faces.contains_key(&reverse)
                && !result
                    .boundary_edges
                    .contains(&((*v0).min(*v1), (*v0).max(*v1)))
            {
                // This is a boundary edge that wasn't detected earlier
                // (shouldn't happen if we detected boundaries correctly)
            }
        }
    }

    result
}

/// Validate that face normals match vertex normals (approximately)
///
/// This checks if the vertex normals are consistent with the geometric normals
/// computed from the face vertices.
///
/// # Arguments
/// * `vertices` - The mesh vertices
/// * `faces` - The mesh faces (triangles)
/// * `tolerance` - Dot product threshold (1.0 = perfect match, 0.0 = perpendicular)
///
/// # Returns
/// List of face indices where normals don't match
#[allow(dead_code)]
pub fn check_normal_consistency(
    vertices: &[VertexWrapper],
    faces: &[VpxFace],
    tolerance: f32,
) -> Vec<usize> {
    let mut inconsistent = Vec::new();

    for (face_idx, face) in faces.iter().enumerate() {
        if face.i0 < 0
            || face.i0 >= vertices.len() as i64
            || face.i1 < 0
            || face.i1 >= vertices.len() as i64
            || face.i2 < 0
            || face.i2 >= vertices.len() as i64
        {
            continue;
        }

        let v0 = &vertices[face.i0 as usize].vertex;
        let v1 = &vertices[face.i1 as usize].vertex;
        let v2 = &vertices[face.i2 as usize].vertex;

        // Compute face normal from vertices
        let e1 = (v1.x - v0.x, v1.y - v0.y, v1.z - v0.z);
        let e2 = (v2.x - v0.x, v2.y - v0.y, v2.z - v0.z);

        let face_normal = (
            e1.1 * e2.2 - e1.2 * e2.1,
            e1.2 * e2.0 - e1.0 * e2.2,
            e1.0 * e2.1 - e1.1 * e2.0,
        );

        let face_normal_len = (face_normal.0 * face_normal.0
            + face_normal.1 * face_normal.1
            + face_normal.2 * face_normal.2)
            .sqrt();

        if face_normal_len < 0.0001 {
            continue; // Degenerate face
        }

        let face_normal = (
            face_normal.0 / face_normal_len,
            face_normal.1 / face_normal_len,
            face_normal.2 / face_normal_len,
        );

        // Check each vertex normal against the face normal
        for v in [v0, v1, v2] {
            let vertex_normal_len = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();
            if vertex_normal_len < 0.0001 {
                continue;
            }

            let dot = (face_normal.0 * v.nx + face_normal.1 * v.ny + face_normal.2 * v.nz)
                / vertex_normal_len;

            // If dot product is negative, normals point in opposite directions
            // (which could indicate wrong winding or inverted normals)
            if dot < tolerance {
                if !inconsistent.contains(&face_idx) {
                    inconsistent.push(face_idx);
                }
                break;
            }
        }
    }

    inconsistent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::model::Vertex3dNoTex2;

    fn make_vertex(x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32) -> VertexWrapper {
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x,
                y,
                z,
                nx,
                ny,
                nz,
                tu: 0.0,
                tv: 0.0,
            },
        )
    }

    #[test]
    fn test_valid_triangle() {
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        ];
        let faces = vec![VpxFace {
            i0: 0,
            i1: 1,
            i2: 2,
        }];

        let result = validate_mesh(&vertices, &faces);
        assert!(result.invalid_indices.is_empty());
        assert!(result.zero_normals.is_empty());
        assert!(result.degenerate_faces.is_empty());
    }

    #[test]
    fn test_invalid_index() {
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        ];
        let faces = vec![VpxFace {
            i0: 0,
            i1: 1,
            i2: 5, // Invalid
        }];

        let result = validate_mesh(&vertices, &faces);
        assert_eq!(result.invalid_indices.len(), 1);
    }

    #[test]
    fn test_zero_normal() {
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 0.0), // Zero normal
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        ];
        let faces = vec![VpxFace {
            i0: 0,
            i1: 1,
            i2: 2,
        }];

        let result = validate_mesh(&vertices, &faces);
        assert_eq!(result.zero_normals.len(), 1);
        assert_eq!(result.zero_normals[0], 0);
    }

    #[test]
    fn test_degenerate_triangle() {
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(2.0, 0.0, 0.0, 0.0, 0.0, 1.0), // Collinear
        ];
        let faces = vec![VpxFace {
            i0: 0,
            i1: 1,
            i2: 2,
        }];

        let result = validate_mesh(&vertices, &faces);
        assert_eq!(result.degenerate_faces.len(), 1);
    }

    #[test]
    fn test_watertight_cube() {
        // A simple cube with 8 vertices and 12 triangles (2 per face)
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, -1.0),
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, -1.0),
            make_vertex(1.0, 1.0, 0.0, 0.0, 0.0, -1.0),
            make_vertex(0.0, 1.0, 0.0, 0.0, 0.0, -1.0),
            make_vertex(0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 1.0, 1.0, 0.0, 0.0, 1.0),
            make_vertex(0.0, 1.0, 1.0, 0.0, 0.0, 1.0),
        ];

        // Cube faces (CCW winding when viewed from outside)
        let faces = vec![
            // Bottom (z=0)
            VpxFace {
                i0: 0,
                i1: 2,
                i2: 1,
            },
            VpxFace {
                i0: 0,
                i1: 3,
                i2: 2,
            },
            // Top (z=1)
            VpxFace {
                i0: 4,
                i1: 5,
                i2: 6,
            },
            VpxFace {
                i0: 4,
                i1: 6,
                i2: 7,
            },
            // Front (y=0)
            VpxFace {
                i0: 0,
                i1: 1,
                i2: 5,
            },
            VpxFace {
                i0: 0,
                i1: 5,
                i2: 4,
            },
            // Back (y=1)
            VpxFace {
                i0: 2,
                i1: 3,
                i2: 7,
            },
            VpxFace {
                i0: 2,
                i1: 7,
                i2: 6,
            },
            // Left (x=0)
            VpxFace {
                i0: 0,
                i1: 4,
                i2: 7,
            },
            VpxFace {
                i0: 0,
                i1: 7,
                i2: 3,
            },
            // Right (x=1)
            VpxFace {
                i0: 1,
                i1: 2,
                i2: 6,
            },
            VpxFace {
                i0: 1,
                i1: 6,
                i2: 5,
            },
        ];

        let result = validate_mesh(&vertices, &faces);
        assert!(
            result.is_watertight(),
            "Cube should be watertight: {:?}",
            result.boundary_edges
        );
    }

    #[test]
    fn test_mesh_with_hole() {
        // A plane with 4 vertices and 2 triangles - has boundary edges
        let vertices = vec![
            make_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(1.0, 1.0, 0.0, 0.0, 0.0, 1.0),
            make_vertex(0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        ];
        let faces = vec![
            VpxFace {
                i0: 0,
                i1: 1,
                i2: 2,
            },
            VpxFace {
                i0: 0,
                i1: 2,
                i2: 3,
            },
        ];

        let result = validate_mesh(&vertices, &faces);
        assert!(!result.is_watertight(), "Plane should have boundary edges");
        assert!(!result.boundary_edges.is_empty());
    }

    #[test]
    fn test_flipper_mesh_validation() {
        use crate::vpx::expanded::flippers::build_flipper_mesh;
        use crate::vpx::gameitem::flipper::Flipper;
        use fake::{Fake, Faker};

        // Create a flipper with randomized values but ensure it's visible
        let mut flipper: Flipper = Faker.fake();
        flipper.name = "TestFlipper".to_string();
        flipper.is_visible = true;
        // Set reasonable values for mesh generation
        flipper.base_radius = 21.5;
        flipper.end_radius = 13.0;
        flipper.flipper_radius_max = 130.0;
        flipper.height = 50.0;
        flipper.rubber_thickness = Some(7.0);
        flipper.rubber_height = Some(19.0);
        flipper.rubber_width = Some(24.0);
        flipper.start_angle = 121.0;

        let (vertices, faces) =
            build_flipper_mesh(&flipper, 0.0).expect("Flipper mesh should be generated");

        let result = validate_mesh(&vertices, &faces);

        // Check for critical issues
        assert!(
            result.invalid_indices.is_empty(),
            "Flipper mesh has invalid indices: {:?}",
            result.invalid_indices
        );
        assert!(
            result.degenerate_faces.is_empty(),
            "Flipper mesh has degenerate faces: {:?}",
            result.degenerate_faces
        );

        // Flipper meshes are typically not fully watertight (they're more like shells)
        // but they shouldn't have non-manifold edges
        assert!(
            result.non_manifold_edges.is_empty(),
            "Flipper mesh has non-manifold edges: {:?}",
            result.non_manifold_edges
        );

        // Check normal consistency - this catches inverted/wrong normals
        // tolerance of 0.0 means normals should point in same hemisphere as face normal
        let inconsistent_normals = check_normal_consistency(&vertices, &faces, 0.0);
        assert!(
            inconsistent_normals.len() < faces.len() / 4, // Allow up to 25% inconsistent (for smooth shading at edges)
            "Flipper mesh has too many faces with inconsistent normals: {} out of {} faces",
            inconsistent_normals.len(),
            faces.len()
        );

        // Report any other issues (not failures, just info)
        if !result.zero_normals.is_empty() {
            eprintln!(
                "Warning: Flipper mesh has {} zero normals",
                result.zero_normals.len()
            );
        }
        if !result.non_unit_normals.is_empty() {
            eprintln!(
                "Warning: Flipper mesh has {} non-unit normals",
                result.non_unit_normals.len()
            );
        }
    }
}
