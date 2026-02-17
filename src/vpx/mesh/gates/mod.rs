//! Gate mesh generation for expanded VPX export
//!
//! This module ports the gate mesh generation from VPinball's gate.cpp.
//! Gates have two parts:
//! - Bracket: The fixed mounting bracket (optional, controlled by show_bracket)
//! - Wire/Plate: The moving gate part (different shapes based on gate_type)
//!
//! Gate types:
//! - WireW: W-shaped wire gate (default)
//! - WireRectangle: Rectangular wire gate
//! - Plate: Solid plate gate
//! - LongPlate: Extended solid plate gate
//!
//! Ported from: VPinball/src/parts/gate.cpp

mod gate_bracket_mesh;
mod gate_long_plate_mesh;
mod gate_plate_mesh;
mod gate_wire_mesh;
mod gate_wire_rectangle_mesh;

use crate::vpx::gameitem::gate::{Gate, GateType};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::math::{Matrix3D, Vertex3D};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

use gate_bracket_mesh::{GATE_BRACKET_INDICES, GATE_BRACKET_MESH};
use gate_long_plate_mesh::{GATE_LONG_PLATE_INDICES, GATE_LONG_PLATE_MESH};
use gate_plate_mesh::{GATE_PLATE_INDICES, GATE_PLATE_MESH};
use gate_wire_mesh::{GATE_WIRE_INDICES, GATE_WIRE_MESH};
use gate_wire_rectangle_mesh::{GATE_WIRE_RECTANGLE_INDICES, GATE_WIRE_RECTANGLE_MESH};

/// Result of gate mesh generation with separate meshes for bracket and wire/plate
///
/// Vertices are centered at origin.
pub struct GateMeshes {
    /// The bracket mesh (if show_bracket is true)
    pub bracket: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// The wire/plate mesh (based on gate_type)
    pub wire: (Vec<VertexWrapper>, Vec<VpxFace>),
}

/// Get the mesh data for a gate type
fn get_mesh_for_type(gate_type: &GateType) -> (&'static [Vertex3dNoTex2], &'static [u16]) {
    match gate_type {
        GateType::WireW => (&GATE_WIRE_MESH, &GATE_WIRE_INDICES),
        GateType::WireRectangle => (&GATE_WIRE_RECTANGLE_MESH, &GATE_WIRE_RECTANGLE_INDICES),
        GateType::Plate => (&GATE_PLATE_MESH, &GATE_PLATE_INDICES),
        GateType::LongPlate => (&GATE_LONG_PLATE_MESH, &GATE_LONG_PLATE_INDICES),
    }
}

/// Generate gate bracket mesh
///
/// From VPinball Gate::GenerateBracketMesh:
/// ```cpp
/// const Matrix3D rotMatrix = Matrix3D::MatrixRotateZ(ANGTORAD(m_d.m_rotation));
/// const Matrix3D vertMatrix = rotMatrix * Matrix3D::MatrixScale(m_d.m_length)
///     * Matrix3D::MatrixTranslate(m_d.m_vCenter.x, m_d.m_vCenter.y, m_d.m_height + m_baseHeight);
/// vertMatrix.TransformPositions(gateBracket, buf, gateBracketNumVertices);
/// rotMatrix.TransformNormals(gateBracket, buf, gateBracketNumVertices);
/// ```
///
/// Note: Translation is NOT applied here - use gate.center and height for node transform.
fn generate_bracket_mesh(gate: &Gate) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let rot_matrix = Matrix3D::rotate_z(gate.rotation.to_radians());
    let vert_matrix = rot_matrix * Matrix3D::scale_uniform(gate.length);

    let vertices: Vec<VertexWrapper> = GATE_BRACKET_MESH
        .iter()
        .map(|v| {
            // Transform position (rotation and scale only, no translation)
            let pos = Vertex3D::new(v.x, v.y, v.z);
            let transformed_pos = vert_matrix.transform_vertex(pos);

            // Transform normal (rotation only)
            let normal = rot_matrix.transform_normal(v.nx, v.ny, v.nz);
            let normal = normal.normalized();

            VertexWrapper {
                vpx_encoded_vertex: [0u8; 32],
                vertex: Vertex3dNoTex2 {
                    x: transformed_pos.x,
                    y: transformed_pos.y,
                    z: transformed_pos.z,
                    nx: normal.x,
                    ny: normal.y,
                    nz: normal.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            }
        })
        .collect();

    let faces: Vec<VpxFace> = GATE_BRACKET_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, faces)
}

/// Generate gate wire/plate mesh
///
/// From VPinball Gate::RenderDynamic (line 417-418):
/// ```cpp
/// const Matrix3D vertMatrix = (fullMatrix
///     * Matrix3D::MatrixScale(m_d.m_length, m_d.m_length, m_d.m_length))
///     * Matrix3D::MatrixTranslate(m_d.m_vCenter.x, m_d.m_vCenter.y, m_d.m_height + m_baseHeight);
/// ```
///
/// Note: The wire mesh IS scaled by length, same as the bracket.
/// Translation is NOT applied here - use gate.center and height for node transform.
fn generate_wire_mesh(
    gate: &Gate,
    mesh: &[Vertex3dNoTex2],
    indices: &[u16],
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    // Rotation and scale (no translation - that goes in node transform)
    let world_matrix =
        Matrix3D::rotate_z(gate.rotation.to_radians()) * Matrix3D::scale_uniform(gate.length);

    let vertices: Vec<VertexWrapper> = mesh
        .iter()
        .map(|v| {
            // Transform position (rotation and scale)
            let pos = Vertex3D::new(v.x, v.y, v.z);
            let transformed_pos = world_matrix.transform_vertex(pos);

            // Transform normal (rotation only, not scale)
            let rot_matrix = Matrix3D::rotate_z(gate.rotation.to_radians());
            let normal = rot_matrix.transform_normal(v.nx, v.ny, v.nz);
            let normal = normal.normalized();

            VertexWrapper {
                vpx_encoded_vertex: [0u8; 32],
                vertex: Vertex3dNoTex2 {
                    x: transformed_pos.x,
                    y: transformed_pos.y,
                    z: transformed_pos.z,
                    nx: normal.x,
                    ny: normal.y,
                    nz: normal.z,
                    tu: v.tu,
                    tv: v.tv,
                },
            }
        })
        .collect();

    let faces: Vec<VpxFace> = indices
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, faces)
}

/// Generate all gate meshes based on the gate parameters
///
/// Vertices are centered at origin. Use `gate.center` and
/// `base_height + gate.height` for the glTF node transform.
///
/// # Arguments
/// * `gate` - The gate definition
///
/// # Returns
/// A GateMeshes struct containing bracket (optional) and wire/plate meshes
pub fn build_gate_meshes(gate: &Gate) -> Option<GateMeshes> {
    if !gate.is_visible {
        return None;
    }

    // Get the appropriate mesh for this gate type (default to WireW if not specified)
    let gate_type = gate.gate_type.as_ref().unwrap_or(&GateType::WireW);
    let (mesh, indices) = get_mesh_for_type(gate_type);

    Some(GateMeshes {
        bracket: if gate.show_bracket {
            Some(generate_bracket_mesh(gate))
        } else {
            None
        },
        wire: generate_wire_mesh(gate, mesh, indices),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    fn make_test_gate(gate_type: GateType, show_bracket: bool, is_visible: bool) -> Gate {
        let mut gate = Gate::default();
        gate.center = Vertex2D { x: 100.0, y: 200.0 };
        gate.length = 50.0;
        gate.height = 30.0;
        gate.rotation = 45.0;
        gate.gate_type = Some(gate_type);
        gate.show_bracket = show_bracket;
        gate.is_visible = is_visible;
        gate
    }

    #[test]
    fn test_build_gate_meshes_wire_w() {
        let gate = make_test_gate(GateType::WireW, true, true);
        let result = build_gate_meshes(&gate);
        assert!(result.is_some());

        let meshes = result.unwrap();
        assert!(meshes.bracket.is_some());

        let (bracket_verts, bracket_faces) = meshes.bracket.unwrap();
        assert_eq!(bracket_verts.len(), 184);
        assert_eq!(bracket_faces.len(), 516 / 3);

        let (wire_verts, wire_faces) = meshes.wire;
        assert_eq!(wire_verts.len(), 186);
        assert_eq!(wire_faces.len(), 1008 / 3);
    }

    #[test]
    fn test_build_gate_meshes_no_bracket() {
        let gate = make_test_gate(GateType::WireW, false, true);
        let result = build_gate_meshes(&gate);
        assert!(result.is_some());

        let meshes = result.unwrap();
        assert!(meshes.bracket.is_none());
    }

    #[test]
    fn test_build_gate_meshes_invisible() {
        let gate = make_test_gate(GateType::WireW, true, false);
        let result = build_gate_meshes(&gate);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_gate_meshes_all_types() {
        let types = [
            GateType::WireW,
            GateType::WireRectangle,
            GateType::Plate,
            GateType::LongPlate,
        ];

        for gate_type in types {
            let gate = make_test_gate(gate_type.clone(), true, true);
            let result = build_gate_meshes(&gate);
            assert!(
                result.is_some(),
                "Failed to generate mesh for {:?}",
                gate_type
            );
        }
    }
}
