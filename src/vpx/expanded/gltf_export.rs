//! Whole-table GLTF/GLB export
//!
//! This module provides functionality to export an entire VPX table as a single
//! GLTF/GLB file containing all meshes (primitives, walls, ramps, rubbers, flashers).
//!
//! ## Coordinate System Transformation
//!
//! VPinball uses a left-handed coordinate system with Z-up:
//! - X: right (across the playfield)
//! - Y: towards the player (down the playfield)
//! - Z: up (towards the glass)
//!
//! glTF uses a right-handed coordinate system with Y-up:
//! - X: right
//! - Y: up
//! - Z: towards the viewer (forward)
//!
//! The export applies this transformation (table faces the camera):
//! - VPX X → glTF X (origin stays at left back corner)
//! - VPX Y → glTF Z (player side faces camera)
//! - VPX Z → glTF Y (up)
//!
//! Triangle winding order is reversed to convert from left-handed to right-handed.

use super::WriteError;
use super::bumpers::build_bumper_meshes;
use super::flashers::build_flasher_mesh;
use super::flippers::build_flipper_meshes;
use super::gates::build_gate_meshes;
use super::hittargets::build_hit_target_mesh;
use super::ramps::build_ramp_mesh;
use super::rubbers::build_rubber_mesh;
use super::spinners::build_spinner_meshes;
use super::triggers::build_trigger_mesh;
use super::walls::{TableDimensions, build_wall_meshes};
use crate::filesystem::FileSystem;
use crate::vpx::VPX;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::light::Light;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gltf::{
    GLB_BIN_CHUNK_TYPE, GLB_CHUNK_HEADER_BYTES, GLB_HEADER_BYTES, GLB_JSON_CHUNK_TYPE,
    GLTF_COMPONENT_TYPE_FLOAT, GLTF_COMPONENT_TYPE_UNSIGNED_INT,
    GLTF_COMPONENT_TYPE_UNSIGNED_SHORT, GLTF_FILTER_LINEAR, GLTF_FILTER_LINEAR_MIPMAP_LINEAR,
    GLTF_MAGIC, GLTF_PRIMITIVE_MODE_TRIANGLES, GLTF_TARGET_ARRAY_BUFFER,
    GLTF_TARGET_ELEMENT_ARRAY_BUFFER, GLTF_VERSION, GLTF_WRAP_REPEAT,
};
use crate::vpx::image::ImageData;
use crate::vpx::material::MaterialType;
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::VpxFace;
use crate::vpx::units::vpu_to_m;
use byteorder::{LittleEndian, WriteBytesExt};
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::path::Path;

/// Special material name for the playfield
const PLAYFIELD_MATERIAL_NAME: &str = "__playfield__";

/// A named mesh ready for GLTF export
struct NamedMesh {
    name: String,
    vertices: Vec<VertexWrapper>,
    indices: Vec<VpxFace>,
    material_name: Option<String>,
    /// Optional texture name (image_a for flashers)
    texture_name: Option<String>,
    /// Optional color tint for the texture (RGBA, 0-1 range)
    /// Used for flashers to apply their color and alpha
    color_tint: Option<[f32; 4]>,
    /// Layer name for organizing meshes in the scene hierarchy
    layer_name: Option<String>,
    /// Optional light transmission factor (0.0 = opaque, 1.0 = fully transmissive)
    /// Used for KHR_materials_transmission extension.
    /// Derived from VPinball's disable_lighting_below: transmission = 1.0 - disable_lighting_below
    transmission_factor: Option<f32>,
}

// Re-export camera types from the camera module
use super::camera::GltfCamera;

/// Transform primitive vertices using the primitive's transformation matrix.
///
/// All transformations are done in VPX coordinate space.
/// The coordinate conversion to glTF happens when writing the GLB.
///
/// Ported from VPinball's Primitive::RecalculateMatrices() in primitive.cpp:
/// ```cpp
/// RTmatrix = ((MatrixTranslate(tra) * MatrixRotateZ(rot[2])) * MatrixRotateY(rot[1])) * MatrixRotateX(rot[0]);
/// RTmatrix = ((RTmatrix * MatrixRotateZ(obj_rot[8])) * MatrixRotateY(obj_rot[7])) * MatrixRotateX(obj_rot[6]);
/// fullMatrix = (MatrixScale(size) * RTmatrix) * MatrixTranslate(position);
/// ```
fn transform_primitive_vertices(
    vertices: Vec<VertexWrapper>,
    primitive: &crate::vpx::gameitem::primitive::Primitive,
) -> Vec<VertexWrapper> {
    use crate::vpx::math::{Matrix3D, Vertex3D, deg_to_rad};

    let pos = &primitive.position;
    let size = &primitive.size;
    let rot = &primitive.rot_and_tra;

    // rot_and_tra indices:
    // 0-2: RotX, RotY, RotZ (degrees)
    // 3-5: TraX, TraY, TraZ
    // 6-8: ObjRotX, ObjRotY, ObjRotZ (degrees)

    // Build the transformation matrix matching VPinball's RecalculateMatrices()
    // RTmatrix = Translate(tra) * RotZ * RotY * RotX
    let rt_matrix = Matrix3D::translate(rot[3], rot[4], rot[5])
        * Matrix3D::rotate_z(deg_to_rad(rot[2]))
        * Matrix3D::rotate_y(deg_to_rad(rot[1]))
        * Matrix3D::rotate_x(deg_to_rad(rot[0]));

    // RTmatrix = RTmatrix * ObjRotZ * ObjRotY * ObjRotX
    let rt_matrix = rt_matrix
        * Matrix3D::rotate_z(deg_to_rad(rot[8]))
        * Matrix3D::rotate_y(deg_to_rad(rot[7]))
        * Matrix3D::rotate_x(deg_to_rad(rot[6]));

    // fullMatrix = Scale * RTmatrix * Translate(position)
    let full_matrix = Matrix3D::scale(size.x, size.y, size.z)
        * rt_matrix
        * Matrix3D::translate(pos.x, pos.y, pos.z);

    vertices
        .into_iter()
        .map(|mut vw| {
            // Transform position
            let v = Vertex3D::new(vw.vertex.x, vw.vertex.y, vw.vertex.z);
            let transformed = full_matrix.transform_vertex(v);
            vw.vertex.x = transformed.x;
            vw.vertex.y = transformed.y;
            vw.vertex.z = transformed.z;

            // Transform normals (rotation only, no translation/scale)
            let nx = vw.vertex.nx;
            let ny = vw.vertex.ny;
            let nz = vw.vertex.nz;

            if !nx.is_nan() && !ny.is_nan() && !nz.is_nan() {
                let normal = full_matrix.transform_normal(nx, ny, nz);
                // Normalize
                let len = normal.length();
                if len > 0.0 {
                    vw.vertex.nx = normal.x / len;
                    vw.vertex.ny = normal.y / len;
                    vw.vertex.nz = normal.z / len;
                }
            }

            vw
        })
        .collect()
}

/// A simple material representation for glTF export
struct GltfMaterial {
    name: String,
    base_color: [f32; 4], // RGBA
    metallic: f32,
    roughness: f32,
    /// Whether opacity/alpha blending is active
    /// When false, the material is fully opaque regardless of the opacity value
    opacity_active: bool,
}

/// Collect all materials from a VPX file
fn collect_materials(vpx: &VPX) -> HashMap<String, GltfMaterial> {
    let mut materials = HashMap::new();

    // Try new format first (10.8+)
    if let Some(ref mats) = vpx.gamedata.materials {
        for mat in mats {
            let gltf_mat = GltfMaterial {
                name: mat.name.clone(),
                base_color: [
                    mat.base_color.r as f32 / 255.0,
                    mat.base_color.g as f32 / 255.0,
                    mat.base_color.b as f32 / 255.0,
                    if mat.opacity_active { mat.opacity } else { 1.0 },
                ],
                metallic: if mat.type_ == MaterialType::Metal {
                    1.0
                } else {
                    0.0
                },
                // VPinball roughness: 0=diffuse(matte)..1=specular(shiny)
                // glTF roughness: 0=specular(shiny)..1=diffuse(matte)
                // So we invert: glTF_roughness = 1.0 - VPX_roughness
                roughness: 1.0 - mat.roughness,
                opacity_active: mat.opacity_active,
            };
            materials.insert(mat.name.clone(), gltf_mat);
        }
    } else {
        // Fall back to old format
        for mat in &vpx.gamedata.materials_old {
            // opacity_active is encoded in the lowest bit of opacity_active_edge_alpha
            let opacity_active = (mat.opacity_active_edge_alpha & 1) != 0;
            let gltf_mat = GltfMaterial {
                name: mat.name.clone(),
                base_color: [
                    mat.base_color.r as f32 / 255.0,
                    mat.base_color.g as f32 / 255.0,
                    mat.base_color.b as f32 / 255.0,
                    if opacity_active { mat.opacity } else { 1.0 },
                ],
                metallic: if mat.is_metal { 1.0 } else { 0.0 },
                // VPinball roughness: 0=diffuse(matte)..1=specular(shiny)
                // glTF roughness: 0=specular(shiny)..1=diffuse(matte)
                // So we invert: glTF_roughness = 1.0 - VPX_roughness
                roughness: 1.0 - mat.roughness,
                opacity_active,
            };
            materials.insert(mat.name.clone(), gltf_mat);
        }
    }

    materials
}

/// Find the playfield image in the VPX
fn find_playfield_image(vpx: &VPX) -> Option<&ImageData> {
    let playfield_image_name = &vpx.gamedata.image;
    if playfield_image_name.is_empty() {
        return None;
    }
    vpx.images
        .iter()
        .find(|img| img.name.eq_ignore_ascii_case(playfield_image_name))
}

/// Get the image data bytes from an ImageData, converting bitmap to PNG if needed
fn get_image_bytes(image: &ImageData) -> Option<Vec<u8>> {
    // Prefer jpeg/png data over raw bitmap
    if let Some(ref jpeg) = image.jpeg {
        Some(jpeg.data.clone())
    } else if let Some(ref bits) = image.bits {
        // Convert bitmap to PNG
        use crate::vpx::image::vpx_image_to_dynamic_image;
        use std::io::Cursor;

        let dynamic_image =
            vpx_image_to_dynamic_image(&bits.lzw_compressed_data, image.width, image.height);

        let mut png_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        if dynamic_image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .is_ok()
        {
            Some(png_bytes)
        } else {
            None
        }
    } else {
        None
    }
}

/// Build an implicit playfield mesh (quad at z=0) like VPinball does when no explicit playfield_mesh exists
///
/// From VPinball player.cpp:
/// - 4 vertices at corners: (left,top), (right,top), (left,bottom), (right,bottom)
/// - z = 0, normal = (0, 0, 1)
/// - UV: (0,0), (1,0), (0,1), (1,1)
/// - 6 indices: [0,1,2], [2,1,3]
fn build_implicit_playfield_mesh(vpx: &VPX, playfield_material_name: &str) -> NamedMesh {
    let left = vpx.gamedata.left;
    let top = vpx.gamedata.top;
    let right = vpx.gamedata.right;
    let bottom = vpx.gamedata.bottom;

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

    NamedMesh {
        name: "playfield_mesh".to_string(),
        vertices,
        indices,
        material_name: Some(playfield_material_name.to_string()),
        texture_name: None,
        color_tint: None,
        layer_name: None,
        transmission_factor: None,
    }
}

/// Get the effective layer name for a game item
/// Uses editor_layer_name if set (prefixed with "Layer_"), otherwise falls back to "Layer_{editor_layer + 1}"
fn get_layer_name(editor_layer_name: &Option<String>, editor_layer: Option<u32>) -> Option<String> {
    if let Some(name) = editor_layer_name
        && !name.is_empty()
    {
        // Prefix custom layer names with "Layer_" for consistency
        return Some(format!("Layer_{}", name));
    }
    // Fall back to layer number if available
    editor_layer.map(|layer| format!("Layer_{}", layer + 1))
}

/// Calculate the effective range for a light in meters.
///
/// The range is determined in the following priority:
/// 1. Calculate the maximum distance from the center to any drag point
/// 2. Fall back to `falloff_radius` if no drag points are defined
///
/// This range represents the light's area of effect. For lights with drag points
/// (polygon-shaped lights), the range approximates the polygon boundary since
/// glTF's KHR_lights_punctual extension only supports point lights.
#[allow(unused)]
fn calculate_light_range(light: &Light) -> f32 {
    if !light.drag_points.is_empty() {
        // Calculate maximum distance from center to any drag point
        let max_dist_sq = light
            .drag_points
            .iter()
            .map(|dp| {
                let dx = dp.x - light.center.x;
                let dy = dp.y - light.center.y;
                dx * dx + dy * dy
            })
            .fold(0.0f32, |a, b| a.max(b));
        vpu_to_m(max_dist_sq.sqrt())
    } else {
        // Fall back to falloff_radius if no drag points
        vpu_to_m(light.falloff_radius)
    }
}

/// Collect all meshes from a VPX file
fn collect_meshes(vpx: &VPX) -> Vec<NamedMesh> {
    let mut meshes = Vec::new();
    let mut has_explicit_playfield = false;

    // Get the playfield material name to assign to playfield primitives
    // VPinball uses the table's playfield material, or we use a special name
    let playfield_material_name = if vpx.gamedata.playfield_material.is_empty() {
        PLAYFIELD_MATERIAL_NAME.to_string()
    } else {
        vpx.gamedata.playfield_material.clone()
    };

    // Table dimensions for UV calculation (wall tops use table-space UVs)
    let table_dims = TableDimensions::new(
        vpx.gamedata.left,
        vpx.gamedata.top,
        vpx.gamedata.right,
        vpx.gamedata.bottom,
    );

    for gameitem in &vpx.gameitems {
        match gameitem {
            GameItemEnum::Primitive(primitive) => {
                if !primitive.is_visible {
                    continue; // Skip invisible primitives
                }
                if let Ok(Some(read_mesh)) = primitive.read_mesh() {
                    let transformed = transform_primitive_vertices(read_mesh.vertices, primitive);

                    // If it's the playfield, VPinball assigns m_szMaterial and m_szImage from table settings
                    let is_playfield = primitive.is_playfield();

                    if is_playfield {
                        has_explicit_playfield = true;
                    }

                    // Determine material and texture:
                    // - If it's the playfield primitive, use the playfield material (which has the texture)
                    // - Otherwise, use the primitive's image as texture if set, or material
                    let (material_name, texture_name) = if is_playfield {
                        (Some(playfield_material_name.clone()), None)
                    } else if !primitive.image.is_empty() {
                        // Primitive has a texture image - use it
                        (None, Some(primitive.image.clone()))
                    } else if !primitive.material.is_empty() {
                        (Some(primitive.material.clone()), None)
                    } else {
                        (None, None)
                    };

                    // Calculate transmission factor from disable_lighting_below
                    // VPinball: disable_lighting_below 0.0 = fully transmits light, 1.0 = blocks light
                    // glTF KHR_materials_transmission: 0.0 = opaque, 1.0 = fully transmissive
                    // So: transmission = 1.0 - disable_lighting_below
                    let transmission_factor = primitive
                        .disable_lighting_below
                        .filter(|&v| v < 1.0) // Only set if < 1.0 (i.e., some transmission)
                        .map(|v| 1.0 - v);

                    meshes.push(NamedMesh {
                        name: primitive.name.clone(),
                        vertices: transformed,
                        indices: read_mesh.indices,
                        material_name,
                        texture_name,
                        color_tint: None,
                        layer_name: get_layer_name(
                            &primitive.editor_layer_name,
                            primitive.editor_layer,
                        ),
                        transmission_factor,
                    });
                }
            }
            GameItemEnum::Wall(wall) => {
                if !wall.is_top_bottom_visible && !wall.is_side_visible {
                    continue; // Skip fully invisible walls
                }
                if let Some(wall_meshes) = build_wall_meshes(wall, &table_dims) {
                    // Add top mesh if visible
                    if wall.is_top_bottom_visible
                        && let Some((vertices, indices)) = wall_meshes.top
                    {
                        // Top surface: use image (texture) or top_material
                        // Note: display_texture only affects editor preview, not runtime rendering
                        let (material_name, texture_name) = if !wall.image.is_empty() {
                            // Use texture for top surface
                            (None, Some(wall.image.clone()))
                        } else if !wall.top_material.is_empty() {
                            (Some(wall.top_material.clone()), None)
                        } else {
                            (None, None)
                        };

                        // Calculate transmission factor from disable_lighting_below
                        // VPinball: disable_lighting_below 0.0 = fully transmits light, 1.0 = blocks light
                        // glTF KHR_materials_transmission: 0.0 = opaque, 1.0 = fully transmissive
                        // So: transmission = 1.0 - disable_lighting_below
                        let transmission_factor = wall
                            .disable_lighting_below
                            .filter(|&v| v < 1.0) // Only set if < 1.0 (i.e., some transmission)
                            .map(|v| 1.0 - v);

                        meshes.push(NamedMesh {
                            name: format!("{}Top", wall.name),
                            vertices,
                            indices,
                            material_name,
                            texture_name,
                            color_tint: None,
                            layer_name: get_layer_name(&wall.editor_layer_name, wall.editor_layer),
                            transmission_factor,
                        });
                    }

                    // Add side mesh if visible
                    if wall.is_side_visible
                        && let Some((vertices, indices)) = wall_meshes.side
                    {
                        // Side surface: use side_image (texture) or side_material
                        // Note: display_texture only affects editor preview, not runtime rendering
                        let (material_name, texture_name) = if !wall.side_image.is_empty() {
                            // Use texture for side surface
                            (None, Some(wall.side_image.clone()))
                        } else if !wall.side_material.is_empty() {
                            (Some(wall.side_material.clone()), None)
                        } else {
                            (None, None)
                        };

                        // Side surfaces also use the same disable_lighting_below value
                        let transmission_factor = wall
                            .disable_lighting_below
                            .filter(|&v| v < 1.0)
                            .map(|v| 1.0 - v);

                        meshes.push(NamedMesh {
                            name: format!("{}Side", wall.name),
                            vertices,
                            indices,
                            material_name,
                            texture_name,
                            color_tint: None,
                            layer_name: get_layer_name(&wall.editor_layer_name, wall.editor_layer),
                            transmission_factor,
                        });
                    }
                }
            }
            GameItemEnum::Ramp(ramp) => {
                if !ramp.is_visible {
                    continue; // Skip invisible ramps
                }
                if let Some((vertices, indices)) = build_ramp_mesh(ramp) {
                    // Ramps use image as texture, or material if no image
                    let (material_name, texture_name) = if !ramp.image.is_empty() {
                        // Ramp has a texture image - use it
                        (None, Some(ramp.image.clone()))
                    } else if !ramp.material.is_empty() {
                        (Some(ramp.material.clone()), None)
                    } else {
                        (None, None)
                    };
                    meshes.push(NamedMesh {
                        name: ramp.name.clone(),
                        vertices,
                        indices,
                        material_name,
                        texture_name,
                        color_tint: None,
                        layer_name: get_layer_name(&ramp.editor_layer_name, ramp.editor_layer),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Rubber(rubber) => {
                if !rubber.is_visible {
                    continue; // Skip invisible rubbers
                }
                if let Some((vertices, indices)) = build_rubber_mesh(rubber) {
                    let material_name = if rubber.material.is_empty() {
                        None
                    } else {
                        Some(rubber.material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: rubber.name.clone(),
                        vertices,
                        indices,
                        material_name,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&rubber.editor_layer_name, rubber.editor_layer),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Flasher(flasher) => {
                if !flasher.is_visible {
                    continue; // Skip invisible flashers
                }
                if let Some((vertices, indices)) = build_flasher_mesh(flasher) {
                    // Flashers use image_a as their texture
                    let texture_name = if flasher.image_a.is_empty() {
                        None
                    } else {
                        Some(flasher.image_a.clone())
                    };
                    // Flashers have color tint and alpha (0-100)
                    // VPinball applies: color * (alpha * intensity_scale / 100.0)
                    let color_tint = Some([
                        flasher.color.r as f32 / 255.0,
                        flasher.color.g as f32 / 255.0,
                        flasher.color.b as f32 / 255.0,
                        flasher.alpha as f32 / 100.0,
                    ]);
                    meshes.push(NamedMesh {
                        name: flasher.name.clone(),
                        vertices,
                        indices,
                        material_name: None,
                        texture_name,
                        color_tint,
                        layer_name: get_layer_name(
                            &flasher.editor_layer_name,
                            flasher.editor_layer,
                        ),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Flipper(flipper) => {
                if !flipper.is_visible {
                    continue; // Skip invisible flippers
                }
                // TODO: get surface height from the table
                if let Some(flipper_meshes) = build_flipper_meshes(flipper, 0.0) {
                    // Add base flipper mesh
                    let (base_vertices, base_indices) = flipper_meshes.base;
                    let base_material = if flipper.material.is_empty() {
                        None
                    } else {
                        Some(flipper.material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: format!("{}Base", flipper.name),
                        vertices: base_vertices,
                        indices: base_indices,
                        material_name: base_material,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(
                            &flipper.editor_layer_name,
                            flipper.editor_layer,
                        ),
                        transmission_factor: None,
                    });

                    // Add rubber mesh if present
                    if let Some((rubber_vertices, rubber_indices)) = flipper_meshes.rubber {
                        let rubber_material = if flipper.rubber_material.is_empty() {
                            None
                        } else {
                            Some(flipper.rubber_material.clone())
                        };
                        meshes.push(NamedMesh {
                            name: format!("{}Rubber", flipper.name),
                            vertices: rubber_vertices,
                            indices: rubber_indices,
                            material_name: rubber_material,
                            texture_name: None,
                            color_tint: None,
                            layer_name: get_layer_name(
                                &flipper.editor_layer_name,
                                flipper.editor_layer,
                            ),
                            transmission_factor: None,
                        });
                    }
                }
            }
            GameItemEnum::Bumper(bumper) => {
                // TODO: get surface height from the table
                let bumper_meshes = build_bumper_meshes(bumper, 0.0);

                // Add base mesh if visible
                if let Some((base_vertices, base_indices)) = bumper_meshes.base {
                    let base_material = if bumper.base_material.is_empty() {
                        None
                    } else {
                        Some(bumper.base_material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: format!("{}Base", bumper.name),
                        vertices: base_vertices,
                        indices: base_indices,
                        material_name: base_material,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&bumper.editor_layer_name, bumper.editor_layer),
                        transmission_factor: None,
                    });
                }

                // Add socket mesh if visible
                if let Some((socket_vertices, socket_indices)) = bumper_meshes.socket {
                    let socket_material = if bumper.socket_material.is_empty() {
                        None
                    } else {
                        Some(bumper.socket_material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: format!("{}Socket", bumper.name),
                        vertices: socket_vertices,
                        indices: socket_indices,
                        material_name: socket_material,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&bumper.editor_layer_name, bumper.editor_layer),
                        transmission_factor: None,
                    });
                }

                // Add ring mesh if visible
                if let Some((ring_vertices, ring_indices)) = bumper_meshes.ring {
                    let ring_material = bumper
                        .ring_material
                        .as_ref()
                        .and_then(|m| if m.is_empty() { None } else { Some(m.clone()) });
                    meshes.push(NamedMesh {
                        name: format!("{}Ring", bumper.name),
                        vertices: ring_vertices,
                        indices: ring_indices,
                        material_name: ring_material,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&bumper.editor_layer_name, bumper.editor_layer),
                        transmission_factor: None,
                    });
                }

                // Add cap mesh if visible
                if let Some((cap_vertices, cap_indices)) = bumper_meshes.cap {
                    let cap_material = if bumper.cap_material.is_empty() {
                        None
                    } else {
                        Some(bumper.cap_material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: format!("{}Cap", bumper.name),
                        vertices: cap_vertices,
                        indices: cap_indices,
                        material_name: cap_material,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&bumper.editor_layer_name, bumper.editor_layer),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Spinner(spinner) => {
                if !spinner.is_visible {
                    continue; // Skip invisible spinners
                }
                // TODO: get surface height from the table
                let spinner_meshes = build_spinner_meshes(spinner, 0.0);

                // Add bracket mesh if visible
                if let Some((bracket_vertices, bracket_indices)) = spinner_meshes.bracket {
                    // Bracket uses a default metal material (no material property in VPX)
                    meshes.push(NamedMesh {
                        name: format!("{}Bracket", spinner.name),
                        vertices: bracket_vertices,
                        indices: bracket_indices,
                        material_name: None,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(
                            &spinner.editor_layer_name,
                            spinner.editor_layer,
                        ),
                        transmission_factor: None,
                    });
                }

                // Add plate mesh
                let (plate_vertices, plate_indices) = spinner_meshes.plate;
                let plate_material = if spinner.material.is_empty() {
                    None
                } else {
                    Some(spinner.material.clone())
                };
                let plate_texture = if spinner.image.is_empty() {
                    None
                } else {
                    Some(spinner.image.clone())
                };
                meshes.push(NamedMesh {
                    name: format!("{}Plate", spinner.name),
                    vertices: plate_vertices,
                    indices: plate_indices,
                    material_name: plate_material,
                    texture_name: plate_texture,
                    color_tint: None,
                    layer_name: get_layer_name(&spinner.editor_layer_name, spinner.editor_layer),
                    transmission_factor: None,
                });
            }
            GameItemEnum::HitTarget(hit_target) => {
                if let Some((vertices, indices)) = build_hit_target_mesh(hit_target) {
                    let material_name = if hit_target.material.is_empty() {
                        None
                    } else {
                        Some(hit_target.material.clone())
                    };
                    let texture_name = if hit_target.image.is_empty() {
                        None
                    } else {
                        Some(hit_target.image.clone())
                    };
                    meshes.push(NamedMesh {
                        name: hit_target.name.clone(),
                        vertices,
                        indices,
                        material_name,
                        texture_name,
                        color_tint: None,
                        layer_name: get_layer_name(
                            &hit_target.editor_layer_name,
                            hit_target.editor_layer,
                        ),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Gate(gate) => {
                // TODO: get surface height from the table
                if let Some(gate_meshes) = build_gate_meshes(gate, 0.0) {
                    let material_name = if gate.material.is_empty() {
                        None
                    } else {
                        Some(gate.material.clone())
                    };

                    // Add bracket mesh if visible
                    if let Some((bracket_vertices, bracket_indices)) = gate_meshes.bracket {
                        meshes.push(NamedMesh {
                            name: format!("{}Bracket", gate.name),
                            vertices: bracket_vertices,
                            indices: bracket_indices,
                            material_name: material_name.clone(),
                            texture_name: None,
                            color_tint: None,
                            layer_name: get_layer_name(&gate.editor_layer_name, gate.editor_layer),
                            transmission_factor: None,
                        });
                    }

                    // Add wire/plate mesh
                    let (wire_vertices, wire_indices) = gate_meshes.wire;
                    meshes.push(NamedMesh {
                        name: format!("{}Wire", gate.name),
                        vertices: wire_vertices,
                        indices: wire_indices,
                        material_name,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(&gate.editor_layer_name, gate.editor_layer),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Trigger(trigger) => {
                if !trigger.is_visible {
                    continue; // Skip invisible triggers
                }
                // TODO: get surface height from the table
                if let Some((vertices, indices)) = build_trigger_mesh(trigger, 0.0) {
                    let material_name = if trigger.material.is_empty() {
                        None
                    } else {
                        Some(trigger.material.clone())
                    };

                    meshes.push(NamedMesh {
                        name: trigger.name.clone(),
                        vertices,
                        indices,
                        material_name,
                        texture_name: None,
                        color_tint: None,
                        layer_name: get_layer_name(
                            &trigger.editor_layer_name,
                            trigger.editor_layer,
                        ),
                        transmission_factor: None,
                    });
                }
            }
            GameItemEnum::Light(light) => {
                // Only generate bulb meshes for lights with show_bulb_mesh enabled
                // Skip backglass lights
                if light.is_backglass || !light.show_bulb_mesh {
                    continue;
                }

                // TODO: Get actual surface height from the table
                let surface_height = 0.0;

                if let Some(light_meshes) = super::lights::build_light_meshes(light, surface_height)
                {
                    // Add bulb mesh
                    if let Some((vertices, indices)) = light_meshes.bulb {
                        meshes.push(NamedMesh {
                            name: format!("{}_bulb", light.name),
                            vertices,
                            indices,
                            material_name: None, // Bulb uses a glass-like material
                            texture_name: None,
                            color_tint: None,
                            layer_name: get_layer_name(
                                &light.editor_layer_name,
                                light.editor_layer,
                            ),
                            transmission_factor: Some(0.9), // Glass-like transmission
                        });
                    }

                    // Add socket mesh
                    if let Some((vertices, indices)) = light_meshes.socket {
                        meshes.push(NamedMesh {
                            name: format!("{}_socket", light.name),
                            vertices,
                            indices,
                            material_name: None, // Socket uses a metallic material
                            texture_name: None,
                            color_tint: None,
                            layer_name: get_layer_name(
                                &light.editor_layer_name,
                                light.editor_layer,
                            ),
                            transmission_factor: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // If no explicit playfield_mesh primitive was found, create an implicit one
    // This matches VPinball's behavior in player.cpp
    if !has_explicit_playfield {
        meshes.push(build_implicit_playfield_mesh(vpx, &playfield_material_name));
    }

    meshes
}

/// Build a combined GLTF payload with all meshes
fn build_combined_gltf_payload(
    vpx: &VPX,
    meshes: &[NamedMesh],
    materials: &HashMap<String, GltfMaterial>,
    images: &[ImageData],
    playfield_image: Option<&ImageData>,
    playfield_material_name: &str,
) -> Result<(serde_json::Value, Vec<u8>), WriteError> {
    if meshes.is_empty() {
        return Err(WriteError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No meshes to export",
        )));
    }

    // Build material name -> index map and create glTF materials array
    let mut material_index_map: HashMap<String, usize> = HashMap::new();
    let mut gltf_materials: Vec<serde_json::Value> = Vec::new();
    let mut gltf_textures: Vec<serde_json::Value> = Vec::new();
    let mut gltf_images: Vec<serde_json::Value> = Vec::new();
    let mut gltf_samplers: Vec<serde_json::Value> = Vec::new();

    // We'll store image data in the binary buffer, track where it starts
    let mut bin_data = Vec::new();
    let mut buffer_views = Vec::new();

    // Add playfield material with texture if available
    if let Some(image) = playfield_image
        && let Some(image_bytes) = get_image_bytes(image)
    {
        // Add sampler
        let sampler_idx = gltf_samplers.len();
        gltf_samplers.push(json!({
            "magFilter": GLTF_FILTER_LINEAR,
            "minFilter": GLTF_FILTER_LINEAR_MIPMAP_LINEAR,
            "wrapS": GLTF_WRAP_REPEAT,
            "wrapT": GLTF_WRAP_REPEAT
        }));

        // Write image data to binary buffer
        let image_offset = bin_data.len();
        bin_data.extend_from_slice(&image_bytes);
        let image_length = image_bytes.len();

        // Pad to 4-byte alignment for next data
        while bin_data.len() % 4 != 0 {
            bin_data.push(0);
        }

        // Add buffer view for image
        let image_buffer_view_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": image_offset,
            "byteLength": image_length
        }));

        // Add image referencing the buffer view
        let image_idx = gltf_images.len();
        // Determine MIME type: bitmap is converted to PNG, otherwise check the file extension
        let mime_type = if image.bits.is_some() || image.path.to_lowercase().ends_with(".png") {
            "image/png"
        } else {
            // FIXME this is wrong, we should check the file extension
            "image/jpeg"
        };
        gltf_images.push(json!({
            "bufferView": image_buffer_view_idx,
            "mimeType": mime_type,
            "name": image.name
        }));

        // Add texture
        let texture_idx = gltf_textures.len();
        gltf_textures.push(json!({
            "sampler": sampler_idx,
            "source": image_idx,
            "name": format!("{}_texture", image.name)
        }));

        // Add playfield material with texture
        material_index_map.insert(playfield_material_name.to_string(), gltf_materials.len());
        gltf_materials.push(json!({
            "name": playfield_material_name,
            "pbrMetallicRoughness": {
                "baseColorTexture": {
                    "index": texture_idx
                },
                "metallicFactor": 0.0,
                "roughnessFactor": 0.5
            }
            // Note: No alphaMode - playfield should be opaque (default is OPAQUE)
        }));
    }

    for (name, mat) in materials {
        // Skip if we already added a playfield material with texture
        if material_index_map.contains_key(name) {
            continue;
        }
        material_index_map.insert(name.clone(), gltf_materials.len());

        // VPinball only enables alpha blending when:
        // 1. opacity_active is true, AND
        // 2. opacity < 0.999 (the VPinball threshold)
        //
        // Values >= 0.999 with opacity_active=true are a special mode that enables
        // light-from-below transmission without actual visual transparency.
        // See VPinball Shader.cpp: "alpha blending is only performed if there is
        // an alpha channel or a 'meaningful' (not 0.999) alpha value"
        let needs_alpha_blend = mat.opacity_active && mat.base_color[3] < 0.999;

        if needs_alpha_blend {
            gltf_materials.push(json!({
                "name": mat.name,
                "pbrMetallicRoughness": {
                    "baseColorFactor": mat.base_color,
                    "metallicFactor": mat.metallic,
                    "roughnessFactor": mat.roughness
                },
                "alphaMode": "BLEND"
            }));
        } else {
            gltf_materials.push(json!({
                "name": mat.name,
                "pbrMetallicRoughness": {
                    "baseColorFactor": mat.base_color,
                    "metallicFactor": mat.metallic,
                    "roughnessFactor": mat.roughness
                }
                // No alphaMode = OPAQUE (default)
            }));
        }
    }

    // Build a map of image name -> ImageData for quick lookup
    let image_map: HashMap<String, &ImageData> = images
        .iter()
        .map(|img| (img.name.to_lowercase(), img))
        .collect();

    // Track which textures we've already added (by lowercase name) -> texture index
    let mut texture_index_map: HashMap<String, usize> = HashMap::new();

    // Track materials for meshes without color tint (can be shared)
    // Key: texture name (lowercase), Value: material index
    let mut texture_material_map: HashMap<String, usize> = HashMap::new();

    // Track whether we use the KHR_materials_transmission extension
    let mut uses_transmission_extension = false;

    // Track per-mesh material indices for meshes with color tint (unique materials)
    // Key: mesh index, Value: material index
    let mut mesh_material_map: HashMap<usize, usize> = HashMap::new();

    // First pass: create textures for all unique images
    for mesh in meshes.iter() {
        if let Some(ref texture_name) = mesh.texture_name {
            let texture_key = texture_name.to_lowercase();

            // Skip if we already created a texture for this image
            if texture_index_map.contains_key(&texture_key) {
                continue;
            }

            // Find the image data
            if let Some(image) = image_map.get(&texture_key)
                && let Some(image_bytes) = get_image_bytes(image)
            {
                // Add sampler (reuse if we already have one)
                let sampler_idx = if gltf_samplers.is_empty() {
                    gltf_samplers.push(json!({
                        "magFilter": GLTF_FILTER_LINEAR,
                        "minFilter": GLTF_FILTER_LINEAR_MIPMAP_LINEAR,
                        "wrapS": GLTF_WRAP_REPEAT,
                        "wrapT": GLTF_WRAP_REPEAT
                    }));
                    0
                } else {
                    0 // Reuse the first sampler
                };

                // Write image data to binary buffer
                let image_offset = bin_data.len();
                bin_data.extend_from_slice(&image_bytes);
                let image_length = image_bytes.len();

                // Pad to 4-byte alignment for next data
                while bin_data.len() % 4 != 0 {
                    bin_data.push(0);
                }

                // Add buffer view for image
                let image_buffer_view_idx = buffer_views.len();
                buffer_views.push(json!({
                    "buffer": 0,
                    "byteOffset": image_offset,
                    "byteLength": image_length
                }));

                // Add image referencing the buffer view
                let image_idx = gltf_images.len();
                let mime_type =
                    if image.bits.is_some() || image.path.to_lowercase().ends_with(".png") {
                        "image/png"
                    } else {
                        "image/jpeg"
                    };
                gltf_images.push(json!({
                    "bufferView": image_buffer_view_idx,
                    "mimeType": mime_type,
                    "name": image.name
                }));

                // Add texture
                let texture_idx = gltf_textures.len();
                gltf_textures.push(json!({
                    "sampler": sampler_idx,
                    "source": image_idx,
                    "name": format!("{}_texture", image.name)
                }));

                texture_index_map.insert(texture_key, texture_idx);
            }
        }
    }

    // Second pass: create materials for meshes
    for (mesh_idx, mesh) in meshes.iter().enumerate() {
        // Case 1: Mesh has color_tint - needs unique material
        if let Some(color_tint) = mesh.color_tint {
            let material_idx = gltf_materials.len();
            mesh_material_map.insert(mesh_idx, material_idx);

            if let Some(ref texture_name) = mesh.texture_name {
                let texture_key = texture_name.to_lowercase();
                if let Some(&texture_idx) = texture_index_map.get(&texture_key) {
                    // Material with texture and color tint
                    gltf_materials.push(json!({
                        "name": format!("{}_{}", mesh.name, texture_name),
                        "pbrMetallicRoughness": {
                            "baseColorTexture": {
                                "index": texture_idx
                            },
                            "baseColorFactor": color_tint,
                            "metallicFactor": 0.0,
                            "roughnessFactor": 0.5
                        },
                        "alphaMode": "BLEND",
                        "doubleSided": true
                    }));
                } else {
                    // Texture not found, use color only
                    gltf_materials.push(json!({
                        "name": format!("{}_color", mesh.name),
                        "pbrMetallicRoughness": {
                            "baseColorFactor": color_tint,
                            "metallicFactor": 0.0,
                            "roughnessFactor": 0.5
                        },
                        "alphaMode": "BLEND",
                        "doubleSided": true
                    }));
                }
            } else {
                // No texture, color only (e.g., shadow flashers)
                gltf_materials.push(json!({
                    "name": format!("{}_color", mesh.name),
                    "pbrMetallicRoughness": {
                        "baseColorFactor": color_tint,
                        "metallicFactor": 0.0,
                        "roughnessFactor": 0.5
                    },
                    "alphaMode": "BLEND",
                    "doubleSided": true
                }));
            }
        }
        // Case 2: Mesh has transmission_factor - needs unique material with KHR_materials_transmission
        // This must come before texture-only case because transmission requires unique materials
        else if let Some(transmission) = mesh.transmission_factor {
            // Only create unique material if transmission > 0
            if transmission > 0.0 {
                let material_idx = gltf_materials.len();
                mesh_material_map.insert(mesh_idx, material_idx);
                uses_transmission_extension = true;

                // Get base material properties if available
                let (base_color, metallic, roughness, needs_alpha_blend) = if let Some(ref mat_name) =
                    mesh.material_name
                    && let Some(mat) = materials.get(mat_name)
                {
                    // Check if material needs alpha blending (opacity_active and opacity < 0.999)
                    let needs_blend = mat.opacity_active && mat.base_color[3] < 0.999;
                    (mat.base_color, mat.metallic, mat.roughness, needs_blend)
                } else {
                    ([1.0, 1.0, 1.0, 1.0], 0.0, 0.5, false)
                };

                // Check if mesh also has a texture
                if let Some(ref texture_name) = mesh.texture_name {
                    let texture_key = texture_name.to_lowercase();
                    if let Some(&texture_idx) = texture_index_map.get(&texture_key) {
                        // Material with texture AND transmission
                        // Reduce transmission for textured surfaces since printed decals/stickers
                        // block light - only clear/unprinted areas would transmit light
                        // Use 30% of the original transmission as an approximation
                        let reduced_transmission = transmission * 0.3;

                        let mut material = json!({
                            "name": format!("{}_transmission", mesh.name),
                            "pbrMetallicRoughness": {
                                "baseColorTexture": {
                                    "index": texture_idx
                                },
                                "baseColorFactor": base_color,
                                "metallicFactor": metallic,
                                "roughnessFactor": roughness
                            },
                            "extensions": {
                                "KHR_materials_transmission": {
                                    "transmissionFactor": reduced_transmission
                                }
                            },
                            "doubleSided": true
                        });
                        if needs_alpha_blend {
                            material["alphaMode"] = json!("BLEND");
                        }
                        gltf_materials.push(material);
                    } else {
                        // Texture not found, use color only with transmission
                        let mut material = json!({
                            "name": format!("{}_transmission", mesh.name),
                            "pbrMetallicRoughness": {
                                "baseColorFactor": base_color,
                                "metallicFactor": metallic,
                                "roughnessFactor": roughness
                            },
                            "extensions": {
                                "KHR_materials_transmission": {
                                    "transmissionFactor": transmission
                                }
                            }
                        });
                        if needs_alpha_blend {
                            material["alphaMode"] = json!("BLEND");
                        }
                        gltf_materials.push(material);
                    }
                } else {
                    // No texture, just transmission
                    let mut material = json!({
                        "name": format!("{}_transmission", mesh.name),
                        "pbrMetallicRoughness": {
                            "baseColorFactor": base_color,
                            "metallicFactor": metallic,
                            "roughnessFactor": roughness
                        },
                        "extensions": {
                            "KHR_materials_transmission": {
                                "transmissionFactor": transmission
                            }
                        }
                    });
                    if needs_alpha_blend {
                        material["alphaMode"] = json!("BLEND");
                    }
                    gltf_materials.push(material);
                }
            }
        }
        // Case 3: Mesh has texture but no color_tint and no transmission - can share material
        else if let Some(ref texture_name) = mesh.texture_name {
            let texture_key = texture_name.to_lowercase();

            // Skip if material already exists for this texture
            if texture_material_map.contains_key(&texture_key) {
                continue;
            }

            if let Some(&texture_idx) = texture_index_map.get(&texture_key) {
                let material_idx = gltf_materials.len();
                texture_material_map.insert(texture_key.clone(), material_idx);

                // Check if the image has transparency (is_opaque == Some(false))
                // If is_opaque is None or Some(true), the texture is opaque
                let needs_alpha = image_map
                    .get(&texture_key)
                    .is_some_and(|img| img.is_opaque == Some(false));

                if needs_alpha {
                    // Use MASK mode instead of BLEND to avoid making the entire mesh translucent
                    // MASK mode makes pixels below alphaCutoff fully transparent, above it fully opaque
                    gltf_materials.push(json!({
                        "name": format!("__texture__{}", texture_name),
                        "pbrMetallicRoughness": {
                            "baseColorTexture": {
                                "index": texture_idx
                            },
                            "metallicFactor": 0.0,
                            "roughnessFactor": 0.5
                        },
                        "alphaMode": "MASK",
                        "alphaCutoff": 0.5,
                        "doubleSided": true
                    }));
                } else {
                    gltf_materials.push(json!({
                        "name": format!("__texture__{}", texture_name),
                        "pbrMetallicRoughness": {
                            "baseColorTexture": {
                                "index": texture_idx
                            },
                            "metallicFactor": 0.0,
                            "roughnessFactor": 0.5
                        },
                        // No alphaMode = OPAQUE (default)
                        "doubleSided": true
                    }));
                }
            }
        }
    }

    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut mesh_json = Vec::new();
    let mut accessors = Vec::new();

    // Track layer groups: layer_name -> (layer_node_index, child_node_indices)
    let mut layer_groups: HashMap<String, (usize, Vec<usize>)> = HashMap::new();
    // Track meshes without a layer (will be at root level)
    let mut root_node_indices: Vec<usize> = Vec::new();

    for (mesh_idx, mesh) in meshes.iter().enumerate() {
        let accessor_base = accessors.len();
        let buffer_view_base = buffer_views.len();

        // Write positions (VEC3 float)
        // Transform from VPinball coordinates (left-handed, Z-up) to glTF (right-handed, Y-up):
        //   VPX X → glTF X (keep origin at left)
        //   VPX Y → glTF Z (towards viewer, so player side faces camera)
        //   VPX Z → glTF Y (up)
        // Also scale from VP units to meters
        // Winding order is reversed to change handedness
        let positions_offset = bin_data.len();
        for VertexWrapper { vertex, .. } in &mesh.vertices {
            bin_data
                .write_f32::<LittleEndian>(vpu_to_m(vertex.x))
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(vpu_to_m(vertex.z))
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(vpu_to_m(vertex.y))
                .map_err(WriteError::Io)?;
        }
        let positions_length = bin_data.len() - positions_offset;

        // Write normals (VEC3 float) - same transformation as positions
        let normals_offset = bin_data.len();
        for VertexWrapper { vertex, .. } in &mesh.vertices {
            let nx = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
            let ny = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
            let nz = if vertex.nz.is_nan() { 0.0 } else { vertex.nz };
            bin_data
                .write_f32::<LittleEndian>(nx)
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(nz)
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(ny)
                .map_err(WriteError::Io)?;
        }
        let normals_length = bin_data.len() - normals_offset;

        // Write texcoords (VEC2 float)
        let texcoords_offset = bin_data.len();
        for VertexWrapper { vertex, .. } in &mesh.vertices {
            bin_data
                .write_f32::<LittleEndian>(vertex.tu)
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(vertex.tv)
                .map_err(WriteError::Io)?;
        }
        let texcoords_length = bin_data.len() - texcoords_offset;

        // Write indices (SCALAR uint16 or uint32)
        // Reverse winding order (swap i1 and i2) to convert from left-handed to right-handed
        let indices_offset = bin_data.len();
        let use_u32 = mesh.vertices.len() > 65535;
        for face in &mesh.indices {
            if use_u32 {
                bin_data
                    .write_u32::<LittleEndian>(face.i0 as u32)
                    .map_err(WriteError::Io)?;
                bin_data
                    .write_u32::<LittleEndian>(face.i2 as u32)
                    .map_err(WriteError::Io)?;
                bin_data
                    .write_u32::<LittleEndian>(face.i1 as u32)
                    .map_err(WriteError::Io)?;
            } else {
                bin_data
                    .write_u16::<LittleEndian>(face.i0 as u16)
                    .map_err(WriteError::Io)?;
                bin_data
                    .write_u16::<LittleEndian>(face.i2 as u16)
                    .map_err(WriteError::Io)?;
                bin_data
                    .write_u16::<LittleEndian>(face.i1 as u16)
                    .map_err(WriteError::Io)?;
            }
        }
        let indices_length = bin_data.len() - indices_offset;

        // Calculate bounds in glTF coordinate space (after transformation and scaling)
        // VPX (x, y, z) → glTF (x * scale, gltf_y = z * scale, y * scale)
        let (min_x, max_x, min_y, max_y, min_z, max_z) = mesh.vertices.iter().fold(
            (
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ),
            |(min_x, max_x, min_y, max_y, min_z, max_z), v| {
                // Transform: glTF_x = vpx_x, glTF_y = vpx_z, glTF_z = vpx_y (all scaled)
                let gltf_x = vpu_to_m(v.vertex.x);
                let gltf_y = vpu_to_m(v.vertex.z);
                let gltf_z = vpu_to_m(v.vertex.y);
                (
                    min_x.min(gltf_x),
                    max_x.max(gltf_x),
                    min_y.min(gltf_y),
                    max_y.max(gltf_y),
                    min_z.min(gltf_z),
                    max_z.max(gltf_z),
                )
            },
        );

        // Add buffer views for this mesh
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": positions_offset,
            "byteLength": positions_length,
            "target": GLTF_TARGET_ARRAY_BUFFER
        }));
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": normals_offset,
            "byteLength": normals_length,
            "target": GLTF_TARGET_ARRAY_BUFFER
        }));
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": texcoords_offset,
            "byteLength": texcoords_length,
            "target": GLTF_TARGET_ARRAY_BUFFER
        }));
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": indices_offset,
            "byteLength": indices_length,
            "target": GLTF_TARGET_ELEMENT_ARRAY_BUFFER
        }));

        // Add accessors for this mesh
        accessors.push(json!({
            "bufferView": buffer_view_base,
            "componentType": GLTF_COMPONENT_TYPE_FLOAT,
            "count": mesh.vertices.len(),
            "type": "VEC3",
            "min": [min_x, min_y, min_z],
            "max": [max_x, max_y, max_z]
        }));
        accessors.push(json!({
            "bufferView": buffer_view_base + 1,
            "componentType": GLTF_COMPONENT_TYPE_FLOAT,
            "count": mesh.vertices.len(),
            "type": "VEC3"
        }));
        accessors.push(json!({
            "bufferView": buffer_view_base + 2,
            "componentType": GLTF_COMPONENT_TYPE_FLOAT,
            "count": mesh.vertices.len(),
            "type": "VEC2"
        }));
        accessors.push(json!({
            "bufferView": buffer_view_base + 3,
            "componentType": if use_u32 {
                GLTF_COMPONENT_TYPE_UNSIGNED_INT
            } else {
                GLTF_COMPONENT_TYPE_UNSIGNED_SHORT
            },
            "count": mesh.indices.len() * 3,
            "type": "SCALAR"
        }));

        // Add mesh
        let mut primitive = json!({
            "attributes": {
                "POSITION": accessor_base,
                "NORMAL": accessor_base + 1,
                "TEXCOORD_0": accessor_base + 2
            },
            "indices": accessor_base + 3,
            "mode": GLTF_PRIMITIVE_MODE_TRIANGLES
        });

        // Add material reference if the mesh has a material or texture
        // Check mesh_material_map first (for meshes with color tint - unique materials)
        if let Some(&mat_idx) = mesh_material_map.get(&mesh_idx) {
            primitive["material"] = json!(mat_idx);
        } else if let Some(ref texture_name) = mesh.texture_name {
            // Prioritize texture-based material when mesh has a texture
            // This ensures the texture is applied even if the mesh also has a material_name
            let texture_key = texture_name.to_lowercase();
            if let Some(&mat_idx) = texture_material_map.get(&texture_key) {
                primitive["material"] = json!(mat_idx);
            }
        } else if let Some(ref mat_name) = mesh.material_name
            && let Some(&mat_idx) = material_index_map.get(mat_name)
        {
            // Fall back to VPX material only if there's no texture
            primitive["material"] = json!(mat_idx);
        }

        mesh_json.push(json!({
            "name": mesh.name,
            "primitives": [primitive]
        }));

        // Add node for this mesh
        let node_idx = nodes.len();
        nodes.push(json!({
            "mesh": mesh_idx,
            "name": mesh.name
        }));

        // Organize nodes into layer groups
        if let Some(ref layer_name) = mesh.layer_name {
            if let Some((_, children)) = layer_groups.get_mut(layer_name) {
                // Layer already exists, add this node to its children
                children.push(node_idx);
            } else {
                // Create a new layer group (node will be added later)
                layer_groups.insert(layer_name.clone(), (usize::MAX, vec![node_idx]));
            }
        } else {
            // No layer, add to root level
            root_node_indices.push(node_idx);
        }
    }

    // Don't create layer group nodes yet - wait until after lights are processed
    // so that lights can be added to their layers

    // Pad binary data to 4-byte alignment
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    // Create light nodes for the two default VPinball lights
    // VPinball has two point lights (MAX_LIGHT_SOURCES = 2), both at the same position
    // Default position is (0, 0, 400) in VPX units
    // Convert to glTF coordinates: X stays, Y->Z, Z->Y, all scaled
    let light_height = vpu_to_m(vpx.gamedata.light_height);
    let table_center_x = vpu_to_m((vpx.gamedata.left + vpx.gamedata.right) / 2.0);
    let table_center_z = vpu_to_m((vpx.gamedata.top + vpx.gamedata.bottom) / 2.0);

    // Light emission color (normalized to 0-1)
    let light_color = [
        vpx.gamedata.light0_emission.r as f32 / 255.0,
        vpx.gamedata.light0_emission.g as f32 / 255.0,
        vpx.gamedata.light0_emission.b as f32 / 255.0,
    ];

    // Light intensity - VPinball uses light_emission_scale which is typically very large (e.g., 1000000+)
    // glTF KHR_lights_punctual uses candelas where typical values are much lower
    // Scale down significantly for reasonable Blender rendering
    let light_intensity = (vpx.gamedata.light_emission_scale / 1000000.0).clamp(0.01, 10.0);

    // Light range in meters - cap to reasonable value for glTF
    // VPinball light_range is often very large (e.g., 4000000 VPX units)
    let light_range = vpu_to_m(vpx.gamedata.light_range).min(100.0);

    // Build lights array for KHR_lights_punctual extension
    // Start with the two default VPinball table lights
    let mut gltf_lights = vec![
        json!({
            "name": "TableLight0",
            "type": "point",
            "color": light_color,
            "intensity": light_intensity,
            "range": light_range
        }),
        json!({
            "name": "TableLight1",
            "type": "point",
            "color": light_color,
            "intensity": light_intensity,
            "range": light_range
        }),
    ];

    // // Collect game item lights and add them
    // // Track light info for creating nodes later: (name, x, y, z, layer_name)
    // NOTE: VPinball lights can have a polygon shape defined by drag_points,
    // constraining the light to that area. glTF only supports point/spot/directional
    // lights, so we export as point lights positioned at the center. The polygon
    // shape information is lost in the export. However, we calculate the range
    // based on the furthest drag point from the center to approximate the light's reach.
    //
    // We only export lights whose names start with "GI" (case insensitive) to avoid
    // cluttering the scene with too many lights. GI lights are typically the general
    // illumination lights that provide ambient lighting to the table.
    let mut game_lights: Vec<(String, f32, f32, f32, Option<String>)> = Vec::new();

    for gameitem in &vpx.gameitems {
        if let GameItemEnum::Light(light) = gameitem {
            // Skip backglass lights
            if light.is_backglass {
                continue;
            }

            // Only include lights whose names start with "gi" (case insensitive)
            if !light.name.to_lowercase().starts_with("gi") {
                continue;
            }

            // Get light height (use provided height or default to 0)
            let light_z = light.height.unwrap_or(0.0);

            // Light color (normalized to 0-1)
            let color = [
                light.color.r as f32 / 255.0,
                light.color.g as f32 / 255.0,
                light.color.b as f32 / 255.0,
            ];

            // Light intensity - VPinball intensity values are typically 1-10+
            // Scale down for glTF/Blender where intensity is in candelas
            let intensity = (light.intensity * 0.1).clamp(0.01, 10.0);

            // Calculate light range using the helper function
            let range = calculate_light_range(light);

            gltf_lights.push(json!({
                "name": light.name,
                "type": "point",
                "color": color,
                "intensity": intensity,
                "range": range
            }));

            // Store position info for node creation
            game_lights.push((
                light.name.clone(),
                light.center.x,
                light.center.y,
                light_z,
                get_layer_name(&light.editor_layer_name, light.editor_layer),
            ));
        }
    }

    // Collect all scene root nodes - start with root-level meshes (no layer)
    let mut scene_root_nodes: Vec<usize> = root_node_indices.clone();

    let light_node_0 = json!({
        "name": "TableLight0",
        "translation": [table_center_x, light_height, table_center_z],
        "extensions": {
            "KHR_lights_punctual": {
                "light": 0
            }
        }
    });
    let light_node_1 = json!({
        "name": "TableLight1",
        "translation": [table_center_x, light_height, table_center_z],
        "extensions": {
            "KHR_lights_punctual": {
                "light": 1
            }
        }
    });

    // Add light nodes to the nodes array
    let light_node_0_idx = nodes.len();
    nodes.push(light_node_0);
    scene_root_nodes.push(light_node_0_idx);

    let light_node_1_idx = nodes.len();
    nodes.push(light_node_1);
    scene_root_nodes.push(light_node_1_idx);

    // Add game item light nodes (GI lights only)
    // Light indices start at 2 (after TableLight0 and TableLight1)
    for (i, (name, x, y, z, layer_name)) in game_lights.into_iter().enumerate() {
        let light_idx = i + 2; // Offset by 2 for table lights

        let gltf_x = vpu_to_m(x);
        let gltf_y = vpu_to_m(z); // VPX Z -> glTF Y
        let gltf_z = vpu_to_m(y); // VPX Y -> glTF Z

        let light_node = json!({
            "name": name,
            "translation": [gltf_x, gltf_y, gltf_z],
            "extensions": {
                "KHR_lights_punctual": {
                    "light": light_idx
                }
            }
        });

        let node_idx = nodes.len();
        nodes.push(light_node);

        // Add to layer group if it has a layer, otherwise to root
        if let Some(ref layer) = layer_name {
            if let Some((_, children)) = layer_groups.get_mut(layer) {
                children.push(node_idx);
            } else {
                // Layer doesn't exist yet, create it
                layer_groups.insert(layer.clone(), (usize::MAX, vec![node_idx]));
            }
        } else {
            scene_root_nodes.push(node_idx);
        }
    }

    // Create any new layer group nodes that were added by lights
    for (layer_name, (layer_node_idx, children)) in layer_groups.iter_mut() {
        // Create the layer group node (all layer groups now need to be created here)
        *layer_node_idx = nodes.len();
        nodes.push(json!({
            "name": layer_name,
            "children": children
        }));
        scene_root_nodes.push(*layer_node_idx);
    }

    // Create cameras for all three view modes (Desktop, Fullscreen, FSS)
    // Each provides a different view of the table based on VPinball's view settings
    let cameras = GltfCamera::all_from_vpx(vpx);

    // Add all cameras to glTF
    let gltf_cameras: Vec<_> = cameras.iter().map(|c| c.to_gltf_camera_json()).collect();
    for (i, camera) in cameras.iter().enumerate() {
        let camera_node_idx = nodes.len();
        nodes.push(camera.to_gltf_node_json(i));
        scene_root_nodes.push(camera_node_idx);
    }

    // Build list of extensions used
    let mut extensions_used = vec!["KHR_lights_punctual"];
    if uses_transmission_extension {
        extensions_used.push("KHR_materials_transmission");
    }

    let mut gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "vpin"
        },
        "extensionsUsed": extensions_used,
        "extensions": {
            "KHR_lights_punctual": {
                "lights": gltf_lights
            }
        },
        "scene": 0,
        "scenes": [{
            "nodes": scene_root_nodes
        }],
        "nodes": nodes,
        "meshes": mesh_json,
        "cameras": gltf_cameras,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{
            "byteLength": bin_data.len()
        }]
    });

    // Add materials array if there are any materials
    if !gltf_materials.is_empty() {
        gltf_json["materials"] = json!(gltf_materials);
    }

    // Add textures, images, and samplers if we have any
    if !gltf_textures.is_empty() {
        gltf_json["textures"] = json!(gltf_textures);
    }
    if !gltf_images.is_empty() {
        gltf_json["images"] = json!(gltf_images);
    }
    if !gltf_samplers.is_empty() {
        gltf_json["samplers"] = json!(gltf_samplers);
    }

    Ok((gltf_json, bin_data))
}

/// Write GLB file
fn write_glb<W: io::Write>(
    json: &serde_json::Value,
    bin_data: &[u8],
    writer: &mut W,
) -> Result<(), WriteError> {
    let json_string = serde_json::to_string(json).map_err(WriteError::Json)?;
    let json_bytes = json_string.as_bytes();

    // Pad JSON to 4-byte alignment
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let json_padded_length = json_bytes.len() + json_padding;

    // Write GLB header
    writer.write_all(GLTF_MAGIC).map_err(WriteError::Io)?;
    writer
        .write_u32::<LittleEndian>(GLTF_VERSION)
        .map_err(WriteError::Io)?;
    let total_length = GLB_HEADER_BYTES
        + GLB_CHUNK_HEADER_BYTES
        + json_padded_length as u32
        + GLB_CHUNK_HEADER_BYTES
        + bin_data.len() as u32;
    writer
        .write_u32::<LittleEndian>(total_length)
        .map_err(WriteError::Io)?;

    // Write JSON chunk
    writer
        .write_u32::<LittleEndian>(json_padded_length as u32)
        .map_err(WriteError::Io)?;
    writer
        .write_all(GLB_JSON_CHUNK_TYPE)
        .map_err(WriteError::Io)?;
    writer.write_all(json_bytes).map_err(WriteError::Io)?;
    for _ in 0..json_padding {
        writer.write_all(b" ").map_err(WriteError::Io)?;
    }

    // Write BIN chunk
    writer
        .write_u32::<LittleEndian>(bin_data.len() as u32)
        .map_err(WriteError::Io)?;
    writer
        .write_all(GLB_BIN_CHUNK_TYPE)
        .map_err(WriteError::Io)?;
    writer.write_all(bin_data).map_err(WriteError::Io)?;

    Ok(())
}

/// Export the entire VPX table as a GLB file
///
/// This creates a single GLB file containing all meshes from the table:
/// - Primitives (with their embedded mesh data)
/// - Walls (generated from drag points)
/// - Ramps (generated from drag points)
/// - Rubbers (generated from drag points)
/// - Flashers (generated from drag points)
/// - Playfield (generated from table bounds if no explicit playfield mesh is defined)
/// - Materials
///
/// Ignored for now
/// - Lights (from game items and default table lights)
///
/// # Arguments
/// * `vpx` - The VPX table to export
/// * `path` - The output path for the GLB file
/// * `fs` - The filesystem to write to
///
/// # Example
/// ```no_run
/// use vpin::vpx;
/// use vpin::vpx::expanded::export_glb;
/// use vpin::filesystem::RealFileSystem;
/// use std::path::Path;
///
/// let vpx = vpx::read(Path::new("table.vpx")).unwrap();
/// export_glb(&vpx, Path::new("table.glb"), &RealFileSystem).unwrap();
/// ```
pub fn export_glb(vpx: &VPX, path: &Path, fs: &dyn FileSystem) -> Result<(), WriteError> {
    let meshes = collect_meshes(vpx);

    if meshes.is_empty() {
        return Err(WriteError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No meshes found in table",
        )));
    }

    // Collect materials from gamedata
    let materials = collect_materials(vpx);

    // Find playfield image
    let playfield_image = find_playfield_image(vpx);

    // Use the playfield material name from gamedata if set
    let playfield_material_name = if vpx.gamedata.playfield_material.is_empty() {
        PLAYFIELD_MATERIAL_NAME
    } else {
        &vpx.gamedata.playfield_material
    };

    let (json, bin_data) = build_combined_gltf_payload(
        vpx,
        &meshes,
        &materials,
        &vpx.images,
        playfield_image,
        playfield_material_name,
    )?;

    let mut buffer = Vec::new();
    write_glb(&json, &bin_data, &mut buffer)?;

    fs.write_file(path, &buffer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_meshes_empty_vpx_has_implicit_playfield() {
        let vpx = VPX::default();
        let meshes = collect_meshes(&vpx);
        // Even an empty VPX gets an implicit playfield mesh
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].name, "playfield_mesh");
    }

    #[test]
    fn test_export_glb_empty_vpx_succeeds_with_playfield() {
        let vpx = VPX::default();
        let fs = crate::filesystem::MemoryFileSystem::default();
        // Should succeed because we generate an implicit playfield
        let result = export_glb(&vpx, Path::new("test.glb"), &fs);
        assert!(result.is_ok());
    }
}
