//! GLTF (GLB) file reader and writer for primitive mesh data
//!
//! This module provides functions to read and write primitive mesh data in the
//! binary GLTF format (GLB). This format is more efficient than OBJ for large
//! meshes due to its binary representation.
//!
//! The VPX-specific normal bytes (used for NaN handling) are stored in the
//! mesh primitive extras as a base64-encoded string per vertex.

// TODO switch to using gltf crate for reading / writing?

use crate::filesystem::FileSystem;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::obj::VpxFace;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde_json::json;
use std::error::Error;
use std::io::{self, Read};
use std::path::Path;
use tracing::{info_span, instrument};
// We have some issues where the data in the vpx file contains NaN values for normals.
// We store the vpx normals data as extras in the gltf mesh primitive.

pub(crate) const GLTF_MAGIC: &[u8; 4] = b"glTF";
pub(crate) const GLTF_VERSION: u32 = 2;
pub(crate) const GLB_HEADER_BYTES: u32 = 12;
pub(crate) const GLB_CHUNK_HEADER_BYTES: u32 = 8;
pub(crate) const GLB_JSON_CHUNK_TYPE: &[u8; 4] = b"JSON";
pub(crate) const GLB_BIN_CHUNK_TYPE: &[u8; 4] = b"BIN\0";
pub(crate) const GLTF_PRIMITIVE_MODE_TRIANGLES: u32 = 4;
pub(crate) const GLTF_COMPONENT_TYPE_FLOAT: u32 = 5126;
pub(crate) const GLTF_COMPONENT_TYPE_UNSIGNED_SHORT: u32 = 5123;
pub(crate) const GLTF_COMPONENT_TYPE_UNSIGNED_INT: u32 = 5125;
pub(crate) const GLTF_TARGET_ARRAY_BUFFER: u32 = 34962;
pub(crate) const GLTF_TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

// Sampler filter modes (from OpenGL ES 2.0)
pub(crate) const GLTF_FILTER_LINEAR: u32 = 9729;
pub(crate) const GLTF_FILTER_LINEAR_MIPMAP_LINEAR: u32 = 9987;

// Sampler wrap modes (from OpenGL ES 2.0)
pub(crate) const GLTF_WRAP_REPEAT: u32 = 10497;

#[allow(dead_code)]
type VpxNormalBytes = [u8; 12];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GltfContainer {
    Glb,
    Gltf,
}

struct GltfPayload {
    json: serde_json::Value,
    bin_data: Vec<u8>,
}

/// Writes a GLTF/GLB file from the vertices and indices as they are stored in the
/// m3cx and m3ci fields of the primitive.
///
/// The z axis is inverted compared to the vpx file values.
#[instrument(skip(vertices, indices, fs, gltf_file_path), fields(path = ?gltf_file_path, vertex_count = vertices.len(), index_count = indices.len(), container = ?container))]
pub(crate) fn write_gltf(
    name: &str,
    vertices: &[VertexWrapper],
    indices: &[VpxFace],
    gltf_file_path: &Path,
    container: GltfContainer,
    fs: &dyn FileSystem,
) -> Result<(), Box<dyn Error>> {
    match container {
        GltfContainer::Glb => {
            let payload = build_gltf_payload(name, vertices, indices, None)?;
            let mut buffer = Vec::new();
            write_glb_payload(&payload, &mut buffer)?;

            let _span = info_span!("fs_write", bytes = buffer.len()).entered();
            fs.write_file(gltf_file_path, &buffer)?;
        }
        GltfContainer::Gltf => {
            let bin_file_name = gltf_file_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| format!("{stem}.bin"))
                .unwrap_or_else(|| "buffer.bin".to_string());
            let bin_path = gltf_file_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(&bin_file_name);

            let payload = build_gltf_payload(name, vertices, indices, Some(&bin_file_name))?;
            let json_string = serde_json::to_string(&payload.json)?;

            let _span = info_span!("fs_write", bytes = json_string.len()).entered();
            fs.write_file(gltf_file_path, json_string.as_bytes())?;
            drop(_span);

            let _span = info_span!("fs_write", bytes = payload.bin_data.len()).entered();
            fs.write_file(&bin_path, &payload.bin_data)?;
        }
    }

    Ok(())
}

fn build_gltf_payload(
    name: &str,
    vertices: &[VertexWrapper],
    indices: &[VpxFace],
    buffer_uri: Option<&str>,
) -> Result<GltfPayload, Box<dyn Error>> {
    // Build binary buffer with all vertex data
    let mut bin_data = Vec::new();

    // Write positions (VEC3 float)
    let positions_offset = 0;
    for VertexWrapper { vertex, .. } in vertices {
        bin_data.write_f32::<LittleEndian>(vertex.x)?;
        bin_data.write_f32::<LittleEndian>(vertex.y)?;
        bin_data.write_f32::<LittleEndian>(vertex.z)?;
    }
    let positions_length = bin_data.len();

    // Write normals (VEC3 float)
    let normals_offset = bin_data.len();
    let mut vpx_normals = Vec::new(); // Store VPX normal bytes for extras
    for VertexWrapper {
        vpx_encoded_vertex,
        vertex,
    } in vertices
    {
        let nx = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
        let ny = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
        let nz = if vertex.nz.is_nan() { 0.0 } else { vertex.nz };

        bin_data.write_f32::<LittleEndian>(nx)?;
        bin_data.write_f32::<LittleEndian>(ny)?;
        bin_data.write_f32::<LittleEndian>(nz)?;

        // Always store VPX normal bytes to preserve exact binary representation
        // This is critical for bit-perfect round-trips
        let mut normal_bytes = [0u8; 12];
        normal_bytes.copy_from_slice(&vpx_encoded_vertex[12..24]);
        vpx_normals.push(hex::encode(normal_bytes));
    }
    let normals_length = bin_data.len() - normals_offset;

    // Write texcoords (VEC2 float)
    let texcoords_offset = bin_data.len();
    for VertexWrapper { vertex, .. } in vertices {
        bin_data.write_f32::<LittleEndian>(vertex.tu)?;
        bin_data.write_f32::<LittleEndian>(vertex.tv)?;
    }
    let texcoords_length = bin_data.len() - texcoords_offset;

    // Write indices (SCALAR uint16 or uint32)
    let indices_offset = bin_data.len();
    let use_u32 = vertices.len() > 65535;
    for face in indices {
        if use_u32 {
            bin_data.write_u32::<LittleEndian>(face.i0 as u32)?;
            bin_data.write_u32::<LittleEndian>(face.i1 as u32)?;
            bin_data.write_u32::<LittleEndian>(face.i2 as u32)?;
        } else {
            bin_data.write_u16::<LittleEndian>(face.i0 as u16)?;
            bin_data.write_u16::<LittleEndian>(face.i1 as u16)?;
            bin_data.write_u16::<LittleEndian>(face.i2 as u16)?;
        }
    }
    let indices_length = bin_data.len() - indices_offset;

    // Pad binary data to 4-byte alignment
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let buffers = if let Some(uri) = buffer_uri {
        json!([{
            "byteLength": bin_data.len(),
            "uri": uri,
        }])
    } else {
        json!([{
            "byteLength": bin_data.len(),
        }])
    };

    // Create GLTF JSON structure
    let (min_x, max_x, min_y, max_y, min_z, max_z) = vertices.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y, min_z, max_z), v| {
            (
                min_x.min(v.vertex.x),
                max_x.max(v.vertex.x),
                min_y.min(v.vertex.y),
                max_y.max(v.vertex.y),
                min_z.min(v.vertex.z),
                max_z.max(v.vertex.z),
            )
        },
    );

    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "vpin",
        },
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": name}],
        "meshes": [{
            "name": name,
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "TEXCOORD_0": 2,
                },
                "indices": 3,
                "mode": GLTF_PRIMITIVE_MODE_TRIANGLES,
                "extras": {
                    "vpx_normals": vpx_normals,
                }
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": GLTF_COMPONENT_TYPE_FLOAT,
                "count": vertices.len(),
                "type": "VEC3",
                "byteOffset": 0,
                "min": [min_x, min_y, min_z],
                "max": [max_x, max_y, max_z],
            },
            {
                "bufferView": 1,
                "componentType": GLTF_COMPONENT_TYPE_FLOAT,
                "count": vertices.len(),
                "type": "VEC3",
                "byteOffset": 0,
            },
            {
                "bufferView": 2,
                "componentType": GLTF_COMPONENT_TYPE_FLOAT,
                "count": vertices.len(),
                "type": "VEC2",
                "byteOffset": 0,
            },
            {
                "bufferView": 3,
                "componentType": if use_u32 {
                    GLTF_COMPONENT_TYPE_UNSIGNED_INT
                } else {
                    GLTF_COMPONENT_TYPE_UNSIGNED_SHORT
                },
                "count": indices.len() * 3,
                "type": "SCALAR",
                "byteOffset": 0,
            },
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": positions_offset, "byteLength": positions_length, "target": GLTF_TARGET_ARRAY_BUFFER},
            {"buffer": 0, "byteOffset": normals_offset, "byteLength": normals_length, "target": GLTF_TARGET_ARRAY_BUFFER},
            {"buffer": 0, "byteOffset": texcoords_offset, "byteLength": texcoords_length, "target": GLTF_TARGET_ARRAY_BUFFER},
            {"buffer": 0, "byteOffset": indices_offset, "byteLength": indices_length, "target": GLTF_TARGET_ELEMENT_ARRAY_BUFFER},
        ],
        "buffers": buffers,
    });

    Ok(GltfPayload {
        json: gltf_json,
        bin_data,
    })
}

fn write_glb_payload<W: io::Write>(
    payload: &GltfPayload,
    writer: &mut W,
) -> Result<(), Box<dyn Error>> {
    let json_string = serde_json::to_string(&payload.json)?;
    let json_bytes = json_string.as_bytes();

    // Pad JSON to 4-byte alignment
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let json_padded_length = json_bytes.len() + json_padding;

    // Write GLB header
    writer.write_all(GLTF_MAGIC)?; // magic
    writer.write_u32::<LittleEndian>(GLTF_VERSION)?; // version
    let total_length = GLB_HEADER_BYTES
        + GLB_CHUNK_HEADER_BYTES
        + json_padded_length as u32
        + GLB_CHUNK_HEADER_BYTES
        + payload.bin_data.len() as u32;
    writer.write_u32::<LittleEndian>(total_length)?; // length

    // Write JSON chunk
    writer.write_u32::<LittleEndian>(json_padded_length as u32)?; // chunk length
    writer.write_all(GLB_JSON_CHUNK_TYPE)?; // chunk type
    writer.write_all(json_bytes)?;
    for _ in 0..json_padding {
        writer.write_all(b" ")?; // space padding
    }

    // Write BIN chunk
    writer.write_u32::<LittleEndian>(payload.bin_data.len() as u32)?; // chunk length
    writer.write_all(GLB_BIN_CHUNK_TYPE)?; // chunk type
    writer.write_all(&payload.bin_data)?;

    Ok(())
}

#[instrument(skip(fs))]
pub(crate) fn read_gltf(
    gltf_path: &Path,
    container: GltfContainer,
    fs: &dyn FileSystem,
) -> io::Result<(Vec<VertexWrapper>, Vec<VpxFace>)> {
    let (_name, vertices, indices) = match container {
        GltfContainer::Glb => {
            let _span = info_span!("fs_read").entered();
            let glb_data = fs.read_file(gltf_path)?;
            drop(_span);

            let mut cursor = io::Cursor::new(&glb_data);
            read_glb_from_reader(&mut cursor)?
        }
        GltfContainer::Gltf => {
            let _span = info_span!("fs_read").entered();
            let gltf_data = fs.read_file(gltf_path)?;
            drop(_span);

            let gltf_json: serde_json::Value = serde_json::from_slice(&gltf_data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid GLTF JSON: {}", e),
                )
            })?;

            let buffer_uri = gltf_json["buffers"][0]["uri"]
                .as_str()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing buffer uri"))?;
            if buffer_uri.starts_with("data:") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Embedded buffer URIs are not supported",
                ));
            }

            let bin_path = gltf_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(buffer_uri);
            let _span = info_span!("fs_read").entered();
            let bin_data = fs.read_file(&bin_path)?;
            drop(_span);

            parse_gltf_payload(&gltf_json, &bin_data)?
        }
    };

    Ok((vertices, indices))
}

/// Reads a GLB file from a reader and returns the name, vertices and indices.
///
/// This function parses the GLB binary format and reconstructs the vertex data
/// including the VPX-specific normal bytes stored in the extras.
///
/// Returns: `(name, vertices, indices)`
pub(crate) fn read_glb_from_reader<R: Read>(
    reader: &mut R,
) -> io::Result<(String, Vec<VertexWrapper>, Vec<VpxFace>)> {
    let payload = read_glb_payload_from_reader(reader)?;
    parse_gltf_payload(&payload.json, &payload.bin_data)
}

fn read_glb_payload_from_reader<R: Read>(reader: &mut R) -> io::Result<GltfPayload> {
    use byteorder::{LittleEndian, ReadBytesExt};

    // Read all GLB data into memory for random access
    let mut glb_data = Vec::new();
    reader.read_to_end(&mut glb_data)?;

    let mut cursor = io::Cursor::new(&glb_data);

    // Read GLB header
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != GLTF_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid GLB magic",
        ));
    }

    let version = cursor.read_u32::<LittleEndian>()?;
    if version != GLTF_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported GLTF version: {}", version),
        ));
    }

    let _total_length = cursor.read_u32::<LittleEndian>()?;

    // Read JSON chunk
    let json_length = cursor.read_u32::<LittleEndian>()? as usize;
    let mut chunk_type = [0u8; 4];
    cursor.read_exact(&mut chunk_type)?;
    if &chunk_type != GLB_JSON_CHUNK_TYPE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected JSON chunk",
        ));
    }

    let json_start = cursor.position() as usize;
    let json_bytes = &glb_data[json_start..json_start + json_length];
    let gltf_json: serde_json::Value = serde_json::from_slice(json_bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid GLTF JSON: {}", e),
        )
    })?;

    cursor.set_position((json_start + json_length) as u64);

    // Read BIN chunk
    let bin_length = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.read_exact(&mut chunk_type)?;
    if &chunk_type != GLB_BIN_CHUNK_TYPE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected BIN chunk",
        ));
    }

    let bin_start = cursor.position() as usize;
    let bin_data = glb_data[bin_start..bin_start + bin_length].to_vec();

    Ok(GltfPayload {
        json: gltf_json,
        bin_data,
    })
}

fn parse_gltf_payload(
    gltf_json: &serde_json::Value,
    bin_data: &[u8],
) -> io::Result<(String, Vec<VertexWrapper>, Vec<VpxFace>)> {
    use crate::vpx::model::Vertex3dNoTex2;
    use byteorder::{LittleEndian, ReadBytesExt};

    // Parse GLTF structure
    let accessors = gltf_json["accessors"]
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing accessors"))?;
    let buffer_views = gltf_json["bufferViews"]
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing bufferViews"))?;

    // Extract mesh name
    let name = gltf_json["meshes"][0]["name"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Get VPX normals from extras
    let vpx_normals = gltf_json["meshes"][0]["primitives"][0]["extras"]["vpx_normals"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect::<Vec<String>>()
        });

    // Read positions (accessor 0)
    let pos_accessor = &accessors[0];
    let pos_view_idx = pos_accessor["bufferView"].as_u64().unwrap() as usize;
    let pos_view = &buffer_views[pos_view_idx];
    let pos_offset = pos_view["byteOffset"].as_u64().unwrap() as usize;
    let pos_count = pos_accessor["count"].as_u64().unwrap() as usize;

    // Read normals (accessor 1)
    let norm_accessor = &accessors[1];
    let norm_view_idx = norm_accessor["bufferView"].as_u64().unwrap() as usize;
    let norm_view = &buffer_views[norm_view_idx];
    let norm_offset = norm_view["byteOffset"].as_u64().unwrap() as usize;

    // Read texcoords (accessor 2)
    let tex_accessor = &accessors[2];
    let tex_view_idx = tex_accessor["bufferView"].as_u64().unwrap() as usize;
    let tex_view = &buffer_views[tex_view_idx];
    let tex_offset = tex_view["byteOffset"].as_u64().unwrap() as usize;

    // Read indices (accessor 3)
    let idx_accessor = &accessors[3];
    let idx_view_idx = idx_accessor["bufferView"].as_u64().unwrap() as usize;
    let idx_view = &buffer_views[idx_view_idx];
    let idx_offset = idx_view["byteOffset"].as_u64().unwrap() as usize;
    let idx_count = idx_accessor["count"].as_u64().unwrap() as usize;
    let idx_component_type = idx_accessor["componentType"].as_u64().unwrap();
    let use_u32 = idx_component_type == GLTF_COMPONENT_TYPE_UNSIGNED_INT as u64; // UNSIGNED_INT

    // Build vertex data in the same format as write_glb accepts
    let mut vertices = Vec::with_capacity(pos_count);

    for i in 0..pos_count {
        let mut pos_cursor = io::Cursor::new(&bin_data[pos_offset + i * 12..]);
        let x = pos_cursor.read_f32::<LittleEndian>()?;
        let y = pos_cursor.read_f32::<LittleEndian>()?;
        let z = pos_cursor.read_f32::<LittleEndian>()?;

        let mut norm_cursor = io::Cursor::new(&bin_data[norm_offset + i * 12..]);
        let nx = norm_cursor.read_f32::<LittleEndian>()?;
        let ny = norm_cursor.read_f32::<LittleEndian>()?;
        let nz = norm_cursor.read_f32::<LittleEndian>()?;

        let mut tex_cursor = io::Cursor::new(&bin_data[tex_offset + i * 8..]);
        let tu = tex_cursor.read_f32::<LittleEndian>()?;
        let tv = tex_cursor.read_f32::<LittleEndian>()?;

        let vertex = Vertex3dNoTex2 {
            x,
            y,
            z,
            nx,
            ny,
            nz,
            tu,
            tv,
        };

        // Reconstruct the full 32-byte array
        let mut bytes = [0u8; 32];

        // Write position (0-11)
        let mut byte_cursor = std::io::Cursor::new(&mut bytes[0..12]);
        byte_cursor.write_f32::<LittleEndian>(x)?;
        byte_cursor.write_f32::<LittleEndian>(y)?;
        byte_cursor.write_f32::<LittleEndian>(z)?;

        // Restore VPX normal bytes (12-23) from extras
        if let Some(ref normals) = vpx_normals
            && i < normals.len()
            && let Ok(vpx_bytes) = hex::decode(&normals[i])
            && vpx_bytes.len() == 12
        {
            bytes[12..24].copy_from_slice(&vpx_bytes);
        }

        // Write texcoords (24-31)
        let mut byte_cursor = std::io::Cursor::new(&mut bytes[24..32]);
        byte_cursor.write_f32::<LittleEndian>(tu)?;
        byte_cursor.write_f32::<LittleEndian>(tv)?;

        vertices.push(VertexWrapper::new(bytes, vertex));
    }

    let indices = read_glb_indices(bin_data, idx_offset, idx_count, use_u32)?;

    Ok((name, vertices, indices))
}

fn read_glb_indices(
    bin_data: &[u8],
    idx_offset: usize,
    idx_count: usize,
    use_u32: bool,
) -> io::Result<Vec<VpxFace>> {
    let mut indices = Vec::with_capacity(idx_count / 3);
    for i in 0..idx_count / 3 {
        let idx = if use_u32 {
            // Each face has 3 indices, each u32 is 4 bytes, so offset is i * 12
            let mut c = io::Cursor::new(&bin_data[idx_offset + i * 12..]);
            VpxFace::new(
                c.read_u32::<LittleEndian>()? as i64,
                c.read_u32::<LittleEndian>()? as i64,
                c.read_u32::<LittleEndian>()? as i64,
            )
        } else {
            // Each face has 3 indices, each u16 is 2 bytes, so offset is i * 6
            let mut c = io::Cursor::new(&bin_data[idx_offset + i * 6..]);
            VpxFace::new(
                c.read_u16::<LittleEndian>()? as i64,
                c.read_u16::<LittleEndian>()? as i64,
                c.read_u16::<LittleEndian>()? as i64,
            )
        };
        indices.push(idx);
    }
    Ok(indices)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::filesystem::MemoryFileSystem;
    use crate::vpx::model::Vertex3dNoTex2;
    use crate::vpx::obj::{VpxFace, read_obj_from_reader, write_obj_to_writer};
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use testresult::TestResult;

    #[test]
    fn test_write_read_glb() -> TestResult {
        let fs = MemoryFileSystem::new();
        let path = PathBuf::from("/test.glb");

        // Create simple test data
        let vertices = [
            Vertex3dNoTex2 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                nx: 0.0,
                ny: 1.0,
                nz: 0.0,
                tu: 0.0,
                tv: 0.0,
            },
            Vertex3dNoTex2 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                nx: 0.0,
                ny: 1.0,
                nz: 0.0,
                tu: 1.0,
                tv: 0.0,
            },
            Vertex3dNoTex2 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                nx: 0.0,
                ny: 1.0,
                nz: 0.0,
                tu: 0.0,
                tv: 1.0,
            },
        ];
        let indices = vec![VpxFace::new(0, 1, 2)];
        let vertices_with_encoded = vertices
            .iter()
            .map(|v| VertexWrapper::new(v.as_vpx_bytes(), v.clone()))
            .collect::<Vec<VertexWrapper>>();
        // Write GLB
        write_gltf(
            "TestMesh",
            &vertices_with_encoded,
            &indices,
            &path,
            GltfContainer::Glb,
            &fs,
        )?;

        // Read it back
        let (read_vertices, read_indices) = read_gltf(&path, GltfContainer::Glb, &fs)?;

        assert_eq!(vertices_with_encoded, read_vertices);
        assert_eq!(indices, read_indices);
        Ok(())
    }

    #[test]
    fn test_glb_with_nan_normals() -> TestResult {
        let fs = MemoryFileSystem::new();
        let path = PathBuf::from("/test_nan.glb");

        // Create test data with NaN normals
        let mut bytes = [0u8; 32];
        // Put some identifiable data in the normal bytes section
        bytes[12..24].copy_from_slice(&[
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
        ]);

        let vertices = vec![VertexWrapper::new(
            bytes,
            Vertex3dNoTex2 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                nx: f32::NAN,
                ny: f32::NAN,
                nz: f32::NAN,
                tu: 0.0,
                tv: 0.0,
            },
        )];
        let indices = vec![VpxFace::new(0, 0, 0)];

        // Write and read back
        write_gltf(
            "TestNaN",
            &vertices,
            &indices,
            &path,
            GltfContainer::Glb,
            &fs,
        )?;
        let (read_vertices, _) = read_gltf(&path, GltfContainer::Glb, &fs)?;

        assert_eq!(read_vertices.len(), 1);

        Ok(())
    }

    #[test]
    fn test_obj_glb_obj_round_trip() -> TestResult {
        use std::io::Cursor;

        const SCREW_OBJ_BYTES: &[u8] = include_bytes!("../../testdata/screw_f32.obj");

        // Step 1: Read the original OBJ
        let mut reader = Cursor::new(SCREW_OBJ_BYTES);
        let read_result = read_obj_from_reader(&mut reader)?;

        // TODO optimize: we don't need to convert to vpx_vertices and back for this test

        let chunked_vertices = read_result
            .vpx_encoded_vertices
            .chunks(32)
            .map(|chunk| {
                let mut array = [0u8; 32];
                array.copy_from_slice(chunk);
                array
            })
            .collect::<Vec<[u8; 32]>>();
        let vertices = chunked_vertices
            .iter()
            .zip(read_result.final_vertices.iter())
            .map(|(b, v)| VertexWrapper::new(*b, v.clone()))
            .collect::<Vec<VertexWrapper>>();

        let fs = MemoryFileSystem::new();
        let glb_path = PathBuf::from("/roundtrip.glb");

        // Step 2: Write to GLB
        let name = &read_result.name;
        let indices = &read_result.indices;
        write_gltf(name, &vertices, indices, &glb_path, GltfContainer::Glb, &fs)?;

        // Step 3: Read back from GLB
        let glb_data = fs.read_file(&glb_path)?;
        let mut glb_cursor = Cursor::new(&glb_data);
        let (glb_name, glb_vertices, glb_indices) = read_glb_from_reader(&mut glb_cursor)?;

        // Verify the name was preserved
        assert_eq!(
            read_result.name, glb_name,
            "Mesh name should be preserved in GLB round-trip"
        );

        // Step 4: Write OBJ from GLB data
        let mut screw_obj_bytes_after_roundtrip = Vec::new();
        write_obj_to_writer(
            &read_result.name,
            &glb_vertices,
            &glb_indices,
            &mut screw_obj_bytes_after_roundtrip,
        )?;

        // Step 5: Compare original OBJ with OBJ written from GLB
        let original_string = String::from_utf8(SCREW_OBJ_BYTES.to_vec())?;
        // When on Windows the original file will be checked out from git with \r\n line endings.
        let original = if cfg!(windows) {
            original_string.replace("\r\n", "\n")
        } else {
            original_string.to_string()
        };
        let after_roundtrip = String::from_utf8(screw_obj_bytes_after_roundtrip)?;

        assert_eq!(original, after_roundtrip);

        Ok(())
    }

    #[test]
    fn test_write_read_gltf() -> TestResult {
        let fs = MemoryFileSystem::new();
        let path = PathBuf::from("/test.gltf");

        let vertices = [Vertex3dNoTex2 {
            x: 0.25,
            y: 0.5,
            z: 0.75,
            nx: 0.0,
            ny: 1.0,
            nz: 0.0,
            tu: 0.1,
            tv: 0.2,
        }];
        let indices = vec![VpxFace::new(0, 0, 0)];
        let vertices_with_encoded = vertices
            .iter()
            .map(|v| VertexWrapper::new(v.as_vpx_bytes(), v.clone()))
            .collect::<Vec<VertexWrapper>>();

        write_gltf(
            "TestMesh",
            &vertices_with_encoded,
            &indices,
            &path,
            GltfContainer::Gltf,
            &fs,
        )?;

        let (read_vertices, read_indices) = read_gltf(&path, GltfContainer::Gltf, &fs)?;

        assert_eq!(vertices_with_encoded, read_vertices);
        assert_eq!(indices, read_indices);
        Ok(())
    }
}
