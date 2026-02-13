//! Trigger mesh generation for expanded VPX export
//!
//! This module ports the trigger mesh generation from VPinball's trigger.cpp.
//! Triggers can have different shapes:
//! - None: No visible mesh
//! - WireA, WireB, WireC: Simple wire triggers (use triggerSimple mesh with different rotations)
//! - WireD: D-shaped wire trigger
//! - Star: Star-shaped trigger
//! - Button: Button trigger
//! - Inder: Inder-style trigger
//!
//! Shape-specific transformations:
//! - WireB: Rotated -23° around X axis before Z rotation
//! - WireC: Rotated 140° around X axis before Z rotation, Z offset -19
//! - Button: Z offset +5
//! - Button/Star: Scaled by radius instead of scaleX/scaleY
//! - WireA/B/C/D/Inder: Wire thickness applied to vertex positions
//!
//! Ported from: VPinball/src/parts/trigger.cpp

mod trigger_button_mesh;
mod trigger_inder_mesh;
mod trigger_simple_mesh;
mod trigger_star_mesh;
mod trigger_wire_d_mesh;

use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gameitem::trigger::{Trigger, TriggerShape};
use crate::vpx::math::{Matrix3D, Vertex3D};
use crate::vpx::mesh::{generated_mesh_file_name, write_mesh_to_file};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::path::Path;

use trigger_button_mesh::{TRIGGER_BUTTON_INDICES, TRIGGER_BUTTON_MESH};
use trigger_inder_mesh::{TRIGGER_INDER_INDICES, TRIGGER_INDER_MESH};
use trigger_simple_mesh::{TRIGGER_SIMPLE_INDICES, TRIGGER_SIMPLE_MESH};
use trigger_star_mesh::{TRIGGER_STAR_INDICES, TRIGGER_STAR_MESH};
use trigger_wire_d_mesh::{TRIGGER_WIRE_D_INDICES, TRIGGER_WIRE_D_MESH};

/// Get the mesh data for a trigger shape
fn get_mesh_for_shape(shape: &TriggerShape) -> Option<(&'static [Vertex3dNoTex2], &'static [u16])> {
    match shape {
        TriggerShape::None => None,
        TriggerShape::WireA | TriggerShape::WireB | TriggerShape::WireC => {
            Some((&TRIGGER_SIMPLE_MESH, &TRIGGER_SIMPLE_INDICES))
        }
        TriggerShape::WireD => Some((&TRIGGER_WIRE_D_MESH, &TRIGGER_WIRE_D_INDICES)),
        TriggerShape::Star => Some((&TRIGGER_STAR_MESH, &TRIGGER_STAR_INDICES)),
        TriggerShape::Button => Some((&TRIGGER_BUTTON_MESH, &TRIGGER_BUTTON_INDICES)),
        TriggerShape::Inder => Some((&TRIGGER_INDER_MESH, &TRIGGER_INDER_INDICES)),
    }
}

/// Get the Z offset for a trigger shape
/// From VPinball trigger.cpp GenerateMesh():
/// ```cpp
/// float zoffset = (m_d.m_shape == TriggerButton) ? 5.0f : 0.0f;
/// if (m_d.m_shape == TriggerWireC) zoffset = -19.0f;
/// ```
fn get_z_offset(shape: &TriggerShape) -> f32 {
    match shape {
        TriggerShape::Button => 5.0,
        TriggerShape::WireC => -19.0,
        _ => 0.0,
    }
}

/// Get the rotation matrix for a trigger shape
/// From VPinball trigger.cpp GenerateMesh():
/// ```cpp
/// if (m_d.m_shape == TriggerWireB)
///     fullMatrix = Matrix3D::MatrixRotateX(ANGTORAD(-23.f)) * Matrix3D::MatrixRotateZ(ANGTORAD(m_d.m_rotation));
/// else if (m_d.m_shape == TriggerWireC)
///     fullMatrix = Matrix3D::MatrixRotateX(ANGTORAD(140.f)) * Matrix3D::MatrixRotateZ(ANGTORAD(m_d.m_rotation));
/// else
///     fullMatrix = Matrix3D::MatrixRotateZ(ANGTORAD(m_d.m_rotation));
/// ```
fn get_rotation_matrix(shape: &TriggerShape, rotation: f32) -> Matrix3D {
    match shape {
        TriggerShape::WireB => {
            Matrix3D::rotate_x((-23.0_f32).to_radians()) * Matrix3D::rotate_z(rotation.to_radians())
        }
        TriggerShape::WireC => {
            Matrix3D::rotate_x(140.0_f32.to_radians()) * Matrix3D::rotate_z(rotation.to_radians())
        }
        _ => Matrix3D::rotate_z(rotation.to_radians()),
    }
}

/// Check if a trigger shape uses radius scaling (vs scaleX/scaleY)
/// From VPinball trigger.cpp GenerateMesh():
/// ```cpp
/// if (m_d.m_shape == TriggerButton || m_d.m_shape == TriggerStar)
///     // scale by radius
/// else
///     // scale by scaleX/scaleY
/// ```
fn uses_radius_scaling(shape: &TriggerShape) -> bool {
    matches!(shape, TriggerShape::Button | TriggerShape::Star)
}

/// Check if a trigger shape uses wire thickness
/// From VPinball trigger.cpp GenerateMesh():
/// ```cpp
/// if (m_d.m_shape == TriggerWireA || m_d.m_shape == TriggerWireB ||
///     m_d.m_shape == TriggerWireC || m_d.m_shape == TriggerWireD || m_d.m_shape == TriggerInder)
/// ```
fn uses_wire_thickness(shape: &TriggerShape) -> bool {
    matches!(
        shape,
        TriggerShape::WireA
            | TriggerShape::WireB
            | TriggerShape::WireC
            | TriggerShape::WireD
            | TriggerShape::Inder
    )
}

/// Generate trigger mesh
///
/// Ported from VPinball trigger.cpp GenerateMesh()
///
/// # Arguments
/// * `trigger` - The trigger definition
/// * `base_height` - The height of the surface the trigger sits on (from table surface lookup)
///
/// # Returns
/// Tuple of (vertices, indices) if the trigger has a visible mesh, None otherwise
pub fn build_trigger_mesh(
    trigger: &Trigger,
    base_height: f32,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if !trigger.is_visible {
        return None;
    }

    let (mesh, indices) = get_mesh_for_shape(&trigger.shape)?;

    let z_offset = get_z_offset(&trigger.shape);
    let full_matrix = get_rotation_matrix(&trigger.shape, trigger.rotation);
    let uses_radius = uses_radius_scaling(&trigger.shape);
    let apply_wire_thickness = uses_wire_thickness(&trigger.shape);
    let wire_thickness = trigger.wire_thickness.unwrap_or(0.0);

    let vertices: Vec<VertexWrapper> = mesh
        .iter()
        .map(|v| {
            // Transform position by rotation matrix
            let pos = Vertex3D::new(v.x, v.y, v.z);
            let rotated = full_matrix.transform_vertex(pos);

            // Scale and translate
            let (x, y, z) = if uses_radius {
                (
                    rotated.x * trigger.radius + trigger.center.x,
                    rotated.y * trigger.radius + trigger.center.y,
                    rotated.z * trigger.radius + base_height + z_offset,
                )
            } else {
                (
                    rotated.x * trigger.scale_x + trigger.center.x,
                    rotated.y * trigger.scale_y + trigger.center.y,
                    rotated.z * 1.0 + base_height + z_offset,
                )
            };

            // Transform normal
            let normal = full_matrix.transform_normal(v.nx, v.ny, v.nz);
            let normal = normal.normalized();

            // Apply wire thickness if applicable
            let (final_x, final_y, final_z) = if apply_wire_thickness {
                (
                    x + normal.x * wire_thickness,
                    y + normal.y * wire_thickness,
                    z + normal.z * wire_thickness,
                )
            } else {
                (x, y, z)
            };

            VertexWrapper {
                vpx_encoded_vertex: [0u8; 32],
                vertex: Vertex3dNoTex2 {
                    x: final_x,
                    y: final_y,
                    z: final_z,
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

    Some((vertices, faces))
}

/// Write trigger mesh to file
pub(crate) fn write_trigger_mesh(
    gameitems_dir: &Path,
    trigger: &Trigger,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_trigger_mesh(trigger, 0.0) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &trigger.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    fn make_test_trigger(shape: TriggerShape, is_visible: bool) -> Trigger {
        let mut trigger = Trigger::default();
        trigger.center = Vertex2D { x: 100.0, y: 200.0 };
        trigger.radius = 25.0;
        trigger.rotation = 45.0;
        trigger.scale_x = 1.0;
        trigger.scale_y = 1.0;
        trigger.wire_thickness = Some(2.0);
        trigger.shape = shape;
        trigger.is_visible = is_visible;
        trigger
    }

    #[test]
    fn test_build_trigger_mesh_wire_a() {
        let trigger = make_test_trigger(TriggerShape::WireA, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 49);
        assert_eq!(faces.len(), 216 / 3);
    }

    #[test]
    fn test_build_trigger_mesh_star() {
        let trigger = make_test_trigger(TriggerShape::Star, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 231);
        assert_eq!(faces.len(), 510 / 3);
    }

    #[test]
    fn test_build_trigger_mesh_button() {
        let trigger = make_test_trigger(TriggerShape::Button, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 528);
        assert_eq!(faces.len(), 948 / 3);
    }

    #[test]
    fn test_build_trigger_mesh_wire_d() {
        let trigger = make_test_trigger(TriggerShape::WireD, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 203);
        assert_eq!(faces.len(), 798 / 3);
    }

    #[test]
    fn test_build_trigger_mesh_inder() {
        let trigger = make_test_trigger(TriggerShape::Inder, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 152);
        assert_eq!(faces.len(), 312 / 3);
    }

    #[test]
    fn test_build_trigger_mesh_none_shape() {
        let trigger = make_test_trigger(TriggerShape::None, true);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_trigger_mesh_invisible() {
        let trigger = make_test_trigger(TriggerShape::WireA, false);
        let result = build_trigger_mesh(&trigger, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_all_wire_shapes_use_simple_mesh() {
        for shape in [
            TriggerShape::WireA,
            TriggerShape::WireB,
            TriggerShape::WireC,
        ] {
            let trigger = make_test_trigger(shape.clone(), true);
            let result = build_trigger_mesh(&trigger, 0.0);
            assert!(result.is_some(), "Failed for shape {:?}", shape);

            let (vertices, _) = result.unwrap();
            assert_eq!(
                vertices.len(),
                49,
                "Wrong vertex count for shape {:?}",
                shape
            );
        }
    }
}
