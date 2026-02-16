//! glTF utilities for creating glTF/GLB files.
//!
//! This module provides utilities for working with glTF (GL Transmission Format) files,
//! including a fluent builder API for creating glTF materials.
//!
//! <https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html>
//!

use serde::Serialize;
use serde_json::Value;

/// Alpha blending mode for glTF materials.
///
/// According to the glTF 2.0 specification, when `alphaMode` is not specified,
/// the default is `OPAQUE`. This enum only includes `Mask` and `Blend` since
/// `OPAQUE` is the default and doesn't need to be explicitly set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlphaMode {
    /// The rendered output is fully opaque and any alpha value is ignored. (default)
    #[allow(dead_code)]
    Opaque,
    /// Alpha values below `alphaCutoff` are rendered as fully transparent,
    /// above as fully opaque.
    Mask,
    /// Alpha value is used to composite source and destination areas.
    Blend,
}

/// Texture reference info for glTF.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextureInfo {
    /// The index of the texture in the glTF textures array.
    pub index: usize,
}

/// PBR metallic roughness properties for glTF materials.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    /// The RGBA base color factor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f32; 4]>,
    /// Reference to a base color texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    /// The metalness of the material (0.0 = dielectric, 1.0 = metal).
    pub metallic_factor: f32,
    /// The roughness of the material (0.0 = smooth, 1.0 = rough).
    pub roughness_factor: f32,
}

/// KHR_materials_transmission extension data.
///
/// This extension allows specifying transmission of light through a material,
/// enabling effects like glass, water, and other refractive materials.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransmissionExtension {
    /// The base percentage of light transmitted through the surface.
    pub transmission_factor: f32,
}

/// Material extensions container.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MaterialExtensions {
    /// KHR_materials_transmission extension for light transmission effects.
    #[serde(rename = "KHR_materials_transmission")]
    pub transmission: TransmissionExtension,
}

/// A builder for creating glTF material JSON with serde serialization.
///
/// This provides a fluent builder API for constructing glTF material definitions
/// that serialize directly to JSON compatible with the glTF 2.0 specification.
///
/// # Required Properties
///
/// - `name`: The material name
/// - `metallic`: Metalness factor (0.0 = dielectric, 1.0 = metal)
/// - `roughness`: Roughness factor (0.0 = smooth/glossy, 1.0 = rough/matte)
///
/// # Optional Properties
///
/// All other properties can be set using builder methods:
/// - `.base_color()` - RGBA base color factor
/// - `.texture()` - Base color texture index
/// - `.alpha_blend()` / `.alpha_mask()` - Alpha blending mode
/// - `.double_sided()` - Render both sides
/// - `.transmission()` - Light transmission (glass effects)
/// - `.extras()` - Application-specific custom data
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfMaterialBuilder {
    name: String,
    pbr_metallic_roughness: PbrMetallicRoughness,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_mode: Option<AlphaMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_cutoff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    double_sided: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<MaterialExtensions>,
    /// Application-specific data (glTF extras field).
    #[serde(skip_serializing_if = "Option::is_none")]
    extras: Option<Value>,
}

impl GltfMaterialBuilder {
    /// Create a new material builder with required properties.
    ///
    /// # Arguments
    ///
    /// * `name` - The material name (will be visible in glTF viewers)
    /// * `metallic` - Metalness factor (0.0 = dielectric like plastic, 1.0 = metal)
    /// * `roughness` - Roughness factor (0.0 = smooth/mirror, 1.0 = rough/matte)
    pub fn new(name: impl Into<String>, metallic: f32, roughness: f32) -> Self {
        Self {
            name: name.into(),
            pbr_metallic_roughness: PbrMetallicRoughness {
                base_color_factor: None,
                base_color_texture: None,
                metallic_factor: metallic,
                roughness_factor: roughness,
            },
            alpha_mode: None,
            alpha_cutoff: None,
            double_sided: None,
            extensions: None,
            extras: None,
        }
    }

    /// Set the base color factor (RGBA, 0-1 range).
    ///
    /// This color is multiplied with the base color texture if one is set.
    pub fn base_color(mut self, color: [f32; 4]) -> Self {
        self.pbr_metallic_roughness.base_color_factor = Some(color);
        self
    }

    /// Set the base color texture index.
    ///
    /// The index refers to the texture array in the glTF file.
    pub fn texture(mut self, texture_idx: usize) -> Self {
        self.pbr_metallic_roughness.base_color_texture = Some(TextureInfo { index: texture_idx });
        self
    }

    /// Optionally set the base color texture index.
    ///
    /// Convenience method that only sets the texture if `Some` is provided.
    pub fn texture_opt(self, texture_idx: Option<usize>) -> Self {
        if let Some(idx) = texture_idx {
            self.texture(idx)
        } else {
            self
        }
    }

    /// Set alpha mode to BLEND for transparent materials.
    ///
    /// In BLEND mode, the alpha value from the base color (and texture) is used
    /// to composite the material with the background.
    pub fn alpha_blend(mut self) -> Self {
        self.alpha_mode = Some(AlphaMode::Blend);
        self
    }

    /// Set alpha mode to MASK with the given cutoff value.
    ///
    /// In MASK mode, pixels with alpha below the cutoff are fully transparent,
    /// and pixels above the cutoff are fully opaque. This is useful for
    /// decals, leaves, fences, etc.
    ///
    /// # Arguments
    ///
    /// * `cutoff` - Alpha threshold (typically 0.5). Values below this are transparent.
    pub fn alpha_mask(mut self, cutoff: f32) -> Self {
        self.alpha_mode = Some(AlphaMode::Mask);
        self.alpha_cutoff = Some(cutoff);
        self
    }

    /// Conditionally set alpha mode to BLEND if the condition is true.
    pub fn alpha_blend_if(self, condition: bool) -> Self {
        if condition { self.alpha_blend() } else { self }
    }

    /// Conditionally set alpha mode to MASK if the condition is true.
    pub fn alpha_mask_if(self, condition: bool, cutoff: f32) -> Self {
        if condition {
            self.alpha_mask(cutoff)
        } else {
            self
        }
    }

    /// Mark the material as double-sided.
    ///
    /// When true, back-face culling is disabled and both sides of triangles
    /// are rendered. Useful for thin surfaces like leaves, paper, or cloth.
    pub fn double_sided(mut self) -> Self {
        self.double_sided = Some(true);
        self
    }

    /// Add KHR_materials_transmission extension for light transmission.
    ///
    /// This enables light to pass through the material, creating effects
    /// like glass, water, or thin plastic. A transmission factor of 1.0
    /// means full transmission (fully transparent to light).
    ///
    /// Note: This extension requires viewer support for KHR_materials_transmission.
    ///
    /// # Arguments
    ///
    /// * `factor` - Transmission factor (0.0 = opaque, 1.0 = fully transmissive)
    pub fn transmission(mut self, factor: f32) -> Self {
        self.extensions = Some(MaterialExtensions {
            transmission: TransmissionExtension {
                transmission_factor: factor,
            },
        });
        self
    }

    /// Set application-specific custom data in the glTF `extras` field.
    ///
    /// The glTF specification allows any object to have an `extras` field
    /// containing arbitrary application-specific JSON data. This can be used
    /// to store custom properties that are not part of the glTF specification.
    ///
    /// # Arguments
    ///
    /// * `value` - Any JSON value (object, array, string, number, etc.)
    ///
    #[allow(dead_code)]
    pub fn extras(mut self, value: Value) -> Self {
        self.extras = Some(value);
        self
    }

    /// Build the final JSON value.
    ///
    /// Serializes the material builder to a `serde_json::Value` that can be
    /// included in a glTF materials array.
    pub fn build(self) -> Value {
        serde_json::to_value(self).expect("GltfMaterialBuilder serialization should never fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_material() {
        let material = GltfMaterialBuilder::new("Test", 0.0, 0.5).build();

        assert_eq!(material["name"], "Test");
        assert_eq!(material["pbrMetallicRoughness"]["metallicFactor"], 0.0);
        assert_eq!(material["pbrMetallicRoughness"]["roughnessFactor"], 0.5);
        assert!(material.get("alphaMode").is_none());
        assert!(material.get("doubleSided").is_none());
    }

    #[test]
    fn test_material_with_base_color() {
        let material = GltfMaterialBuilder::new("Red", 0.0, 0.5)
            .base_color([1.0, 0.0, 0.0, 1.0])
            .build();

        let color = material["pbrMetallicRoughness"]["baseColorFactor"]
            .as_array()
            .unwrap();
        assert_eq!(color.len(), 4);
        assert_eq!(color[0], 1.0);
        assert_eq!(color[1], 0.0);
        assert_eq!(color[2], 0.0);
        assert_eq!(color[3], 1.0);
    }

    #[test]
    fn test_material_with_texture() {
        let material = GltfMaterialBuilder::new("Textured", 0.0, 0.5)
            .texture(42)
            .build();

        assert_eq!(
            material["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            42
        );
    }

    #[test]
    fn test_alpha_blend() {
        let material = GltfMaterialBuilder::new("Transparent", 0.0, 0.5)
            .alpha_blend()
            .build();

        assert_eq!(material["alphaMode"], "BLEND");
    }

    #[test]
    fn test_alpha_mask() {
        let material = GltfMaterialBuilder::new("Masked", 0.0, 0.5)
            .alpha_mask(0.5)
            .build();

        assert_eq!(material["alphaMode"], "MASK");
        assert_eq!(material["alphaCutoff"], 0.5);
    }

    #[test]
    fn test_double_sided() {
        let material = GltfMaterialBuilder::new("TwoSided", 0.0, 0.5)
            .double_sided()
            .build();

        assert_eq!(material["doubleSided"], true);
    }

    #[test]
    fn test_transmission() {
        let material = GltfMaterialBuilder::new("Glass", 0.0, 0.1)
            .transmission(0.9)
            .build();

        let transmission =
            material["extensions"]["KHR_materials_transmission"]["transmissionFactor"]
                .as_f64()
                .unwrap();
        assert!((transmission - 0.9).abs() < 0.0001);
    }

    #[test]
    fn test_complex_material() {
        let material = GltfMaterialBuilder::new("Complex", 0.5, 0.3)
            .base_color([0.8, 0.2, 0.1, 0.9])
            .texture(5)
            .alpha_blend()
            .double_sided()
            .transmission(0.5)
            .build();

        assert_eq!(material["name"], "Complex");
        assert_eq!(material["pbrMetallicRoughness"]["metallicFactor"], 0.5);
        let roughness = material["pbrMetallicRoughness"]["roughnessFactor"]
            .as_f64()
            .unwrap();
        assert!((roughness - 0.3).abs() < 0.0001);
        assert_eq!(
            material["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            5
        );
        assert_eq!(material["alphaMode"], "BLEND");
        assert_eq!(material["doubleSided"], true);
        assert_eq!(
            material["extensions"]["KHR_materials_transmission"]["transmissionFactor"],
            0.5
        );
    }

    #[test]
    fn test_conditional_alpha_blend() {
        let with_blend = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .alpha_blend_if(true)
            .build();
        assert_eq!(with_blend["alphaMode"], "BLEND");

        let without_blend = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .alpha_blend_if(false)
            .build();
        assert!(without_blend.get("alphaMode").is_none());
    }

    #[test]
    fn test_conditional_alpha_mask() {
        let with_mask = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .alpha_mask_if(true, 0.25)
            .build();
        assert_eq!(with_mask["alphaMode"], "MASK");
        assert_eq!(with_mask["alphaCutoff"], 0.25);

        let without_mask = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .alpha_mask_if(false, 0.25)
            .build();
        assert!(without_mask.get("alphaMode").is_none());
    }

    #[test]
    fn test_texture_opt_some() {
        let material = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .texture_opt(Some(10))
            .build();
        assert_eq!(
            material["pbrMetallicRoughness"]["baseColorTexture"]["index"],
            10
        );
    }

    #[test]
    fn test_texture_opt_none() {
        let material = GltfMaterialBuilder::new("Test", 0.0, 0.5)
            .texture_opt(None)
            .build();
        assert!(
            material["pbrMetallicRoughness"]
                .get("baseColorTexture")
                .is_none()
        );
    }

    #[test]
    fn test_extras() {
        use serde_json::json;

        let material = GltfMaterialBuilder::new("WithExtras", 0.0, 0.5)
            .extras(json!({
                "vpinball": {
                    "material_type": "plastic",
                    "reflection_enabled": true
                },
                "custom_value": 42
            }))
            .build();

        assert_eq!(material["extras"]["vpinball"]["material_type"], "plastic");
        assert_eq!(material["extras"]["vpinball"]["reflection_enabled"], true);
        assert_eq!(material["extras"]["custom_value"], 42);
    }

    #[test]
    fn test_no_extras_by_default() {
        let material = GltfMaterialBuilder::new("NoExtras", 0.0, 0.5).build();
        assert!(material.get("extras").is_none());
    }
}
