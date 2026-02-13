//! Ball mesh generation for expanded VPX export
//!
//! This module generates ball (captive ball) meshes for glTF export.
//! Balls are rendered as textured spheres using a pre-defined unit sphere mesh
//! that is scaled by the ball's radius.
//!
//! Ball textures:
//! - If the ball has an `image` set, use that texture
//! - Otherwise, fall back to `gamedata.ball_image` (table default)
//! - If neither is set, use a default ball appearance
//!
//! Ported from: VPinball/src/parts/ball.cpp and meshes/ballMesh.h

mod ball_mesh;

use crate::vpx::gameitem::ball::Ball;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

#[allow(unused_imports)]
pub use ball_mesh::{BALL_INDICES, BALL_NUM_INDICES, BALL_NUM_VERTICES, BALL_VERTICES};

/// Build the ball mesh
///
/// # Arguments
/// * `ball` - The ball definition
///
/// # Returns
/// Tuple of (vertices, faces) for the ball mesh
pub(super) fn build_ball_mesh(ball: &Ball) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    let radius = ball.radius;

    // Transform the unit sphere vertices by the ball's radius and position
    let vertices: Vec<VertexWrapper> = BALL_VERTICES
        .iter()
        .map(|src| {
            VertexWrapper::new(
                [0u8; 32],
                Vertex3dNoTex2 {
                    // Scale by radius and translate to ball position
                    x: src.x * radius + ball.pos.x,
                    y: src.y * radius + ball.pos.y,
                    z: src.z * radius + ball.pos.z,
                    // Normals stay the same (unit sphere normals)
                    nx: src.nx,
                    ny: src.ny,
                    nz: src.nz,
                    // UV coordinates stay the same
                    tu: src.tu,
                    tv: src.tv,
                },
            )
        })
        .collect();

    // Convert indices to faces (groups of 3)
    let faces: Vec<VpxFace> = BALL_INDICES
        .chunks(3)
        .map(|chunk| VpxFace {
            i0: chunk[0] as i64,
            i1: chunk[1] as i64,
            i2: chunk[2] as i64,
        })
        .collect();

    (vertices, faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::ball::Ball;
    use crate::vpx::gameitem::vertex3d::Vertex3D;

    fn create_test_ball() -> Ball {
        let mut ball = Ball::default();
        ball.name = "TestBall".to_string();
        ball.pos = Vertex3D {
            x: 100.0,
            y: 200.0,
            z: 25.0,
        };
        ball.radius = 25.0;
        ball
    }

    #[test]
    fn test_ball_mesh_generation() {
        let ball = create_test_ball();
        let (vertices, faces) = build_ball_mesh(&ball);

        assert_eq!(vertices.len(), BALL_NUM_VERTICES);
        assert_eq!(faces.len(), BALL_NUM_INDICES / 3);
    }

    #[test]
    fn test_ball_mesh_position() {
        let ball = create_test_ball();
        let (vertices, _) = build_ball_mesh(&ball);

        // Check that the center of the ball is approximately at the ball position
        let sum_x: f32 = vertices.iter().map(|v| v.vertex.x).sum();
        let sum_y: f32 = vertices.iter().map(|v| v.vertex.y).sum();
        let sum_z: f32 = vertices.iter().map(|v| v.vertex.z).sum();

        let avg_x = sum_x / vertices.len() as f32;
        let avg_y = sum_y / vertices.len() as f32;
        let avg_z = sum_z / vertices.len() as f32;

        // The average should be close to the ball position
        assert!((avg_x - ball.pos.x).abs() < 1.0);
        assert!((avg_y - ball.pos.y).abs() < 1.0);
        assert!((avg_z - ball.pos.z).abs() < 1.0);
    }

    #[test]
    fn test_ball_mesh_radius() {
        let ball = create_test_ball();
        let (vertices, _) = build_ball_mesh(&ball);

        // Check that the vertices are approximately at the correct distance from center
        for v in &vertices {
            let dx = v.vertex.x - ball.pos.x;
            let dy = v.vertex.y - ball.pos.y;
            let dz = v.vertex.z - ball.pos.z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            // Distance should be close to radius
            assert!(
                (distance - ball.radius).abs() < 0.1,
                "Vertex distance {} should be close to radius {}",
                distance,
                ball.radius
            );
        }
    }
}
