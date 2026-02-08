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
use super::flashers::build_flasher_mesh;
use super::ramps::build_ramp_mesh;
use super::rubbers::build_rubber_mesh;
use super::walls::build_wall_mesh;
use crate::filesystem::FileSystem;
use crate::vpx::VPX;
use crate::vpx::gameitem::GameItemEnum;
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
use byteorder::{LittleEndian, WriteBytesExt};
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::path::Path;

/// Conversion factor from VP units to meters
/// From VPinball def.h: 50 VPU = 1.0625 inches, 1 inch = 25.4mm
/// So 1 VPU = (25.4 * 1.0625) / 50 mm = 0.539750 mm = 0.000539750 meters
const VP_UNITS_TO_METERS: f32 = (25.4 * 1.0625) / (50.0 * 1000.0);

/// Special material name for the playfield
const PLAYFIELD_MATERIAL_NAME: &str = "__playfield__";

/// A named mesh ready for GLTF export
struct NamedMesh {
    name: String,
    vertices: Vec<VertexWrapper>,
    indices: Vec<VpxFace>,
    material_name: Option<String>,
}

/// Apply primitive transformation (scale, rotation, translation) to vertices
/// All transformations are done in VPX coordinate space.
/// The coordinate conversion to glTF happens when writing the GLB.
///
/// ## Rotation Order (Important!)
///
/// VPinball builds the transformation matrix as (from primitive.cpp):
/// ```text
/// RTmatrix = Translate(tra) * RotZ * RotY * RotX * ObjRotZ * ObjRotY * ObjRotX
/// fullMatrix = Scale * RTmatrix * Translate(pos)
/// ```
///
/// When this matrix is applied to a vertex via `fullMatrix * v`, the rightmost
/// operation is applied first. So the actual order applied to vertices is:
/// 1. Translate(pos)
/// 2. ObjRotX, ObjRotY, ObjRotZ (in that order)
/// 3. RotX, RotY, RotZ (in that order)
/// 4. Translate(tra)
/// 5. Scale
///
/// However, when applying rotations sequentially (not using matrix multiplication),
/// we must apply them in **reverse order** (Z, Y, X) to achieve the same result.
/// This is because matrix multiplication `A * B * v` means B is applied first,
/// but sequential application must reverse this.
///
/// ## Transformation steps applied to each vertex:
/// 1. Scale by size
/// 2. Apply ObjRotZ, ObjRotY, ObjRotX (reverse order for sequential application)
/// 3. Apply RotZ, RotY, RotX (reverse order for sequential application)
/// 4. Translate by tra
/// 5. Translate by position
fn transform_primitive_vertices(
    vertices: Vec<VertexWrapper>,
    primitive: &crate::vpx::gameitem::primitive::Primitive,
) -> Vec<VertexWrapper> {
    use std::f32::consts::PI;

    let pos = &primitive.position;
    let size = &primitive.size;
    let rot = &primitive.rot_and_tra;

    // rot_and_tra indices:
    // 0-2: RotX, RotY, RotZ (degrees)
    // 3-5: TraX, TraY, TraZ
    // 6-8: ObjRotX, ObjRotY, ObjRotZ (degrees)

    let deg_to_rad = |deg: f32| deg * PI / 180.0;

    vertices
        .into_iter()
        .map(|mut vw| {
            let mut x = vw.vertex.x;
            let mut y = vw.vertex.y;
            let mut z = vw.vertex.z;

            // 1. Apply scale first
            x *= size.x;
            y *= size.y;
            z *= size.z;

            // 2. Apply object rotation in reverse order: Z, Y, X
            // ObjRotZ
            let (sin_z, cos_z) = deg_to_rad(rot[8]).sin_cos();
            let (x1, y1) = (x * cos_z - y * sin_z, x * sin_z + y * cos_z);
            x = x1;
            y = y1;
            // ObjRotY
            let (sin_y, cos_y) = deg_to_rad(rot[7]).sin_cos();
            let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
            x = x1;
            z = z1;
            // ObjRotX
            let (sin_x, cos_x) = deg_to_rad(rot[6]).sin_cos();
            let (y1, z1) = (y * cos_x - z * sin_x, y * sin_x + z * cos_x);
            y = y1;
            z = z1;

            // 3. Apply rotation in reverse order: Z, Y, X
            // RotZ
            let (sin_z, cos_z) = deg_to_rad(rot[2]).sin_cos();
            let (x1, y1) = (x * cos_z - y * sin_z, x * sin_z + y * cos_z);
            x = x1;
            y = y1;
            // RotY
            let (sin_y, cos_y) = deg_to_rad(rot[1]).sin_cos();
            let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
            x = x1;
            z = z1;
            // RotX
            let (sin_x, cos_x) = deg_to_rad(rot[0]).sin_cos();
            let (y1, z1) = (y * cos_x - z * sin_x, y * sin_x + z * cos_x);
            y = y1;
            z = z1;

            // 4. Apply translation (TraX, TraY, TraZ) - indices 3, 4, 5
            x += rot[3];
            y += rot[4];
            z += rot[5];

            // 5. Apply position translation
            x += pos.x;
            y += pos.y;
            z += pos.z;

            vw.vertex.x = x;
            vw.vertex.y = y;
            vw.vertex.z = z;

            // Transform normals (rotation only, no translation/scale)
            let mut nx = vw.vertex.nx;
            let mut ny = vw.vertex.ny;
            let mut nz = vw.vertex.nz;

            if !nx.is_nan() && !ny.is_nan() && !nz.is_nan() {
                // Apply object rotation in reverse order: Z, Y, X
                let (sin_z, cos_z) = deg_to_rad(rot[8]).sin_cos();
                let (nx1, ny1) = (nx * cos_z - ny * sin_z, nx * sin_z + ny * cos_z);
                nx = nx1;
                ny = ny1;
                let (sin_y, cos_y) = deg_to_rad(rot[7]).sin_cos();
                let (nx1, nz1) = (nx * cos_y + nz * sin_y, -nx * sin_y + nz * cos_y);
                nx = nx1;
                nz = nz1;
                let (sin_x, cos_x) = deg_to_rad(rot[6]).sin_cos();
                let (ny1, nz1) = (ny * cos_x - nz * sin_x, ny * sin_x + nz * cos_x);
                ny = ny1;
                nz = nz1;

                // Apply rotation in reverse order: Z, Y, X
                let (sin_z, cos_z) = deg_to_rad(rot[2]).sin_cos();
                let (nx1, ny1) = (nx * cos_z - ny * sin_z, nx * sin_z + ny * cos_z);
                nx = nx1;
                ny = ny1;
                let (sin_y, cos_y) = deg_to_rad(rot[1]).sin_cos();
                let (nx1, nz1) = (nx * cos_y + nz * sin_y, -nx * sin_y + nz * cos_y);
                nx = nx1;
                nz = nz1;
                let (sin_x, cos_x) = deg_to_rad(rot[0]).sin_cos();
                let (ny1, nz1) = (ny * cos_x - nz * sin_x, ny * sin_x + nz * cos_x);
                ny = ny1;
                nz = nz1;

                // Normalize
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len > 0.0 {
                    vw.vertex.nx = nx / len;
                    vw.vertex.ny = ny / len;
                    vw.vertex.nz = nz / len;
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
                    mat.opacity,
                ],
                metallic: if mat.type_ == MaterialType::Metal {
                    1.0
                } else {
                    0.0
                },
                roughness: mat.roughness,
            };
            materials.insert(mat.name.clone(), gltf_mat);
        }
    } else {
        // Fall back to old format
        for mat in &vpx.gamedata.materials_old {
            let gltf_mat = GltfMaterial {
                name: mat.name.clone(),
                base_color: [
                    mat.base_color.r as f32 / 255.0,
                    mat.base_color.g as f32 / 255.0,
                    mat.base_color.b as f32 / 255.0,
                    mat.opacity,
                ],
                metallic: if mat.is_metal { 1.0 } else { 0.0 },
                roughness: mat.roughness,
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

                    // Determine material name:
                    // - If it's the playfield primitive, use the playfield material (which has the texture)
                    // - Otherwise, use the primitive's material if set
                    let material_name = if is_playfield {
                        Some(playfield_material_name.clone())
                    } else if !primitive.material.is_empty() {
                        Some(primitive.material.clone())
                    } else {
                        None
                    };

                    meshes.push(NamedMesh {
                        name: primitive.name.clone(),
                        vertices: transformed,
                        indices: read_mesh.indices,
                        material_name,
                    });
                }
            }
            GameItemEnum::Wall(wall) => {
                if !wall.is_top_bottom_visible && !wall.is_side_visible {
                    continue; // Skip fully invisible walls
                }
                if let Some((vertices, indices)) = build_wall_mesh(wall) {
                    let material_name = if wall.top_material.is_empty() {
                        None
                    } else {
                        Some(wall.top_material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: wall.name.clone(),
                        vertices,
                        indices,
                        material_name,
                    });
                }
            }
            GameItemEnum::Ramp(ramp) => {
                if !ramp.is_visible {
                    continue; // Skip invisible ramps
                }
                if let Some((vertices, indices)) = build_ramp_mesh(ramp) {
                    let material_name = if ramp.material.is_empty() {
                        None
                    } else {
                        Some(ramp.material.clone())
                    };
                    meshes.push(NamedMesh {
                        name: ramp.name.clone(),
                        vertices,
                        indices,
                        material_name,
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
                    });
                }
            }
            GameItemEnum::Flasher(flasher) => {
                if !flasher.is_visible {
                    continue; // Skip invisible flashers
                }
                if let Some((vertices, indices)) = build_flasher_mesh(flasher) {
                    // Flashers don't have a material field
                    meshes.push(NamedMesh {
                        name: flasher.name.clone(),
                        vertices,
                        indices,
                        material_name: None,
                    });
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
    meshes: &[NamedMesh],
    materials: &HashMap<String, GltfMaterial>,
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
        }));
    }

    for (name, mat) in materials {
        // Skip if we already added a playfield material with texture
        if material_index_map.contains_key(name) {
            continue;
        }
        material_index_map.insert(name.clone(), gltf_materials.len());
        gltf_materials.push(json!({
            "name": mat.name,
            "pbrMetallicRoughness": {
                "baseColorFactor": mat.base_color,
                "metallicFactor": mat.metallic,
                "roughnessFactor": mat.roughness
            }
        }));
    }
    let mut nodes = Vec::new();
    let mut mesh_json = Vec::new();
    let mut accessors = Vec::new();
    let mut node_indices: Vec<usize> = Vec::new();

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
                .write_f32::<LittleEndian>(vertex.x * VP_UNITS_TO_METERS)
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(vertex.z * VP_UNITS_TO_METERS)
                .map_err(WriteError::Io)?;
            bin_data
                .write_f32::<LittleEndian>(vertex.y * VP_UNITS_TO_METERS)
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
        // VPX (x, y, z) → glTF (x * scale, z * scale, y * scale)
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
                let gltf_x = v.vertex.x * VP_UNITS_TO_METERS;
                let gltf_y = v.vertex.z * VP_UNITS_TO_METERS;
                let gltf_z = v.vertex.y * VP_UNITS_TO_METERS;
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

        // Add material reference if the mesh has a material
        if let Some(ref mat_name) = mesh.material_name
            && let Some(&mat_idx) = material_index_map.get(mat_name)
        {
            primitive["material"] = json!(mat_idx);
        }

        mesh_json.push(json!({
            "name": mesh.name,
            "primitives": [primitive]
        }));

        // Add node
        nodes.push(json!({
            "mesh": mesh_idx,
            "name": mesh.name
        }));
        node_indices.push(mesh_idx);
    }

    // Pad binary data to 4-byte alignment
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let mut gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "vpin"
        },
        "scene": 0,
        "scenes": [{
            "nodes": node_indices
        }],
        "nodes": nodes,
        "meshes": mesh_json,
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
        &meshes,
        &materials,
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
