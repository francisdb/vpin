//! A primitive mesh as the glTF binary vpinball writes (`Mesh::SaveGLB`,
//! serialized by tinygltf): glTF axes in meters, the V texture coordinate
//! flipped, triangle winding reversed, animation frames as morph targets
//! of position and normal deltas

use super::super::json;
use crate::vpx::gameitem::primitive::{Primitive, VertData, read_vpx_animation_frame};
use serde_json::{Map, Value as Json};
use std::io;

const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const TRIANGLES: u32 = 4;

/// vpinball's `VPUTOM`: the double constant rounded to float, then a
/// float multiplication
fn vpu_to_m(value: f32) -> f32 {
    const SCALE: f32 = (0.0254 * 1.0625 / 50.0) as f32;
    value * SCALE
}

/// vpx `(x, y, z)` to glTF `(x, z, y)` in meters
fn to_gltf(x: f32, y: f32, z: f32) -> [f32; 3] {
    [vpu_to_m(x), vpu_to_m(z), vpu_to_m(y)]
}

/// A JSON object from key value pairs, which tinygltf writes sorted
fn object(entries: Vec<(&str, Json)>) -> Json {
    let mut entries = entries;
    entries.sort_by_key(|(a, _)| *a);
    Json::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Map<_, _>>(),
    )
}

#[derive(Default)]
struct Model {
    buffer: Vec<u8>,
    buffer_views: Vec<Json>,
    accessors: Vec<Json>,
}

impl Model {
    /// tinygltf leaves out a zero `byteOffset`
    fn add_accessor(
        &mut self,
        data: &[u8],
        component_type: u32,
        count: usize,
        kind: &str,
        bounds: Option<([f32; 3], [f32; 3])>,
    ) -> usize {
        let mut view = vec![
            ("buffer", Json::from(0)),
            ("byteLength", Json::from(data.len())),
        ];
        if !self.buffer.is_empty() {
            view.push(("byteOffset", Json::from(self.buffer.len())));
        }
        self.buffer_views.push(object(view));
        self.buffer.extend_from_slice(data);
        let mut accessor = vec![
            ("bufferView", Json::from(self.buffer_views.len() - 1)),
            ("componentType", Json::from(component_type)),
            ("count", Json::from(count)),
            ("type", Json::from(kind)),
        ];
        if let Some((min, max)) = bounds {
            let values = |v: [f32; 3]| Json::Array(v.iter().map(|&f| super::float(f)).collect());
            accessor.push(("max", values(max)));
            accessor.push(("min", values(min)));
        }
        self.accessors.push(object(accessor));
        self.accessors.len() - 1
    }
}

fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The GLB of a primitive's mesh, `None` for a primitive without mesh
/// vertices, which vpinball does not write a mesh for
pub(super) fn primitive_glb(primitive: &Primitive) -> io::Result<Option<Vec<u8>>> {
    let invalid = |e: crate::vpx::expanded::WriteError| io::Error::other(e.to_string());
    let Some(mesh) = primitive.read_mesh().map_err(invalid)? else {
        return Ok(None);
    };
    if mesh.vertices.is_empty() {
        return Ok(None);
    }
    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|wrapper| &wrapper.vertex)
        .collect();
    let count = vertices.len();
    let mut model = Model::default();

    let mut positions = Vec::with_capacity(count * 3);
    let mut normals = Vec::with_capacity(count * 3);
    let mut texcoords = Vec::with_capacity(count * 2);
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    for v in &vertices {
        let position = to_gltf(v.x, v.y, v.z);
        positions.extend_from_slice(&position);
        normals.extend_from_slice(&to_gltf(v.nx, v.ny, v.nz));
        texcoords.extend_from_slice(&[v.tu, 1.0 - v.tv]);
        for axis in 0..3 {
            // std::min/std::max: the first argument wins on NaN
            if position[axis] < min[axis] {
                min[axis] = position[axis];
            }
            if max[axis] < position[axis] {
                max[axis] = position[axis];
            }
        }
    }
    let position_accessor =
        model.add_accessor(&floats(&positions), FLOAT, count, "VEC3", Some((min, max)));
    let normal_accessor = model.add_accessor(&floats(&normals), FLOAT, count, "VEC3", None);
    let texcoord_accessor = model.add_accessor(&floats(&texcoords), FLOAT, count, "VEC2", None);

    let indices: Vec<i64> = mesh
        .indices
        .iter()
        .flat_map(|face| [face.i0, face.i2, face.i1])
        .collect();
    let index_accessor = if count <= 65535 {
        let data: Vec<u8> = indices
            .iter()
            .flat_map(|&i| (i as u16).to_le_bytes())
            .collect();
        model.add_accessor(&data, UNSIGNED_SHORT, indices.len(), "SCALAR", None)
    } else {
        let data: Vec<u8> = indices
            .iter()
            .flat_map(|&i| (i as u32).to_le_bytes())
            .collect();
        model.add_accessor(&data, UNSIGNED_INT, indices.len(), "SCALAR", None)
    };

    let mut targets = Vec::new();
    if let (Some(frames), Some(lengths)) = (
        &primitive.compressed_animation_vertices_data,
        &primitive.compressed_animation_vertices_len,
    ) {
        for (frame, length) in frames.iter().zip(lengths) {
            let frame: Vec<VertData> = read_vpx_animation_frame(frame, length).map_err(invalid)?;
            if frame.len() != count {
                continue;
            }
            let mut delta_positions = Vec::with_capacity(count * 3);
            let mut delta_normals = Vec::with_capacity(count * 3);
            for (f, v) in frame.iter().zip(&vertices) {
                delta_positions.extend_from_slice(&to_gltf(f.x - v.x, f.y - v.y, f.z - v.z));
                delta_normals.extend_from_slice(&to_gltf(f.nx - v.nx, f.ny - v.ny, f.nz - v.nz));
            }
            let position =
                model.add_accessor(&floats(&delta_positions), FLOAT, count, "VEC3", None);
            let normal = model.add_accessor(&floats(&delta_normals), FLOAT, count, "VEC3", None);
            targets.push(object(vec![
                ("NORMAL", Json::from(normal)),
                ("POSITION", Json::from(position)),
            ]));
        }
    }

    let mut primitive_json = vec![
        (
            "attributes",
            object(vec![
                ("NORMAL", Json::from(normal_accessor)),
                ("POSITION", Json::from(position_accessor)),
                ("TEXCOORD_0", Json::from(texcoord_accessor)),
            ]),
        ),
        ("indices", Json::from(index_accessor)),
        ("mode", Json::from(TRIANGLES)),
    ];
    if !targets.is_empty() {
        primitive_json.push(("targets", Json::Array(targets)));
    }
    let document = object(vec![
        (
            "accessors",
            Json::Array(std::mem::take(&mut model.accessors)),
        ),
        (
            "asset",
            object(vec![
                ("generator", Json::from("Visual Pinball")),
                ("version", Json::from("2.0")),
            ]),
        ),
        (
            "bufferViews",
            Json::Array(std::mem::take(&mut model.buffer_views)),
        ),
        (
            "buffers",
            Json::Array(vec![object(vec![(
                "byteLength",
                Json::from(model.buffer.len()),
            )])]),
        ),
        (
            "meshes",
            Json::Array(vec![object(vec![
                ("name", Json::from("mesh")),
                ("primitives", Json::Array(vec![object(primitive_json)])),
            ])]),
        ),
        (
            "nodes",
            Json::Array(vec![object(vec![("mesh", Json::from(0))])]),
        ),
        ("scene", Json::from(0)),
        (
            "scenes",
            Json::Array(vec![object(vec![(
                "nodes",
                Json::Array(vec![Json::from(0)]),
            )])]),
        ),
    ]);
    let content = json::to_compact_vec(&document).map_err(io::Error::other)?;
    Ok(Some(glb(&content, &model.buffer)))
}

/// The GLB container, its chunks padded to 4 bytes: the JSON with spaces,
/// the binary data with zeros
fn glb(content: &[u8], binary: &[u8]) -> Vec<u8> {
    let padding = |len: usize| (4 - len % 4) % 4;
    let content_padding = padding(content.len());
    let binary_padding = padding(binary.len());
    let mut length = 12 + 8 + content.len() + content_padding;
    if !binary.is_empty() {
        length += 8 + binary.len() + binary_padding;
    }
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&((content.len() + content_padding) as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(content);
    out.extend(std::iter::repeat_n(b' ', content_padding));
    if !binary.is_empty() {
        out.extend_from_slice(&((binary.len() + binary_padding) as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(binary);
        out.extend(std::iter::repeat_n(0, binary_padding));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::primitive::{compress_mesh_data, write_animation_vertex_data};
    use crate::vpx::mesh::test_utils::create_minimal_mesh_data;
    use bytes::BytesMut;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn animated_primitive(offsets: &[f32]) -> io::Result<Primitive> {
        let (vertices, indices, num_vertices, num_indices) = create_minimal_mesh_data();
        let base = Primitive {
            num_vertices: Some(num_vertices),
            num_indices: Some(num_indices),
            compressed_vertices_len: Some(vertices.len() as u32),
            compressed_vertices_data: Some(vertices),
            compressed_indices_len: Some(indices.len() as u32),
            compressed_indices_data: Some(indices),
            use_3d_mesh: true,
            ..Default::default()
        };
        let mesh = base
            .read_mesh()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let vertices = mesh.map(|mesh| mesh.vertices).unwrap_or_default();
        let mut frames = Vec::new();
        for offset in offsets {
            let mut buff = BytesMut::new();
            for vertex in &vertices {
                let v = &vertex.vertex;
                let moved = VertData {
                    x: v.x + offset,
                    y: v.y,
                    z: v.z + 2.0 * offset,
                    nx: v.nx,
                    ny: v.ny,
                    nz: v.nz,
                };
                write_animation_vertex_data(&mut buff, &moved);
            }
            frames.push(compress_mesh_data(&buff)?);
        }
        let lengths = frames.iter().map(|frame| frame.len() as u32).collect();
        Ok(Primitive {
            compressed_animation_vertices_len: Some(lengths),
            compressed_animation_vertices_data: Some(frames),
            ..base
        })
    }

    fn json_chunk(glb: &[u8]) -> serde_json::Result<Json> {
        let length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        serde_json::from_slice(&glb[20..20 + length])
    }

    #[test]
    fn animation_frames_become_morph_targets() -> TestResult {
        let glb = primitive_glb(&animated_primitive(&[1.0, 3.0])?)?.ok_or("a mesh")?;
        assert_eq!(&glb[..4], b"glTF");
        assert_eq!(glb.len() % 4, 0);
        let document = json_chunk(&glb)?;
        let targets = document["meshes"][0]["primitives"][0]["targets"]
            .as_array()
            .ok_or("targets")?;
        assert_eq!(targets.len(), 2);

        // the second frame moved vpx x by 3 and z by 6: glTF x and y
        let accessor = targets[1]["POSITION"].as_u64().ok_or("accessor")? as usize;
        let view = document["accessors"][accessor]["bufferView"]
            .as_u64()
            .ok_or("view")? as usize;
        let offset = document["bufferViews"][view]["byteOffset"]
            .as_u64()
            .ok_or("offset")? as usize;
        let json_length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let binary = &glb[20 + json_length + 8..];
        let delta: Vec<f32> = binary[offset..offset + 12]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        assert_eq!(delta, vec![vpu_to_m(3.0), vpu_to_m(6.0), 0.0]);
        Ok(())
    }

    #[test]
    fn a_primitive_without_mesh_vertices_has_no_glb() -> TestResult {
        let primitive = Primitive {
            use_3d_mesh: true,
            ..Default::default()
        };
        assert_eq!(primitive_glb(&primitive)?, None);
        Ok(())
    }
}
