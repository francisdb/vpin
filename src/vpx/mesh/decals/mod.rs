//! Decal mesh generation for expanded VPX export
//!
//! This module ports the decal mesh generation from Visual Pinball's decal.cpp.
//! Decals are simple textured quads that sit on surfaces with a slight Z offset.
//!
//! Decal types:
//! - **Image**: Uses an image texture from the table
//! - **Text**: Renders text (not supported in glTF export - text would need to be
//!   pre-rendered to a texture)
//!
//! The mesh is a simple quad (4 vertices) rendered as a triangle strip in VPinball.
//! For glTF export, we convert to indexed triangles.
//!
//! Ported from: VPinball/src/parts/decal.cpp

use crate::vpx::gameitem::decal::{Decal, DecalType};
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use log::warn;

/// Generate the decal mesh
///
/// Vertices are centered at origin.
///
/// # Arguments
/// * `decal` - The decal definition
///
/// # Returns
/// Tuple of (vertices, faces) or None if the decal should not be rendered
/// (e.g., text decals which we can't render without a font renderer)
pub fn build_decal_mesh(decal: &Decal) -> Option<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    // Skip text decals - we can't render text without a font renderer
    // In VPinball, text is rendered to a texture at runtime
    if decal.decal_type == DecalType::Text {
        warn!(
            "Skipping text decal '{}': text decals require runtime font rendering",
            decal.name
        );
        return None;
    }

    // Skip backglass decals - they are rendered in screen space in VPinball,
    // not as 3D geometry on the table. They would need separate handling
    // as a 2D overlay or separate scene.
    if decal.backglass {
        warn!(
            "Skipping backglass decal '{}': backglass decals are rendered in screen space, not as 3D geometry",
            decal.name
        );
        return None;
    }

    // Skip if no image is set for image decals
    if decal.decal_type == DecalType::Image && decal.image.is_empty() {
        warn!("Skipping image decal '{}': no image specified", decal.name);
        return None;
    }

    // Calculate dimensions
    // For image decals, leading and descent are 0
    let leading = 0.0_f32;
    let descent = 0.0_f32;

    let halfwidth = decal.width * 0.5;
    let halfheight = decal.height * 0.5;

    // Rotation
    let radangle = decal.rotation.to_radians();
    let sn = radangle.sin();
    let cs = radangle.cos();

    // Z position: 0 (node transform will add surface_height + 0.2)
    let z = 0.0;

    // Build the 4 vertices of the quad (centered at origin)
    // From decal.cpp lines 653-688
    // Note: VPinball uses TRIANGLESTRIP order, we'll convert to indexed triangles
    let vertices = vec![
        // Vertex 0: top-left
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: sn * (halfheight + leading) - cs * halfwidth,
                y: -cs * (halfheight + leading) - sn * halfwidth,
                z,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 0.0,
            },
        ),
        // Vertex 1: top-right
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: sn * (halfheight + leading) + cs * halfwidth,
                y: -cs * (halfheight + leading) + sn * halfwidth,
                z,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 1.0,
                tv: 0.0,
            },
        ),
        // Vertex 2: bottom-left
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: -sn * (halfheight + descent) - cs * halfwidth,
                y: cs * (halfheight + descent) - sn * halfwidth,
                z,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 1.0,
            },
        ),
        // Vertex 3: bottom-right
        VertexWrapper::new(
            [0u8; 32],
            Vertex3dNoTex2 {
                x: -sn * (halfheight + descent) + cs * halfwidth,
                y: cs * (halfheight + descent) + sn * halfwidth,
                z,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 1.0,
                tv: 1.0,
            },
        ),
    ];

    // Convert TRIANGLESTRIP (0,1,2,3) to indexed triangles
    // Triangle 1: 0, 1, 2
    // Triangle 2: 2, 1, 3
    let faces = vec![
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

    Some((vertices, faces))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::vertex2d::Vertex2D;

    fn create_test_decal(decal_type: DecalType) -> Decal {
        Decal {
            center: Vertex2D { x: 100.0, y: 200.0 },
            width: 50.0,
            height: 30.0,
            rotation: 0.0,
            decal_type,
            image: "test_image".to_string(),
            name: "TestDecal".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_image_decal_generates_mesh() {
        let decal = create_test_decal(DecalType::Image);
        let result = build_decal_mesh(&decal);
        assert!(result.is_some());

        let (vertices, faces) = result.unwrap();
        assert_eq!(vertices.len(), 4);
        assert_eq!(faces.len(), 2);
    }

    #[test]
    fn test_text_decal_returns_none() {
        let decal = create_test_decal(DecalType::Text);
        let result = build_decal_mesh(&decal);
        assert!(result.is_none());
    }

    #[test]
    fn test_image_decal_without_image_returns_none() {
        let mut decal = create_test_decal(DecalType::Image);
        decal.image = String::new();
        let result = build_decal_mesh(&decal);
        assert!(result.is_none());
    }

    #[test]
    fn test_backglass_decal_returns_none() {
        let mut decal = create_test_decal(DecalType::Image);
        decal.backglass = true;
        let result = build_decal_mesh(&decal);
        assert!(result.is_none());
    }

    #[test]
    fn test_decal_z_at_origin() {
        let decal = create_test_decal(DecalType::Image);
        let (vertices, _) = build_decal_mesh(&decal).unwrap();

        // All vertices should have z = 0 (node transform adds surface_height + 0.2)
        for v in &vertices {
            assert!(v.vertex.z.abs() < 0.001);
        }
    }

    #[test]
    fn test_decal_uv_coordinates() {
        let decal = create_test_decal(DecalType::Image);
        let (vertices, _) = build_decal_mesh(&decal).unwrap();

        // Check UV corners
        assert!((vertices[0].vertex.tu - 0.0).abs() < 0.001); // top-left
        assert!((vertices[0].vertex.tv - 0.0).abs() < 0.001);
        assert!((vertices[1].vertex.tu - 1.0).abs() < 0.001); // top-right
        assert!((vertices[1].vertex.tv - 0.0).abs() < 0.001);
        assert!((vertices[2].vertex.tu - 0.0).abs() < 0.001); // bottom-left
        assert!((vertices[2].vertex.tv - 1.0).abs() < 0.001);
        assert!((vertices[3].vertex.tu - 1.0).abs() < 0.001); // bottom-right
        assert!((vertices[3].vertex.tv - 1.0).abs() < 0.001);
    }
}
