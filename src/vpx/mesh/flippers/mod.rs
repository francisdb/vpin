//! Flipper mesh generation for expanded VPX export
//!
//! This module ports the flipper mesh generation from Visual Pinball's flipper.cpp.
//! Flippers use a pre-defined base mesh that is scaled and transformed based on
//! the flipper's parameters (base radius, end radius, length, height, etc.).
//!
//! Ported from: VisualPinball.Engine/VPT/Flipper/FlipperMeshGenerator.cs
//! Original C++: VPinball/src/parts/flipper.cpp

mod flipper_base_mesh;

use crate::vpx::gameitem::flipper::Flipper;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use std::f32::consts::PI;

use crate::vpx::math::{Mat3, Vec2, Vec3};
pub use flipper_base_mesh::*;

/// Result of flipper mesh generation with separate base and rubber meshes
///
/// Vertices are centered at the flipper's rotation pivot point (center).
/// The center position is returned for use as a glTF node transform.
pub struct FlipperMeshes {
    /// The base flipper mesh (uses flipper.material)
    pub base: (Vec<VertexWrapper>, Vec<VpxFace>),
    /// The rubber mesh on top of the flipper (uses flipper.rubber_material)
    /// Only present if rubber_thickness > 0
    pub rubber: Option<(Vec<VertexWrapper>, Vec<VpxFace>)>,
    /// Center position (rotation pivot) in VPX coordinates.
    pub center: Vec3,
}

/// Reference vertices for the flipper TIP bottom (the end that hits the ball)
/// Note: These are at y ≈ 0.786-0.887 (the tip end of the flipper)
/// From VPinball src/parts/flipper.cpp vertsTipBottomf
#[rustfmt::skip]
static VERTS_TIP_BOTTOM: [Vec3; 13] = [
    Vec3 { x: -0.101425, y: 0.786319, z: 0.003753 },
    Vec3 { x: -0.097969, y: 0.812569, z: 0.003753 },
    Vec3 { x: -0.087837, y: 0.837031, z: 0.003753 },
    Vec3 { x: -0.071718, y: 0.858037, z: 0.003753 },
    Vec3 { x: -0.050713, y: 0.874155, z: 0.003753 },
    Vec3 { x: -0.026251, y: 0.884288, z: 0.003753 },
    Vec3 { x: 0.0, y: 0.887744, z: 0.003753 },
    Vec3 { x: 0.026251, y: 0.884288, z: 0.003753 },
    Vec3 { x: 0.050713, y: 0.874155, z: 0.003753 },
    Vec3 { x: 0.071718, y: 0.858037, z: 0.003753 },
    Vec3 { x: 0.087837, y: 0.837031, z: 0.003753 },
    Vec3 { x: 0.097969, y: 0.812569, z: 0.003753 },
    Vec3 { x: 0.101425, y: 0.786319, z: 0.003753 },
];

/// Reference vertices for the flipper TIP top
/// From VPinball src/parts/flipper.cpp vertsTipTopf
#[rustfmt::skip]
static VERTS_TIP_TOP: [Vec3; 13] = [
    Vec3 { x: -0.101425, y: 0.786319, z: 1.004253 },
    Vec3 { x: -0.097969, y: 0.812569, z: 1.004253 },
    Vec3 { x: -0.087837, y: 0.837031, z: 1.004253 },
    Vec3 { x: -0.071718, y: 0.858037, z: 1.004253 },
    Vec3 { x: -0.050713, y: 0.874155, z: 1.004253 },
    Vec3 { x: -0.026251, y: 0.884288, z: 1.004253 },
    Vec3 { x: 0.0, y: 0.887744, z: 1.004253 },
    Vec3 { x: 0.026251, y: 0.884288, z: 1.004253 },
    Vec3 { x: 0.050713, y: 0.874155, z: 1.004253 },
    Vec3 { x: 0.071718, y: 0.858037, z: 1.004253 },
    Vec3 { x: 0.087837, y: 0.837031, z: 1.004253 },
    Vec3 { x: 0.097969, y: 0.812569, z: 1.004253 },
    Vec3 { x: 0.101425, y: 0.786319, z: 1.004253 },
];

/// Reference vertices for the flipper BASE bottom (the pivot end)
/// Note: These are at y ≈ -0.1 to 0 (the base/pivot of the flipper)
/// From VPinball src/parts/flipper.cpp vertsBaseBottomf
#[rustfmt::skip]
static VERTS_BASE_BOTTOM: [Vec3; 13] = [
    Vec3 { x: -0.100762, y: 0.0, z: 0.003753 },
    Vec3 { x: -0.097329, y: -0.026079, z: 0.003753 },
    Vec3 { x: -0.087263, y: -0.050381, z: 0.003753 },
    Vec3 { x: -0.071250, y: -0.071250, z: 0.003753 },
    Vec3 { x: -0.050381, y: -0.087263, z: 0.003753 },
    Vec3 { x: -0.026079, y: -0.097329, z: 0.003753 },
    Vec3 { x: 0.0, y: -0.100762, z: 0.003753 },
    Vec3 { x: 0.026079, y: -0.097329, z: 0.003753 },
    Vec3 { x: 0.050381, y: -0.087263, z: 0.003753 },
    Vec3 { x: 0.071250, y: -0.071250, z: 0.003753 },
    Vec3 { x: 0.087263, y: -0.050381, z: 0.003753 },
    Vec3 { x: 0.097329, y: -0.026079, z: 0.003753 },
    Vec3 { x: 0.100762, y: 0.0, z: 0.003753 },
];

/// Reference vertices for the flipper BASE top
/// From VPinball src/parts/flipper.cpp vertsBaseTopf
#[rustfmt::skip]
static VERTS_BASE_TOP: [Vec3; 13] = [
    Vec3 { x: -0.100762, y: 0.0, z: 1.004253 },
    Vec3 { x: -0.097329, y: -0.026079, z: 1.004253 },
    Vec3 { x: -0.087263, y: -0.050381, z: 1.004253 },
    Vec3 { x: -0.071250, y: -0.071250, z: 1.004253 },
    Vec3 { x: -0.050381, y: -0.087263, z: 1.004253 },
    Vec3 { x: -0.026079, y: -0.097329, z: 1.004253 },
    Vec3 { x: 0.0, y: -0.100762, z: 1.004253 },
    Vec3 { x: 0.026079, y: -0.097329, z: 1.004253 },
    Vec3 { x: 0.050381, y: -0.087263, z: 1.004253 },
    Vec3 { x: 0.071250, y: -0.071250, z: 1.004253 },
    Vec3 { x: 0.087263, y: -0.050381, z: 1.004253 },
    Vec3 { x: 0.097329, y: -0.026079, z: 1.004253 },
    Vec3 { x: 0.100762, y: 0.0, z: 1.004253 },
];

/// Sign function matching VPinball's sgn()
fn sgn(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Apply fix for flipper vertex scaling
/// Ported from VPinball flipper.cpp ApplyFix()
///
/// This function adjusts vertex positions and normals to scale the flipper
/// base and tip to the desired radii.
fn apply_fix(
    vert: &mut Vertex3dNoTex2,
    center: Vec2,
    mid_angle: f32,
    radius: f32,
    new_center: Vec2,
    fix_angle_scale: f32,
) {
    let mut v_angle = (vert.y - center.y).atan2(vert.x - center.x);
    let mut n_angle = vert.ny.atan2(vert.nx);

    // We want to have angles with same sign as mid_angle, fix it:
    if mid_angle < 0.0 {
        if v_angle > 0.0 {
            v_angle -= PI * 2.0;
        }
        if n_angle > 0.0 {
            n_angle -= PI * 2.0;
        }
    } else {
        if v_angle < 0.0 {
            v_angle += PI * 2.0;
        }
        if n_angle < 0.0 {
            n_angle += PI * 2.0;
        }
    }

    let sgn_mid = sgn(mid_angle);
    n_angle -= (v_angle - mid_angle) * fix_angle_scale * sgn_mid;
    v_angle -= (v_angle - mid_angle) * fix_angle_scale * sgn_mid;

    let n_length = (vert.nx * vert.nx + vert.ny * vert.ny).sqrt();

    vert.x = v_angle.cos() * radius + new_center.x;
    vert.y = v_angle.sin() * radius + new_center.y;
    vert.nx = n_angle.cos() * n_length;
    vert.ny = n_angle.sin() * n_length;
}

/// Check if vertex matches a reference vertex (with floating point tolerance)
fn vertex_matches(v: &Vertex3dNoTex2, r: &Vec3) -> bool {
    const EPSILON: f32 = 0.0001;
    (v.x - r.x).abs() < EPSILON && (v.y - r.y).abs() < EPSILON && (v.z - r.z).abs() < EPSILON
}

/// Build flipper mesh geometry
///
/// # Arguments
/// * `flipper` - The flipper data
/// * `surface_height` - The height of the surface the flipper is on (typically 0.0)
///
/// # Returns
/// Tuple of (vertices, indices) for the flipper mesh, or None if flipper is not visible
pub fn build_flipper_mesh(
    flipper: &Flipper,
    surface_height: f32,
) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    if !flipper.is_visible {
        return None;
    }

    let rubber_thickness = flipper
        .rubber_thickness
        .unwrap_or(flipper.rubber_thickness_int as f32);
    let rubber_height = flipper
        .rubber_height
        .unwrap_or(flipper.rubber_height_int as f32);
    let rubber_width = flipper
        .rubber_width
        .unwrap_or(flipper.rubber_width_int as f32);

    // Calculate angle needed to fix P0 location
    let sin_angle =
        ((flipper.base_radius - flipper.end_radius) / flipper.flipper_radius_max).clamp(-1.0, 1.0);
    let fix_angle = sin_angle.asin();
    let fix_angle_scale = fix_angle / (PI * 0.5);

    let base_radius = flipper.base_radius - rubber_thickness;
    let end_radius = flipper.end_radius - rubber_thickness;

    // Generate base flipper mesh
    let mut temp: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

    // Scale the base and tip vertices
    for t in 0..13 {
        for vert in temp.iter_mut() {
            if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
        }
    }

    // Apply rotation (180 degrees) and transformations
    let rotation_matrix = Mat3::rotate_z(180.0_f32.to_radians());
    let start_angle_rad = flipper.start_angle.to_radians();

    let mut vertices = Vec::with_capacity(FLIPPER_BASE_NUM_VERTICES * 2);

    for (i, temp_vert) in temp.iter().enumerate() {
        let rotated = rotation_matrix.multiply_vector(Vec3 {
            x: temp_vert.x,
            y: temp_vert.y,
            z: temp_vert.z,
        });

        let mut vert = Vertex3dNoTex2 {
            x: rotated.x,
            y: rotated.y,
            z: rotated.z * flipper.height + surface_height,
            nx: FLIPPER_BASE_MESH[i].nx,
            ny: FLIPPER_BASE_MESH[i].ny,
            nz: FLIPPER_BASE_MESH[i].nz,
            tu: FLIPPER_BASE_MESH[i].tu,
            tv: FLIPPER_BASE_MESH[i].tv,
        };

        // Apply normal rotation
        let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
            x: vert.nx,
            y: vert.ny,
            z: vert.nz,
        });
        vert.nx = rotated_normal.x;
        vert.ny = rotated_normal.y;
        vert.nz = rotated_normal.z;

        // Apply start angle rotation and translate to flipper center
        let (sin_a, cos_a) = start_angle_rad.sin_cos();
        let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
        let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

        // Rotate normal as well
        let final_nx = vert.nx * cos_a - vert.ny * sin_a;
        let final_ny = vert.nx * sin_a + vert.ny * cos_a;

        vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: final_x,
                y: final_y,
                z: vert.z,
                nx: final_nx,
                ny: final_ny,
                nz: vert.nz,
                tu: vert.tu,
                tv: vert.tv,
            },
        ));
    }

    // Generate rubber mesh if rubber_thickness > 0
    if rubber_thickness > 0.0 {
        let mut temp_rubber: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

        // Scale for rubber (with thickness added back)
        for t in 0..13 {
            for vert in temp_rubber.iter_mut() {
                if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
            }
        }

        for (i, temp_vert) in temp_rubber.iter().enumerate() {
            let rotated = rotation_matrix.multiply_vector(Vec3 {
                x: temp_vert.x,
                y: temp_vert.y,
                z: temp_vert.z,
            });

            let mut vert = Vertex3dNoTex2 {
                x: rotated.x,
                y: rotated.y,
                z: rotated.z * rubber_width + (surface_height + rubber_height),
                nx: FLIPPER_BASE_MESH[i].nx,
                ny: FLIPPER_BASE_MESH[i].ny,
                nz: FLIPPER_BASE_MESH[i].nz,
                tu: FLIPPER_BASE_MESH[i].tu,
                tv: FLIPPER_BASE_MESH[i].tv + 0.5,
            };

            // Apply normal rotation
            let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
                x: vert.nx,
                y: vert.ny,
                z: vert.nz,
            });
            vert.nx = rotated_normal.x;
            vert.ny = rotated_normal.y;
            vert.nz = rotated_normal.z;

            // Apply start angle rotation and translate to flipper center
            let (sin_a, cos_a) = start_angle_rad.sin_cos();
            let final_x = vert.x * cos_a - vert.y * sin_a + flipper.center.x;
            let final_y = vert.x * sin_a + vert.y * cos_a + flipper.center.y;

            // Rotate normal as well
            let final_nx = vert.nx * cos_a - vert.ny * sin_a;
            let final_ny = vert.nx * sin_a + vert.ny * cos_a;

            vertices.push(VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: final_x,
                    y: final_y,
                    z: vert.z,
                    nx: final_nx,
                    ny: final_ny,
                    nz: vert.nz,
                    tu: vert.tu,
                    tv: vert.tv,
                },
            ));
        }
    }

    // Build indices
    let mut indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES * 2 / 3);

    // Base mesh indices
    for chunk in FLIPPER_BASE_INDICES.chunks(3) {
        indices.push(VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        });
    }

    // Rubber mesh indices (offset by base vertex count)
    if rubber_thickness > 0.0 {
        for chunk in FLIPPER_BASE_INDICES.chunks(3) {
            indices.push(VpxFace {
                i0: (chunk[0] as i64) + FLIPPER_BASE_NUM_VERTICES as i64,
                i1: (chunk[1] as i64) + FLIPPER_BASE_NUM_VERTICES as i64,
                i2: (chunk[2] as i64) + FLIPPER_BASE_NUM_VERTICES as i64,
            });
        }
    }

    Some((vertices, indices))
}

/// Build flipper mesh geometry with separate base and rubber meshes
///
/// This is the preferred function for GLB export as it allows assigning
/// different materials to the base flipper and rubber.
///
/// # Arguments
/// * `flipper` - The flipper data
/// * `surface_height` - The height of the surface the flipper is on (typically 0.0)
///
/// # Returns
/// FlipperMeshes with separate base and rubber meshes, or None if flipper is not visible
pub fn build_flipper_meshes(flipper: &Flipper, surface_height: f32) -> Option<FlipperMeshes> {
    if !flipper.is_visible {
        return None;
    }

    let rubber_thickness = flipper
        .rubber_thickness
        .unwrap_or(flipper.rubber_thickness_int as f32);
    let rubber_height = flipper
        .rubber_height
        .unwrap_or(flipper.rubber_height_int as f32);
    let rubber_width = flipper
        .rubber_width
        .unwrap_or(flipper.rubber_width_int as f32);

    // Calculate angle needed to fix P0 location
    let sin_angle =
        ((flipper.base_radius - flipper.end_radius) / flipper.flipper_radius_max).clamp(-1.0, 1.0);
    let fix_angle = sin_angle.asin();
    let fix_angle_scale = fix_angle / (PI * 0.5);

    let base_radius = flipper.base_radius - rubber_thickness;
    let end_radius = flipper.end_radius - rubber_thickness;

    // Generate base flipper mesh
    let mut temp: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

    // Scale the base and tip vertices
    for t in 0..13 {
        for vert in temp.iter_mut() {
            if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_BASE_BOTTOM[6].x,
                        y: VERTS_BASE_BOTTOM[0].y,
                    },
                    -PI * 0.5,
                    base_radius,
                    Vec2 { x: 0.0, y: 0.0 },
                    fix_angle_scale,
                );
            }
            if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                apply_fix(
                    vert,
                    Vec2 {
                        x: VERTS_TIP_BOTTOM[6].x,
                        y: VERTS_TIP_BOTTOM[0].y,
                    },
                    PI * 0.5,
                    end_radius,
                    Vec2 {
                        x: 0.0,
                        y: flipper.flipper_radius_max,
                    },
                    fix_angle_scale,
                );
            }
        }
    }

    // Apply rotation (180 degrees) and transformations
    let rotation_matrix = Mat3::rotate_z(180.0_f32.to_radians());
    let start_angle_rad = flipper.start_angle.to_radians();

    // Build base mesh vertices
    let mut base_vertices = Vec::with_capacity(FLIPPER_BASE_NUM_VERTICES);
    for (i, temp_vert) in temp.iter().enumerate() {
        let rotated = rotation_matrix.multiply_vector(Vec3 {
            x: temp_vert.x,
            y: temp_vert.y,
            z: temp_vert.z,
        });

        let mut vert = Vertex3dNoTex2 {
            x: rotated.x,
            y: rotated.y,
            z: rotated.z * flipper.height + surface_height,
            nx: FLIPPER_BASE_MESH[i].nx,
            ny: FLIPPER_BASE_MESH[i].ny,
            nz: FLIPPER_BASE_MESH[i].nz,
            tu: FLIPPER_BASE_MESH[i].tu,
            tv: FLIPPER_BASE_MESH[i].tv,
        };

        // Apply normal rotation
        let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
            x: vert.nx,
            y: vert.ny,
            z: vert.nz,
        });
        vert.nx = rotated_normal.x;
        vert.ny = rotated_normal.y;
        vert.nz = rotated_normal.z;

        // Apply start angle rotation (do NOT translate to flipper center - that goes in node transform)
        let (sin_a, cos_a) = start_angle_rad.sin_cos();
        let final_x = vert.x * cos_a - vert.y * sin_a;
        let final_y = vert.x * sin_a + vert.y * cos_a;

        // Rotate normal as well
        let final_nx = vert.nx * cos_a - vert.ny * sin_a;
        let final_ny = vert.nx * sin_a + vert.ny * cos_a;

        base_vertices.push(VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: final_x,
                y: final_y,
                z: vert.z,
                nx: final_nx,
                ny: final_ny,
                nz: vert.nz,
                tu: vert.tu,
                tv: vert.tv,
            },
        ));
    }

    // Build base mesh indices
    let mut base_indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES / 3);
    for chunk in FLIPPER_BASE_INDICES.chunks(3) {
        base_indices.push(VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        });
    }

    // Generate rubber mesh if rubber_thickness > 0
    let rubber = if rubber_thickness > 0.0 {
        let mut temp_rubber: Vec<Vertex3dNoTex2> = FLIPPER_BASE_MESH.to_vec();

        // Scale for rubber (with thickness added back)
        for t in 0..13 {
            for vert in temp_rubber.iter_mut() {
                if vertex_matches(vert, &VERTS_BASE_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_BOTTOM[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_BASE_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_BASE_BOTTOM[6].x,
                            y: VERTS_BASE_BOTTOM[0].y,
                        },
                        -PI * 0.5,
                        base_radius + rubber_thickness,
                        Vec2 { x: 0.0, y: 0.0 },
                        fix_angle_scale,
                    );
                }
                if vertex_matches(vert, &VERTS_TIP_TOP[t]) {
                    apply_fix(
                        vert,
                        Vec2 {
                            x: VERTS_TIP_BOTTOM[6].x,
                            y: VERTS_TIP_BOTTOM[0].y,
                        },
                        PI * 0.5,
                        end_radius + rubber_thickness,
                        Vec2 {
                            x: 0.0,
                            y: flipper.flipper_radius_max,
                        },
                        fix_angle_scale,
                    );
                }
            }
        }

        let mut rubber_vertices = Vec::with_capacity(FLIPPER_BASE_NUM_VERTICES);
        for (i, temp_vert) in temp_rubber.iter().enumerate() {
            let rotated = rotation_matrix.multiply_vector(Vec3 {
                x: temp_vert.x,
                y: temp_vert.y,
                z: temp_vert.z,
            });

            let mut vert = Vertex3dNoTex2 {
                x: rotated.x,
                y: rotated.y,
                z: rotated.z * rubber_width + (surface_height + rubber_height),
                nx: FLIPPER_BASE_MESH[i].nx,
                ny: FLIPPER_BASE_MESH[i].ny,
                nz: FLIPPER_BASE_MESH[i].nz,
                tu: FLIPPER_BASE_MESH[i].tu,
                tv: FLIPPER_BASE_MESH[i].tv + 0.5,
            };

            // Apply normal rotation
            let rotated_normal = rotation_matrix.multiply_vector_no_translate(Vec3 {
                x: vert.nx,
                y: vert.ny,
                z: vert.nz,
            });
            vert.nx = rotated_normal.x;
            vert.ny = rotated_normal.y;
            vert.nz = rotated_normal.z;

            // Apply start angle rotation (do NOT translate to flipper center - that goes in node transform)
            let (sin_a, cos_a) = start_angle_rad.sin_cos();
            let final_x = vert.x * cos_a - vert.y * sin_a;
            let final_y = vert.x * sin_a + vert.y * cos_a;

            // Rotate normal as well
            let final_nx = vert.nx * cos_a - vert.ny * sin_a;
            let final_ny = vert.nx * sin_a + vert.ny * cos_a;

            rubber_vertices.push(VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    x: final_x,
                    y: final_y,
                    z: vert.z,
                    nx: final_nx,
                    ny: final_ny,
                    nz: vert.nz,
                    tu: vert.tu,
                    tv: vert.tv,
                },
            ));
        }

        // Build rubber mesh indices (same as base, no offset needed since separate mesh)
        let mut rubber_indices: Vec<VpxFace> = Vec::with_capacity(FLIPPER_BASE_NUM_INDICES / 3);
        for chunk in FLIPPER_BASE_INDICES.chunks(3) {
            rubber_indices.push(VpxFace {
                i0: chunk[0] as i64,
                i1: chunk[1] as i64,
                i2: chunk[2] as i64,
            });
        }

        Some((rubber_vertices, rubber_indices))
    } else {
        None
    };

    Some(FlipperMeshes {
        base: (base_vertices, base_indices),
        rubber,
        center: Vec3 {
            x: flipper.center.x,
            y: flipper.center.y,
            z: surface_height,
        },
    })
}
