//! GLB writer for a single primitive mesh, converted to the glTF frame

use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gltf::*;
use crate::vpx::le::WriteLe;
use crate::vpx::obj::VpxFace;
use serde_json::json;
use std::error::Error;
use std::io;
#[cfg(test)]
use {crate::vpx::le::ReadLe, std::io::Read};

pub(crate) struct GltfPayload {
    pub(crate) json: serde_json::Value,
    pub(crate) bin_data: Vec<u8>,
}

/// How [`build_gltf_payload`] converts the vpx-internal mesh data.
///
/// Winding is derived, not configured: the per-triangle corner order is
/// reversed exactly when `axes` flips handedness relative to
/// vpx-internal, matching the OBJ and whole-table glTF exporters.
pub(crate) struct SingleMeshConversion {
    /// Axis map applied to positions and normals.
    pub(crate) axes: crate::vpx::units::AxisConvention,
    /// Multiplier applied to positions only (never normals).
    pub(crate) position_scale: f32,
}

pub(crate) fn build_gltf_payload(
    name: &str,
    vertices: &[VertexWrapper],
    indices: &[VpxFace],
    conversion: &SingleMeshConversion,
) -> Result<GltfPayload, Box<dyn Error>> {
    use crate::vpx::units::AxisConvention;

    let axes = conversion.axes;
    let scale = conversion.position_scale;
    let reverse_winding = axes.flips_handedness(AxisConvention::ZUpLeftHanded);

    // Build binary buffer with all vertex data
    let mut bin_data = Vec::new();

    // Write positions (VEC3 float), scaled and mapped to the target
    // axes. The scale multiplication is skipped entirely at 1.0 so the
    // identity conversion stays bit-exact for every input, and the
    // mapped values are kept for the min/max accessor bounds below.
    let positions_offset = 0;
    let mut mapped_positions = Vec::with_capacity(vertices.len());
    for VertexWrapper { vertex, .. } in vertices {
        let (x, y, z) = if scale == 1.0 {
            (vertex.x, vertex.y, vertex.z)
        } else {
            (vertex.x * scale, vertex.y * scale, vertex.z * scale)
        };
        let mapped = axes.from_vpx(x, y, z);
        bin_data.write_f32_le(mapped[0])?;
        bin_data.write_f32_le(mapped[1])?;
        bin_data.write_f32_le(mapped[2])?;
        mapped_positions.push(mapped);
    }
    let positions_length = bin_data.len();

    // Write normals (VEC3 float) - same axis mapping, never scaled,
    // NaN -> 0
    let normals_offset = bin_data.len();
    for VertexWrapper { vertex, .. } in vertices {
        let nx = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
        let ny = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
        let nz = if vertex.nz.is_nan() { 0.0 } else { vertex.nz };
        let [nx, ny, nz] = axes.from_vpx(nx, ny, nz);

        bin_data.write_f32_le(nx)?;
        bin_data.write_f32_le(ny)?;
        bin_data.write_f32_le(nz)?;
    }
    let normals_length = bin_data.len() - normals_offset;

    // Write texcoords (VEC2 float)
    let texcoords_offset = bin_data.len();
    for VertexWrapper { vertex, .. } in vertices {
        bin_data.write_f32_le(vertex.tu)?;
        bin_data.write_f32_le(vertex.tv)?;
    }
    let texcoords_length = bin_data.len() - texcoords_offset;

    // Write indices (SCALAR uint16 or uint32)
    let indices_offset = bin_data.len();
    let use_u32 = vertices.len() > 65535;
    for face in indices {
        // Reverse the corner order (swap i1/i2) when the axis map flips
        // handedness, so front faces stay front.
        let (i1, i2) = if reverse_winding {
            (face.i2, face.i1)
        } else {
            (face.i1, face.i2)
        };
        if use_u32 {
            bin_data.write_u32_le(face.i0 as u32)?;
            bin_data.write_u32_le(i1 as u32)?;
            bin_data.write_u32_le(i2 as u32)?;
        } else {
            bin_data.write_u16_le(face.i0 as u16)?;
            bin_data.write_u16_le(i1 as u16)?;
            bin_data.write_u16_le(i2 as u16)?;
        }
    }
    let indices_length = bin_data.len() - indices_offset;

    // Pad binary data to 4-byte alignment
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let buffers = json!([{
        "byteLength": bin_data.len(),
    }]);

    // Create GLTF JSON structure; POSITION bounds are computed over the
    // mapped values actually written to the buffer.
    let (min_x, max_x, min_y, max_y, min_z, max_z) = mapped_positions.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y, min_z, max_z), [x, y, z]| {
            (
                min_x.min(*x),
                max_x.max(*x),
                min_y.min(*y),
                max_y.max(*y),
                min_z.min(*z),
                max_z.max(*z),
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

pub(crate) fn write_glb_payload<W: io::Write>(
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
    writer.write_u32_le(GLTF_VERSION)?; // version
    let total_length = GLB_HEADER_BYTES
        + GLB_CHUNK_HEADER_BYTES
        + json_padded_length as u32
        + GLB_CHUNK_HEADER_BYTES
        + payload.bin_data.len() as u32;
    writer.write_u32_le(total_length)?; // length

    // Write JSON chunk
    writer.write_u32_le(json_padded_length as u32)?; // chunk length
    writer.write_all(GLB_JSON_CHUNK_TYPE)?; // chunk type
    writer.write_all(json_bytes)?;
    for _ in 0..json_padding {
        writer.write_all(b" ")?; // space padding
    }

    // Write BIN chunk
    writer.write_u32_le(payload.bin_data.len() as u32)?; // chunk length
    writer.write_all(GLB_BIN_CHUNK_TYPE)?; // chunk type
    writer.write_all(&payload.bin_data)?;

    Ok(())
}

#[cfg(test)]
/// Reads a GLB file from a reader and returns the name, vertices and indices.
///
/// Only the tests use it, to check what the writer produced.
///
/// Returns: `(name, vertices, indices)`
pub(crate) fn read_glb_from_reader<R: Read>(
    reader: &mut R,
) -> io::Result<(String, Vec<VertexWrapper>, Vec<VpxFace>)> {
    let payload = read_glb_payload_from_reader(reader)?;
    parse_gltf_payload(&payload.json, &payload.bin_data)
}

#[cfg(test)]
fn read_glb_payload_from_reader<R: Read>(reader: &mut R) -> io::Result<GltfPayload> {
    use crate::vpx::le::ReadLe;

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

    let version = cursor.read_u32_le()?;
    if version != GLTF_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported GLTF version: {}", version),
        ));
    }

    let _total_length = cursor.read_u32_le()?;

    // Read JSON chunk
    let json_length = cursor.read_u32_le()? as usize;
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
    let bin_length = cursor.read_u32_le()? as usize;
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

#[cfg(test)]
fn invalid_data(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
/// Read a non-negative integer field of a glTF JSON object
fn json_usize(value: &serde_json::Value, what: &str) -> io::Result<usize> {
    value
        .as_u64()
        .map(|v| v as usize)
        .ok_or_else(|| invalid_data(format!("Missing or invalid {what}")))
}

#[cfg(test)]
/// Look up an accessor and the buffer view it points to
fn accessor_and_view<'a>(
    accessors: &'a [serde_json::Value],
    buffer_views: &'a [serde_json::Value],
    index: usize,
    what: &str,
) -> io::Result<(&'a serde_json::Value, &'a serde_json::Value)> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| invalid_data(format!("Missing {what} accessor {index}")))?;
    let view_index = json_usize(
        &accessor["bufferView"],
        &format!("{what} accessor bufferView"),
    )?;
    let view = buffer_views
        .get(view_index)
        .ok_or_else(|| invalid_data(format!("Missing {what} bufferView {view_index}")))?;
    Ok((accessor, view))
}

#[cfg(test)]
/// A bounds-checked window into the binary buffer
fn bin_slice<'a>(
    bin_data: &'a [u8],
    offset: usize,
    len: usize,
    what: &str,
) -> io::Result<&'a [u8]> {
    offset
        .checked_add(len)
        .and_then(|end| bin_data.get(offset..end))
        .ok_or_else(|| {
            invalid_data(format!(
                "{what} at offset {offset} ({len} bytes) is outside the {} byte buffer",
                bin_data.len()
            ))
        })
}

#[cfg(test)]
fn parse_gltf_payload(
    gltf_json: &serde_json::Value,
    bin_data: &[u8],
) -> io::Result<(String, Vec<VertexWrapper>, Vec<VpxFace>)> {
    use crate::vpx::le::ReadLe;
    use crate::vpx::model::Vertex3dNoTex2;

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

    // Read positions (accessor 0)
    let (pos_accessor, pos_view) = accessor_and_view(accessors, buffer_views, 0, "position")?;
    let pos_offset = json_usize(&pos_view["byteOffset"], "position byteOffset")?;
    let pos_count = json_usize(&pos_accessor["count"], "position count")?;

    // Read normals (accessor 1)
    let (_, norm_view) = accessor_and_view(accessors, buffer_views, 1, "normal")?;
    let norm_offset = json_usize(&norm_view["byteOffset"], "normal byteOffset")?;

    // Read texcoords (accessor 2)
    let (_, tex_view) = accessor_and_view(accessors, buffer_views, 2, "texcoord")?;
    let tex_offset = json_usize(&tex_view["byteOffset"], "texcoord byteOffset")?;

    // Read indices (accessor 3)
    let (idx_accessor, idx_view) = accessor_and_view(accessors, buffer_views, 3, "index")?;
    let idx_offset = json_usize(&idx_view["byteOffset"], "index byteOffset")?;
    let idx_count = json_usize(&idx_accessor["count"], "index count")?;
    let idx_component_type = json_usize(&idx_accessor["componentType"], "index componentType")?;
    let use_u32 = idx_component_type == GLTF_COMPONENT_TYPE_UNSIGNED_INT as usize; // UNSIGNED_INT

    // the whole vertex range must fit, not just the first vertex
    bin_slice(bin_data, pos_offset, pos_count * 12, "position data")?;
    bin_slice(bin_data, norm_offset, pos_count * 12, "normal data")?;
    bin_slice(bin_data, tex_offset, pos_count * 8, "texcoord data")?;

    // Build vertex data in the same format as write_glb accepts
    let mut vertices = Vec::with_capacity(pos_count);

    for i in 0..pos_count {
        let mut pos_cursor = io::Cursor::new(&bin_data[pos_offset + i * 12..]);
        let x = pos_cursor.read_f32_le()?;
        let y = pos_cursor.read_f32_le()?;
        let z = pos_cursor.read_f32_le()?;

        let mut norm_cursor = io::Cursor::new(&bin_data[norm_offset + i * 12..]);
        let nx = norm_cursor.read_f32_le()?;
        let ny = norm_cursor.read_f32_le()?;
        let nz = norm_cursor.read_f32_le()?;

        let mut tex_cursor = io::Cursor::new(&bin_data[tex_offset + i * 8..]);
        let tu = tex_cursor.read_f32_le()?;
        let tv = tex_cursor.read_f32_le()?;

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
        byte_cursor.write_f32_le(x)?;
        byte_cursor.write_f32_le(y)?;
        byte_cursor.write_f32_le(z)?;

        // Write texcoords (24-31)
        let mut byte_cursor = std::io::Cursor::new(&mut bytes[24..32]);
        byte_cursor.write_f32_le(tu)?;
        byte_cursor.write_f32_le(tv)?;

        vertices.push(VertexWrapper::new(bytes, vertex));
    }

    let indices = read_glb_indices(bin_data, idx_offset, idx_count, use_u32)?;

    Ok((name, vertices, indices))
}

#[cfg(test)]
fn read_glb_indices(
    bin_data: &[u8],
    idx_offset: usize,
    idx_count: usize,
    use_u32: bool,
) -> io::Result<Vec<VpxFace>> {
    let index_size = if use_u32 { 4 } else { 2 };
    bin_slice(bin_data, idx_offset, idx_count * index_size, "index data")?;
    let mut indices = Vec::with_capacity(idx_count / 3);
    for i in 0..idx_count / 3 {
        let idx = if use_u32 {
            // Each face has 3 indices, each u32 is 4 bytes, so offset is i * 12
            let mut c = io::Cursor::new(&bin_data[idx_offset + i * 12..]);
            VpxFace::new(
                c.read_u32_le()? as i64,
                c.read_u32_le()? as i64,
                c.read_u32_le()? as i64,
            )
        } else {
            // Each face has 3 indices, each u16 is 2 bytes, so offset is i * 6
            let mut c = io::Cursor::new(&bin_data[idx_offset + i * 6..]);
            VpxFace::new(
                c.read_u16_le()? as i64,
                c.read_u16_le()? as i64,
                c.read_u16_le()? as i64,
            )
        };
        indices.push(idx);
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::model::Vertex3dNoTex2;
    use crate::vpx::units::AxisConvention;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    #[test]
    fn single_mesh_glb_is_in_the_gltf_frame() -> TestResult {
        let vertex = |x: f32, y: f32, z: f32| {
            let v = Vertex3dNoTex2 {
                x,
                y,
                z,
                nx: 0.0,
                ny: 0.0,
                nz: 1.0,
                tu: 0.0,
                tv: 0.0,
            };
            VertexWrapper::new(v.as_vpx_bytes(), v)
        };
        let vertices = [
            vertex(0.0, 0.0, 0.5),
            vertex(1.0, 0.0, 0.5),
            vertex(0.0, 1.0, 0.5),
        ];
        let conversion = SingleMeshConversion {
            axes: AxisConvention::YUpRightHanded,
            position_scale: 2.0,
        };
        let payload = build_gltf_payload("tri", &vertices, &[VpxFace::new(0, 1, 2)], &conversion)?;
        let mut glb = Vec::new();
        write_glb_payload(&payload, &mut glb)?;

        let (name, vertices, faces) = read_glb_from_reader(&mut io::Cursor::new(&glb))?;
        assert_eq!(name, "tri");
        // vpx (0, 1, 0.5) * 2 -> glTF (0, 1, 2), normals unscaled, winding reversed
        let v2 = &vertices[2].vertex;
        assert_eq!((v2.x, v2.y, v2.z), (0.0, 1.0, 2.0));
        assert_eq!((v2.nx, v2.ny, v2.nz), (0.0, 1.0, 0.0));
        assert_eq!(faces, vec![VpxFace::new(0, 2, 1)]);
        Ok(())
    }
}
