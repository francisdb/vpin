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
pub(super) mod tests {
    use super::*;
    use crate::vpx::gameitem::primitive::{compress_mesh_data, write_animation_vertex_data};
    use crate::vpx::mesh::test_utils::create_minimal_mesh_data;
    use bytes::BytesMut;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    pub(super) fn animated_primitive(offsets: &[f32]) -> io::Result<Primitive> {
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

/// A mesh as vpinball reads it from a glTF binary
pub(super) struct GlbMesh {
    pub(super) vertices: Vec<crate::vpx::model::Vertex3dNoTex2>,
    /// Three per triangle, in vpx winding
    pub(super) indices: Vec<u32>,
    pub(super) frames: Vec<Vec<VertData>>,
}

/// vpinball's `MTOVPU`: the double constant rounded to float, then a
/// float multiplication
fn m_to_vpu(value: f32) -> f32 {
    const SCALE: f32 = (50.0 / (0.0254 * 1.0625)) as f32;
    value * SCALE
}

/// glTF `(x, y, z)` in meters to vpx `(x, z, y)`
fn to_vpx(x: f32, y: f32, z: f32) -> [f32; 3] {
    [m_to_vpu(x), m_to_vpu(z), m_to_vpu(y)]
}

struct GlbFile<'a> {
    json: Json,
    binary: &'a [u8],
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn parse_glb(data: &[u8]) -> io::Result<GlbFile<'_>> {
    let word = |offset: usize| -> io::Result<u32> {
        data.get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| invalid("truncated GLB".to_string()))
    };
    if data.get(..4) != Some(b"glTF") {
        return Err(invalid("not a GLB file".to_string()));
    }
    let mut offset = 12;
    let mut json = None;
    let mut binary: &[u8] = &[];
    while offset + 8 <= data.len() {
        let length = word(offset)? as usize;
        let kind = word(offset + 4)?;
        let chunk = data
            .get(offset + 8..offset + 8 + length)
            .ok_or_else(|| invalid("truncated GLB chunk".to_string()))?;
        match kind {
            0x4E4F_534A => {
                json = Some(
                    serde_json::from_slice(chunk).map_err(|e| invalid(format!("GLB JSON: {e}")))?,
                );
            }
            0x004E_4942 => binary = chunk,
            _ => {}
        }
        offset += 8 + length;
    }
    let json = json.ok_or_else(|| invalid("GLB without JSON chunk".to_string()))?;
    Ok(GlbFile { json, binary })
}

impl GlbFile<'_> {
    fn number(value: &Json, what: &str) -> io::Result<usize> {
        value
            .as_u64()
            .map(|v| v as usize)
            .ok_or_else(|| invalid(format!("GLB: missing {what}")))
    }

    /// The bytes an accessor starts at, its stride and count
    fn accessor(
        &self,
        index: usize,
        component_size: usize,
        components: usize,
    ) -> io::Result<(usize, usize, usize)> {
        let accessor = &self.json["accessors"][index];
        let view_index = Self::number(&accessor["bufferView"], "accessor bufferView")?;
        let view = &self.json["bufferViews"][view_index];
        // only the binary chunk of a GLB is supported as buffer
        let buffer = Self::number(&view["buffer"], "bufferView buffer")?;
        if self.json["buffers"][buffer].get("uri").is_some() {
            return Err(invalid(
                "GLB: external buffers are not supported".to_string(),
            ));
        }
        let base = view["byteOffset"].as_u64().unwrap_or(0) as usize
            + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = match view["byteStride"].as_u64().unwrap_or(0) as usize {
            0 => component_size * components,
            stride => stride,
        };
        let count = Self::number(&accessor["count"], "accessor count")?;
        let end = count
            .checked_sub(1)
            .map_or(Some(base), |last| {
                last.checked_mul(stride)
                    .and_then(|o| o.checked_add(base + component_size * components))
            })
            .ok_or_else(|| invalid("GLB: accessor out of range".to_string()))?;
        if end > self.binary.len() {
            return Err(invalid(
                "GLB: accessor outside the binary chunk".to_string(),
            ));
        }
        Ok((base, stride, count))
    }

    /// A float accessor as a flat array (`ReadFloatAccessor`), `None` for
    /// one that is missing, empty or not of floats
    fn floats(&self, index: Option<usize>, components: usize) -> io::Result<Option<Vec<f32>>> {
        let Some(index) =
            index.filter(|&i| i < self.json["accessors"].as_array().map_or(0, Vec::len))
        else {
            return Ok(None);
        };
        let accessor = &self.json["accessors"][index];
        if accessor["componentType"] != FLOAT || accessor["count"].as_u64().unwrap_or(0) == 0 {
            return Ok(None);
        }
        let (base, stride, count) = self.accessor(index, 4, components)?;
        let mut values = Vec::with_capacity(count * components);
        for i in 0..count {
            for c in 0..components {
                let o = base + i * stride + c * 4;
                let b = &self.binary[o..o + 4];
                values.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
        }
        Ok(Some(values))
    }
}

/// The mesh of a glTF binary as vpinball reads it (`Mesh::LoadGLB`): the
/// first triangle primitive, converted to vpx axes in VP units, the V
/// texture coordinate flipped back, triangle winding reversed back, morph
/// targets as animation frames. A mesh without normals gets `(0, 0, 1)`.
pub(super) fn read_glb(data: &[u8]) -> io::Result<GlbMesh> {
    let glb = parse_glb(data)?;
    let primitive = glb.json["meshes"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|mesh| mesh["primitives"].as_array().into_iter().flatten())
        .find(|primitive| {
            primitive["mode"]
                .as_u64()
                .is_none_or(|mode| mode == u64::from(TRIANGLES))
        })
        .ok_or_else(|| invalid("GLB mesh contains no triangle primitive".to_string()))?;
    let attribute = |name: &str| primitive["attributes"][name].as_u64().map(|i| i as usize);
    let positions = glb
        .floats(attribute("POSITION"), 3)?
        .ok_or_else(|| invalid("GLB mesh has no positions".to_string()))?;
    let normals = glb.floats(attribute("NORMAL"), 3)?;
    let texcoords = glb.floats(attribute("TEXCOORD_0"), 2)?;
    let count = positions.len() / 3;
    if normals.as_ref().is_some_and(|n| n.len() != positions.len())
        || texcoords.as_ref().is_some_and(|t| t.len() != count * 2)
    {
        return Err(invalid(
            "GLB mesh has inconsistent attribute vertex counts".to_string(),
        ));
    }

    let index_accessor = primitive["indices"]
        .as_u64()
        .ok_or_else(|| invalid("GLB mesh primitive has no index buffer".to_string()))?
        as usize;
    let component_size = match glb.json["accessors"][index_accessor]["componentType"].as_u64() {
        Some(5121) => 1,
        Some(5123) => 2,
        _ => 4,
    };
    let (base, stride, index_count) = glb.accessor(index_accessor, component_size, 1)?;
    let index = |i: usize| -> u32 {
        let o = base + i * stride;
        let b = &glb.binary[o..o + component_size];
        match component_size {
            1 => u32::from(b[0]),
            2 => u32::from(u16::from_le_bytes([b[0], b[1]])),
            _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        }
    };
    let mut indices = Vec::with_capacity(index_count);
    for triangle in 0..index_count / 3 {
        let i = triangle * 3;
        indices.extend_from_slice(&[index(i), index(i + 2), index(i + 1)]);
    }

    let mut vertices = Vec::with_capacity(count);
    for i in 0..count {
        let [x, y, z] = to_vpx(positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);
        let [nx, ny, nz] = match &normals {
            Some(n) => to_vpx(n[i * 3], n[i * 3 + 1], n[i * 3 + 2]),
            None => [0.0, 0.0, 1.0],
        };
        let (tu, tv) = match &texcoords {
            Some(t) => (t[i * 2], 1.0 - t[i * 2 + 1]),
            None => (0.0, 0.0),
        };
        vertices.push(crate::vpx::model::Vertex3dNoTex2 {
            x,
            y,
            z,
            nx,
            ny,
            nz,
            tu,
            tv,
        });
    }

    let mut frames = Vec::new();
    for target in primitive["targets"].as_array().into_iter().flatten() {
        let Some(delta_positions) =
            glb.floats(target["POSITION"].as_u64().map(|i| i as usize), 3)?
        else {
            continue;
        };
        if delta_positions.len() != positions.len() {
            continue;
        }
        let delta_normals = glb
            .floats(target["NORMAL"].as_u64().map(|i| i as usize), 3)?
            .filter(|d| d.len() == positions.len());
        let frame = (0..count)
            .map(|i| {
                let p = |c: usize| positions[i * 3 + c] + delta_positions[i * 3 + c];
                let [x, y, z] = to_vpx(p(0), p(1), p(2));
                let [nx, ny, nz] = match (&delta_normals, &normals) {
                    (Some(d), Some(n)) => {
                        let v = |c: usize| n[i * 3 + c] + d[i * 3 + c];
                        to_vpx(v(0), v(1), v(2))
                    }
                    _ => [vertices[i].nx, vertices[i].ny, vertices[i].nz],
                };
                VertData {
                    x,
                    y,
                    z,
                    nx,
                    ny,
                    nz,
                }
            })
            .collect();
        frames.push(frame);
    }
    Ok(GlbMesh {
        vertices,
        indices,
        frames,
    })
}

#[cfg(test)]
mod read_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    /// A GLB of one triangle as another tool may write it: positions and
    /// texture coordinates interleaved, no normals, 8-bit indices
    fn foreign_glb() -> Vec<u8> {
        let mut binary = Vec::new();
        for (position, uv) in [
            ([0.0f32, 0.0, 0.0], [0.0f32, 0.0]),
            ([1.0, 0.0, 0.0], [1.0, 0.0]),
            ([0.0, 0.5, 0.25], [0.0, 1.0]),
        ] {
            for v in position.iter().chain(&uv) {
                binary.extend_from_slice(&v.to_le_bytes());
            }
        }
        binary.extend_from_slice(&[0, 1, 2, 0]);
        let document = serde_json::json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": binary.len()}],
            "bufferViews": [
                {"buffer": 0, "byteLength": 60, "byteStride": 20},
                {"buffer": 0, "byteOffset": 60, "byteLength": 3}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": FLOAT, "count": 3, "type": "VEC3"},
                {"bufferView": 0, "byteOffset": 12, "componentType": FLOAT, "count": 3, "type": "VEC2"},
                {"bufferView": 1, "componentType": 5121, "count": 3, "type": "SCALAR"}
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "TEXCOORD_0": 1}, "indices": 2}]}]
        });
        glb(&serde_json::to_vec(&document).unwrap_or_default(), &binary)
    }

    #[test]
    fn a_foreign_layout_is_read_like_vpinball_reads_it() -> TestResult {
        let mesh = read_glb(&foreign_glb())?;
        assert_eq!(mesh.indices, vec![0, 2, 1]);
        let v = &mesh.vertices[2];
        // glTF (0, 0.5, 0.25) m is vpx (0, 0.25, 0.5) m, in VP units
        assert_eq!((v.x, v.y, v.z), (0.0, m_to_vpu(0.25), m_to_vpu(0.5)));
        assert_eq!((v.nx, v.ny, v.nz), (0.0, 0.0, 1.0));
        assert_eq!((v.tu, v.tv), (0.0, 0.0));
        assert!(mesh.frames.is_empty());
        Ok(())
    }

    #[test]
    fn a_written_mesh_reads_back_with_its_frames() -> TestResult {
        let primitive = tests::animated_primitive(&[1.0, 3.0])?;
        let glb = primitive_glb(&primitive)?.ok_or("a mesh")?;
        let mesh = read_glb(&glb)?;
        let original = primitive.read_mesh()?.ok_or("a mesh")?;
        let original_indices: Vec<u32> = original
            .indices
            .iter()
            .flat_map(|face| [face.i0 as u32, face.i1 as u32, face.i2 as u32])
            .collect();
        assert_eq!(mesh.indices, original_indices);
        assert_eq!(mesh.vertices.len(), original.vertices.len());
        for (read, written) in mesh.vertices.iter().zip(&original.vertices) {
            let written = &written.vertex;
            assert!((read.x - written.x).abs() <= written.x.abs() * 2.5e-7 + 1e-9);
            assert_eq!((read.tu, read.tv), (written.tu, written.tv));
        }
        assert_eq!(mesh.frames.len(), 2);
        Ok(())
    }
}
