//! Playfield mesh generation for expanded VPX export
//!
//! This module generates the implicit playfield mesh when no explicit
//! `playfield_mesh` primitive exists in the table.
//!
//! Ported from: VPinball player.cpp
//!
//! The playfield is a simple quad at z=0 covering the table bounds.

use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;

/// Build an implicit playfield mesh (quad at z=0) like VPinball does when no explicit playfield_mesh exists
///
/// From VPinball player.cpp:
/// - 4 vertices at corners: (left,top), (right,top), (left,bottom), (right,bottom)
/// - z = 0, normal = (0, 0, 1)
/// - UV: (0,0), (1,0), (0,1), (1,1)
/// - 6 indices: [0,1,2], [2,1,3]
///
/// # Arguments
/// * `left` - Left edge of the playfield (table bounds)
/// * `top` - Top edge of the playfield (table bounds)
/// * `right` - Right edge of the playfield (table bounds)
/// * `bottom` - Bottom edge of the playfield (table bounds)
///
/// # Returns
/// A tuple of (vertices, indices) for the playfield mesh
pub fn build_playfield_mesh(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> (Vec<VertexWrapper>, Vec<VpxFace>) {
    // Create 4 vertices matching VPinball's layout:
    // offs = x + y * 2, so:
    // (x=0,y=0) -> offs=0: left, top
    // (x=1,y=0) -> offs=1: right, top
    // (x=0,y=1) -> offs=2: left, bottom
    // (x=1,y=1) -> offs=3: right, bottom
    let vertices = vec![
        // offs=0: (x=0, y=0) -> left, top
        VertexWrapper::new(
            [0u8; 32], // Not needed for export, just a placeholder
            Vertex3dNoTex2 {
                x: left,
                y: top,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 0.0,
            },
        ),
        // offs=1: (x=1, y=0) -> right, top
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: right,
                y: top,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 1.0,
                tv: 0.0,
            },
        ),
        // offs=2: (x=0, y=1) -> left, bottom
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: left,
                y: bottom,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 1.0,
            },
        ),
        // offs=3: (x=1, y=1) -> right, bottom
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: right,
                y: bottom,
                z: 0.0,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 1.0,
                tv: 1.0,
            },
        ),
    ];

    // Indices from VPinball: [0,1,2], [2,1,3]
    let indices = vec![
        VpxFace {
            i0: 0,
            i1: 1,
            i2: 2,
        },
        VpxFace {
            i0: 2,
            i1: 1,
            i2: 3,
        },
    ];

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_playfield_mesh_creates_quad() {
        let (vertices, indices) = build_playfield_mesh(0.0, 0.0, 1000.0, 2000.0);

        // Should have 4 vertices
        assert_eq!(vertices.len(), 4);

        // Should have 2 triangles (6 indices total)
        assert_eq!(indices.len(), 2);

        // Check vertex positions
        assert_eq!(vertices[0].vertex.x, 0.0); // left
        assert_eq!(vertices[0].vertex.y, 0.0); // top
        assert_eq!(vertices[1].vertex.x, 1000.0); // right
        assert_eq!(vertices[1].vertex.y, 0.0); // top
        assert_eq!(vertices[2].vertex.x, 0.0); // left
        assert_eq!(vertices[2].vertex.y, 2000.0); // bottom
        assert_eq!(vertices[3].vertex.x, 1000.0); // right
        assert_eq!(vertices[3].vertex.y, 2000.0); // bottom

        // All vertices at z=0
        for v in &vertices {
            assert_eq!(v.vertex.z, 0.0);
        }

        // All normals pointing up (0, 0, 1)
        for v in &vertices {
            assert_eq!(v.vertex.nx, 0.0);
            assert_eq!(v.vertex.ny, 0.0);
            assert_eq!(v.vertex.nz, 1.0);
        }

        // Check UVs
        assert_eq!(vertices[0].vertex.tu, 0.0);
        assert_eq!(vertices[0].vertex.tv, 0.0);
        assert_eq!(vertices[1].vertex.tu, 1.0);
        assert_eq!(vertices[1].vertex.tv, 0.0);
        assert_eq!(vertices[2].vertex.tu, 0.0);
        assert_eq!(vertices[2].vertex.tv, 1.0);
        assert_eq!(vertices[3].vertex.tu, 1.0);
        assert_eq!(vertices[3].vertex.tv, 1.0);
    }
}
