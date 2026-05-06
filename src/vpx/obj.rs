//! Wavefront OBJ file reader and writer.
//!
//! This binds the vpx format to the wavefront obj format for easier
//! inspection and editing.
//!
//! The dialect produced and consumed here matches vpinball's own
//! `ObjLoader::Save` / `ObjLoader::Load` (with `convertToLeftHanded=true,
//! flipTv=true`) so that:
//!
//! * an OBJ extracted from a vpx by this library can be opened in the
//!   vpinball editor without flipped UVs or inside-out faces;
//! * an OBJ exported from vpinball can be assembled back into a vpx by
//!   this library.
//!
//! Concretely, on write:
//!
//! * vertex Z is negated (`obj_z = -vpx_z`),
//! * normal Z is negated (`obj_nz = -vpx_nz`),
//! * V texture coordinate is flipped (`obj_v = 1 - vpx_tv`),
//! * triangle face winding is reversed (`(i0, i1, i2)` is emitted as `f i2 i1 i0`).
//!
//! Reading inverts each of those transformations.

use crate::filesystem::FileSystem;
use crate::vpx::expanded::BytesMutExt;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::model::Vertex3dNoTex2;
use bytes::{BufMut, BytesMut};
use log::warn;
use std::error::Error;
use std::io;
use std::io::{BufRead, ErrorKind};
use std::path::Path;
use tracing::{info_span, instrument};
use wavefront_obj_io::{ObjReader, ObjWriter};
// We have some issues where the data in the vpx file contains NaN values for normals.
// Therefore, we came up with an elaborate way to store the vpx normals data as a comment in the obj file.
// To be seen if we keep this as it comes with considerable overhead.

type VpxNormalBytes = [u8; 12];

fn obj_vpx_comment(bytes: &VpxNormalBytes) -> String {
    // a comment with the full normal bytes as hex string
    let hex = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<String>>()
        .join(" ");
    format!("vpx {hex}")
}

fn obj_parse_vpx_comment(comment: &str) -> Option<VpxNormalBytes> {
    if let Some(hex) = comment.strip_prefix("vpx ") {
        let bytes = hex
            .split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).unwrap())
            .collect::<Vec<u8>>();
        if bytes.len() == 12 {
            let mut result = [0; 12];
            result.copy_from_slice(&bytes);
            Some(result)
        } else {
            None
        }
    } else {
        None
    }
}

/// Writes a wavefront obj file from the vertices and indices
/// as they are stored in the m3cx and m3ci fields of the primitive
///
/// VPinball exports obj files with negated z axis compared to vpx files internal representation.
/// So we have to negate the vertex/normal z values + reverse face winding order.
pub(crate) fn write_obj_to_writer<W: io::Write>(
    name: &str,
    vpx_vertices: &[VertexWrapper],
    vpx_indices: &[VpxFace],
    writer: &mut W,
) -> Result<(), Box<dyn Error>> {
    let mut obj_writer: wavefront_obj_io::IoObjWriter<_, f32> =
        wavefront_obj_io::IoObjWriter::new(writer);

    // // material library
    // let mtl_file_path = obj_file_path.with_extension("mtl");
    // let mtllib = Entity::MtlLib {
    //     name: mtl_file_path
    //         .file_name()
    //         .unwrap()
    //         .to_str()
    //         .unwrap()
    //         .to_string(),
    // };
    // obj_writer.write(&mut writer, &mtllib)?;

    obj_writer.write_comment("VPXTOOL table OBJ file")?;
    obj_writer.write_comment("VPXTOOL OBJ file")?;
    obj_writer.write_comment(format!(
        "numVerts: {} numFaces: {}",
        vpx_vertices.len(),
        vpx_indices.len()
    ))?;
    obj_writer.write_object_name(name)?;

    for VertexWrapper { vertex, .. } in vpx_vertices {
        obj_writer.write_vertex(vertex.x, vertex.y, -vertex.z, None)?;
    }
    for VertexWrapper { vertex, .. } in vpx_vertices {
        // vpinball's WriteVertexInfo flips V on write (tv -> 1 - tv)
        obj_writer.write_texture_coordinate(vertex.tu, Some(1.0 - vertex.tv), None)?;
    }
    for VertexWrapper {
        vpx_encoded_vertex,
        vertex,
    } in vpx_vertices
    {
        // if one of the values is NaN we write a special comment with the bytes
        if vertex.nx.is_nan() || vertex.ny.is_nan() || vertex.nz.is_nan() {
            warn!("NaN found in vertex normal: {vertex:?}");
            let data = vpx_encoded_vertex[12..24].try_into()?;
            let content = obj_vpx_comment(&data);
            obj_writer.write_comment(content)?;
        }
        let x = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
        let y = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
        let z = if vertex.nz.is_nan() { 0.0 } else { -vertex.nz };
        obj_writer.write_normal(x, y, z)?;
    }
    // Write all faces in groups of 3. vpinball's WriteFaceInfoLong reverses
    // the winding order ((i0, i1, i2) emitted as `f i2 i1 i0`); we match it.
    for face in vpx_indices {
        // obj indices are 1 based
        let v1 = face.i2 + 1;
        let v2 = face.i1 + 1;
        let v3 = face.i0 + 1;
        obj_writer.write_face(&[
            (v1 as usize, Some(v1 as usize), Some(v1 as usize)),
            (v2 as usize, Some(v2 as usize), Some(v2 as usize)),
            (v3 as usize, Some(v3 as usize), Some(v3 as usize)),
        ])?;
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct ReadObjResult {
    #[allow(unused)]
    pub(crate) name: String,
    // TOD investigate if we really need this and try to keep symmetry with the writing side
    #[allow(unused)]
    pub(crate) final_vertices: Vec<Vertex3dNoTex2>,
    pub(crate) vertices: Vec<(f32, f32, f32, Option<f32>)>,
    pub(crate) indices: Vec<VpxFace>,
    pub(crate) vpx_encoded_vertices: BytesMut,
}

pub(crate) fn read_obj_from_reader<R: BufRead>(mut reader: &mut R) -> io::Result<ReadObjResult> {
    let ObjData {
        name,
        vertices,
        texture_coordinates,
        normals,
        indices,
    } = read_obj(&mut reader).map_err(|e| io::Error::other(format!("Error reading obj: {}", e)))?;

    let mut final_vertices = Vec::with_capacity(vertices.len());
    let mut vpx_encoded_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for ((v, vt), vn) in vertices
        .iter()
        .zip(texture_coordinates.iter())
        .zip(normals.iter())
    {
        let nx = vn.x;
        let ny = vn.y;
        let nz = -vn.z;

        let vertext = Vertex3dNoTex2 {
            x: v.0,
            y: v.1,
            z: -v.2,
            nx,
            ny,
            nz,
            tu: vt.0,
            // vpinball's ObjLoader::Load (with flipTv=true) inverts the
            // V flip applied on write.
            tv: 1.0 - vt.1.unwrap_or(0.0),
        };
        write_vertex(&mut vpx_encoded_vertices, &vertext, &vn.vpx_bytes);
        final_vertices.push(vertext);
    }

    Ok(ReadObjResult {
        name,
        final_vertices,
        vertices,
        indices,
        vpx_encoded_vertices,
    })
}

pub(crate) fn write_vertex_index_for_vpx(
    bytes_per_index: u8,
    vpx_indices: &mut BytesMut,
    vertex_index: i64,
) {
    if bytes_per_index == 2 {
        vpx_indices.put_u16_le(vertex_index as u16);
    } else {
        vpx_indices.put_u32_le(vertex_index as u32);
    }
}

fn write_vertex(
    buff: &mut BytesMut,
    vertex: &Vertex3dNoTex2,
    vpx_vertex_normal_data: &Option<[u8; 12]>,
) {
    buff.put_f32_le(vertex.x);
    buff.put_f32_le(vertex.y);
    buff.put_f32_le(vertex.z);
    // normals
    if let Some(bytes) = vpx_vertex_normal_data {
        buff.put_slice(bytes);
    } else {
        buff.put_f32_le_nan_as_zero(vertex.nx);
        buff.put_f32_le_nan_as_zero(vertex.ny);
        buff.put_f32_le_nan_as_zero(vertex.nz);
    }
    // texture coordinates
    buff.put_f32_le(vertex.tu);
    buff.put_f32_le(vertex.tv);
}

#[instrument(skip(vertices, indices, fs, obj_file_path), fields(path = ?obj_file_path, vertex_count = vertices.len(), index_count = indices.len()))]
pub(crate) fn write_obj(
    name: &str,
    vertices: &[VertexWrapper],
    indices: &[VpxFace],
    obj_file_path: &Path,
    fs: &dyn FileSystem,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = Vec::new();
    write_obj_to_writer(name, vertices, indices, &mut buffer)?;

    let _span = info_span!("fs_write", bytes = buffer.len()).entered();
    fs.write_file(obj_file_path, &buffer)?;

    Ok(())
}

#[derive(Default)]
struct VpxObjReader {
    /// Raw face corners as they appear in the OBJ. Each face may be a
    /// triangle (vpinball-format) or an n-gon with mismatched `v/vt/vn`
    /// (Blender-format).
    raw_faces: Vec<Vec<FaceCorner>>,
    /// Set if any face is not already in vpinball's strict triangle +
    /// matched-indices form. Drives the post-parse normalization fast path.
    needs_normalize: bool,
    vertices: Vec<(f32, f32, f32, Option<f32>)>,
    texture_coordinates: Vec<(f32, Option<f32>, Option<f32>)>,
    normals: Vec<VpxObjNormal>,
    object_count: usize,
    /// keeps the previous comment to be associated with the next normal
    previous_comment: Option<String>,
    name: String,
}

impl VpxObjReader {
    fn new() -> Self {
        Self {
            raw_faces: Vec::with_capacity(8 * 1024),
            needs_normalize: false,
            vertices: Vec::with_capacity(8 * 1024),
            texture_coordinates: Vec::with_capacity(8 * 1024),
            normals: Vec::with_capacity(8 * 1024),
            object_count: 0,
            previous_comment: None,
            name: String::new(),
        }
    }

    /// Reads the stream and returns the ObjData consuming the reader.
    ///
    /// If every face is a vpinball-format triangle (3 corners with matching
    /// `v/vt/vn`), the v/vt/vn arrays are returned as-is and the indices
    /// list is built directly with the per-triangle reverse that mirrors
    /// vpinball's `WriteFaceInfoLong`.
    ///
    /// Otherwise (Blender-style n-gons or mismatched corners), the
    /// [`triangulate_and_dedup`] step runs over the collected data with
    /// `reverse_corners=true`: faces are corner-reversed, fan-triangulated
    /// and `(pos, uv, normal)` corners are deduplicated. The v/vt/vn arrays
    /// are rebuilt to be aligned (same length, one entry per combined
    /// corner).
    fn read<R: io::Read>(mut self, reader: &mut R) -> io::Result<ObjData> {
        wavefront_obj_io::read_obj_file(reader, &mut self)?;
        if self.object_count != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Only a single object is supported for vpx, found {}",
                    self.object_count
                ),
            ));
        }

        if self.needs_normalize {
            let normalized = triangulate_and_dedup(
                &self.raw_faces,
                self.vertices.len(),
                self.texture_coordinates.len(),
                self.normals.len(),
                true,
            )?;

            let aligned_vertices = normalized
                .combined
                .iter()
                .map(|(p, _, _)| self.vertices[*p])
                .collect();
            let aligned_tex_coords = normalized
                .combined
                .iter()
                .map(|(_, t, _)| self.texture_coordinates[*t])
                .collect();
            let aligned_normals = normalized
                .combined
                .iter()
                .map(|(_, _, n)| self.normals[*n].clone())
                .collect();
            // Triangles from `triangulate_and_dedup` are already in
            // vpinball's m_indices convention (matches the result of
            // `ObjLoader::Load`'s corner-reverse + fan-triangulate +
            // dedup). No further reversal needed here.
            let indices = normalized
                .triangles
                .iter()
                .map(|t| VpxFace {
                    i0: t[0] as i64,
                    i1: t[1] as i64,
                    i2: t[2] as i64,
                })
                .collect();

            Ok(ObjData {
                name: self.name,
                vertices: aligned_vertices,
                texture_coordinates: aligned_tex_coords,
                normals: aligned_normals,
                indices,
            })
        } else {
            // Strict fast path: each face is already a triangle with
            // matching v/vt/vn. Reverse the per-triangle corner order to
            // undo vpinball's `WriteFaceInfoLong` reversal.
            let indices = self
                .raw_faces
                .iter()
                .map(|face| VpxFace {
                    i0: face[2].v as i64 - 1,
                    i1: face[1].v as i64 - 1,
                    i2: face[0].v as i64 - 1,
                })
                .collect();
            Ok(ObjData {
                name: self.name,
                vertices: self.vertices,
                texture_coordinates: self.texture_coordinates,
                normals: self.normals,
                indices,
            })
        }
    }
}

impl ObjReader<f32> for VpxObjReader {
    fn read_comment(&mut self, comment: &str) {
        self.previous_comment = Some(comment.to_string());
    }

    fn read_object_name(&mut self, name: &str) {
        self.object_count += 1;
        self.name = name.to_string();
        self.previous_comment = None;
    }

    fn read_vertex(&mut self, x: f32, y: f32, z: f32, w: Option<f32>) {
        self.vertices.push((x, y, z, w));
        self.previous_comment = None;
    }

    fn read_texture_coordinate(&mut self, u: f32, v: Option<f32>, w: Option<f32>) {
        self.texture_coordinates.push((u, v, w));
        self.previous_comment = None;
    }

    fn read_normal(&mut self, nx: f32, ny: f32, nz: f32) {
        // If on the write side there was a NaN value that will be stored in a comment
        // This way we stay symmetric
        if let Some(comment) = &self.previous_comment {
            // parse the comment as hex string
            if let Some(bytes) = obj_parse_vpx_comment(comment) {
                // use the bytes as the normal
                self.normals
                    .push(VpxObjNormal::new(nx, ny, nz, Some(bytes)));
            } else {
                self.normals.push(VpxObjNormal::new(nx, ny, nz, None));
            }
        } else {
            self.normals.push(VpxObjNormal::new(nx, ny, nz, None));
        }
        self.previous_comment = None;
    }

    fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {
        let mut all_matched = vertex_indices.len() == 3;
        let corners: Vec<FaceCorner> = vertex_indices
            .iter()
            .map(|(v, vt, vn)| {
                let v = *v as u32;
                let vt = vt.map(|x| x as u32).unwrap_or(0);
                let vn = vn.map(|x| x as u32).unwrap_or(0);
                if v != vt || v != vn {
                    all_matched = false;
                }
                FaceCorner { v, vt, vn }
            })
            .collect();
        if !all_matched {
            self.needs_normalize = true;
        }
        self.raw_faces.push(corners);
        self.previous_comment = None;
    }
}

#[instrument(skip(reader))]
pub(crate) fn read_obj<R: BufRead>(mut reader: &mut R) -> std::io::Result<ObjData> {
    let vpx_reader = VpxObjReader::new();
    vpx_reader.read(&mut reader)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VpxObjNormal {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    // in case the normal had NaN values, we store the original vpx bytes here
    vpx_bytes: Option<VpxNormalBytes>,
}
impl VpxObjNormal {
    fn new(x: f32, y: f32, z: f32, vpx_bytes: Option<VpxNormalBytes>) -> Self {
        Self { x, y, z, vpx_bytes }
    }
}

/// A face in the vpx file, consisting of three vertex indices
///
/// zero-based indices
/// vpx based winding order
///
/// Normally the faces are also storing pointers to texture and normal indices,
/// but in vpx files these are always the same as the vertex indices.
///
/// *I do wonder if these indices can be negative?*
/// TODO do these really need to be i64?
#[derive(Debug, Clone, PartialEq)]
pub struct VpxFace {
    pub i0: i64,
    pub i1: i64,
    pub i2: i64,
}
impl VpxFace {
    pub(crate) fn new(i0: i64, i1: i64, i2: i64) -> Self {
        Self { i0, i1, i2 }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct ObjData {
    pub name: String,
    pub vertices: Vec<(f32, f32, f32, Option<f32>)>,
    pub texture_coordinates: Vec<(f32, Option<f32>, Option<f32>)>,
    pub normals: Vec<VpxObjNormal>,
    /// Indices can also be relative, so they can be negative
    /// stored by three as vertex, texture, normal are all the same
    ///
    /// Here they are 0-based, in obj files they are 1-based
    pub indices: Vec<VpxFace>,
}

// ---------------------------------------------------------------------------
// OBJ format normalization shared between the lenient read path and the
// standalone OBJ -> OBJ converter exposed via wasm.
// ---------------------------------------------------------------------------

/// One corner of a face as parsed from a `f` line. `vt` / `vn` are 0 when
/// missing in the source - that turns into an `InvalidIndex` error during
/// [`triangulate_and_dedup`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceCorner {
    pub(crate) v: u32,
    pub(crate) vt: u32,
    pub(crate) vn: u32,
}

/// Result of [`triangulate_and_dedup`].
pub(crate) struct NormalizedFaces {
    /// One entry per unique combined `(pos_idx, uv_idx, normal_idx)` tuple,
    /// in the order it was first seen by the dedup walk. Indices are
    /// 0-based into the source position / texcoord / normal arrays.
    pub(crate) combined: Vec<(usize, usize, usize)>,
    /// Triangle indices into [`Self::combined`].
    pub(crate) triangles: Vec<[u32; 3]>,
}

fn resolve_index(idx: u32, len: usize, kind: &str, lineno_hint: &str) -> io::Result<usize> {
    if idx == 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("{}: missing {} index in face", lineno_hint, kind),
        ));
    }
    let resolved = idx as usize - 1;
    if resolved >= len {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "{}: {} index {} out of range (have {})",
                lineno_hint, kind, idx, len
            ),
        ));
    }
    Ok(resolved)
}

/// Fan-triangulate every face and deduplicate `(pos, uv, normal)` corners
/// into a flat vertex array.
///
/// When `reverse_corners` is true, each face's corners are reversed before
/// triangulation - matches vpinball's `ObjLoader::Load`, used by the
/// lenient path of [`read_obj_from_reader`] so a Blender-exported OBJ
/// produces the same mesh as vpinball would.
///
/// When false, faces triangulate in their original OBJ order, preserving
/// the source winding direction. Used by the renderer-friendly mesh API.
pub(crate) fn triangulate_and_dedup(
    raw_faces: &[Vec<FaceCorner>],
    positions_len: usize,
    tex_coords_len: usize,
    normals_len: usize,
    reverse_corners: bool,
) -> io::Result<NormalizedFaces> {
    let mut triangles_with_corners: Vec<[(usize, usize, usize); 3]> =
        Vec::with_capacity(raw_faces.len());
    for face in raw_faces {
        if face.len() < 3 {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "face with less than 3 vertices",
            ));
        }
        let mut corners = Vec::with_capacity(face.len());
        for c in face {
            let p = resolve_index(c.v, positions_len, "vertex", "face")?;
            let t = resolve_index(c.vt, tex_coords_len, "texcoord", "face")?;
            let n = resolve_index(c.vn, normals_len, "normal", "face")?;
            corners.push((p, t, n));
        }

        if reverse_corners {
            corners.reverse();
        }

        // Fan triangulation: (0, i, i+1) for i in 1..n-1.
        for i in 1..corners.len() - 1 {
            triangles_with_corners.push([corners[0], corners[i], corners[i + 1]]);
        }
    }

    let mut combined: Vec<(usize, usize, usize)> = Vec::new();
    let mut combined_lookup = std::collections::HashMap::<(usize, usize, usize), u32>::new();
    let mut triangles: Vec<[u32; 3]> = Vec::with_capacity(triangles_with_corners.len());

    for tri in &triangles_with_corners {
        let mut idx = [0u32; 3];
        for (k, corner) in tri.iter().enumerate() {
            let next = combined.len() as u32;
            let entry = combined_lookup.entry(*corner).or_insert_with(|| {
                combined.push(*corner);
                next
            });
            idx[k] = *entry;
        }
        triangles.push(idx);
    }

    Ok(NormalizedFaces {
        combined,
        triangles,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::filesystem::MemoryFileSystem;
    use pretty_assertions::assert_eq;
    use std::io::{BufReader, Cursor};
    use testresult::TestResult;

    #[test]
    fn read_minimal_obj() -> TestResult {
        let obj_contents = r#"
o minimal
v 1.0 2.0 3.0
vt 2.0 4.0
vn 0.0 1.0 0.0
f 1/1/1 1/1/1 1/1/1
        "#;
        let mut reader = BufReader::new(obj_contents.as_bytes());
        let read_data = read_obj(&mut reader)?;
        let expected = ObjData {
            name: "minimal".to_string(),
            vertices: vec![(1.0f32, 2.0f32, 3.0f32, None)],
            texture_coordinates: vec![(2.0f32, Some(4.0f32), None)],
            normals: vec![VpxObjNormal::new(0.0f32, 1.0f32, 0.0f32, None)],
            indices: vec![VpxFace::new(0, 0, 0)],
        };
        assert_eq!(read_data, expected);
        Ok(())
    }

    #[test]
    fn roundtrip_minimal_obj() -> TestResult {
        // minimal obj with a single triangle
        let obj_contents = r#"# VPXTOOL table OBJ file
# VPXTOOL OBJ file
# numVerts: 3 numFaces: 1
o minimal
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
vn 0 0 1
vn 0 0 1
f 1/1/1 2/2/2 3/3/3
"#;

        let mut reader = BufReader::new(obj_contents.as_bytes());
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
            .map(|(b, v)| VertexWrapper {
                vpx_encoded_vertex: *b,
                vertex: (*v).clone(),
            })
            .collect::<Vec<VertexWrapper>>();

        let mut buffer = Vec::new();
        write_obj_to_writer(
            &read_result.name,
            &vertices,
            &read_result.indices,
            &mut buffer,
        )?;

        let written_obj_contents = String::from_utf8(buffer)?;
        // When on Windows the original file will be checked out with \r\n line endings.
        let original = if cfg!(windows) {
            obj_contents.replace("\r\n", "\n")
        } else {
            obj_contents.to_string()
        };
        // The obj file will always be written with \n line endings.
        assert_eq!(original, written_obj_contents);
        Ok(())
    }

    #[test]
    fn test_read_obj_with_nan() -> TestResult {
        let obj_contents = r#"o with_nan
v 1.0 2.0 3.0
vt 2.0 4.0
# vpx 01 02 03 04 05 06 07 08 09 0a 0b 0c
vn NaN 1.0 0.0
vn 1.0 2.0 3.0
f 1/1/1 1/1/1 1/1/1
        "#;
        let mut reader = BufReader::new(obj_contents.as_bytes());
        let read_data = read_obj(&mut reader)?;
        let expected = ObjData {
            name: "with_nan".to_string(),
            vertices: vec![(1.0f32, 2.0f32, 3.0f32, None)],
            texture_coordinates: vec![(2.0f32, Some(4.0f32), None)],
            normals: vec![
                VpxObjNormal::new(
                    f32::NAN,
                    1.0f32,
                    0.0f32,
                    Some([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
                ),
                VpxObjNormal::new(1.0f32, 2.0f32, 3.0f32, None),
            ],
            indices: vec![VpxFace::new(0, 0, 0)],
        };
        // we can't compare a structure with NaN values
        assert_eq!(read_data.name, expected.name);
        assert_eq!(read_data.vertices, expected.vertices);
        assert_eq!(read_data.texture_coordinates, expected.texture_coordinates);
        assert_eq!(read_data.normals.len(), expected.normals.len());
        assert_eq!(
            read_data.normals.first().unwrap().y,
            expected.normals.first().unwrap().y
        );
        assert_eq!(read_data.normals[1].y, expected.normals[1].y);
        assert_eq!(read_data.indices, expected.indices);
        Ok(())
    }

    #[test]
    #[should_panic(expected = "InvalidDigit")]
    fn test_read_obj_with_nan_invalid() {
        let obj_contents = r#"o with_nan
v 1.0 2.0 3.0
vt 2.0 4.0
# vpx 01 02 03 04 05 06 07 08 09 0a 0b 0c compouter says no
vn NaN 1.0 0.0
f 1/1/1 1/1/1 1/1/1
        "#;
        let mut reader = BufReader::new(obj_contents.as_bytes());
        read_obj(&mut reader).unwrap();
    }

    const VPIN_SCREW2_OBJ_BYTES: &[u8] = include_bytes!("../../testdata/vpin_screw2.obj");

    #[test]
    fn test_read_write_obj() -> TestResult {
        let mut reader = Cursor::new(VPIN_SCREW2_OBJ_BYTES);
        let obj_data = read_obj(&mut reader)?;

        // Convert each parsed OBJ corner into a `VertexWrapper` in vpx
        // coordinates. This mirrors what `read_obj_from_reader` does: negate
        // Z on positions and normals, and flip the V texture coordinate
        // (`vpx_tv = 1 - obj_v`) so the subsequent `write_obj` round-trips
        // back to the original OBJ bytes.
        use byteorder::{LittleEndian, WriteBytesExt};
        let vertices: Vec<VertexWrapper> = obj_data
            .vertices
            .iter()
            .zip(&obj_data.texture_coordinates)
            .zip(&obj_data.normals)
            .map(|((v, vt), vn)| {
                let mut bytes = [0u8; 32];

                let z = -v.2;
                let nz = -vn.z;
                let tv = 1.0 - vt.1.unwrap_or(0.0);

                // Write position bytes (0-11)
                let mut cursor = std::io::Cursor::new(&mut bytes[0..12]);
                cursor.write_f32::<LittleEndian>(v.0).unwrap();
                cursor.write_f32::<LittleEndian>(v.1).unwrap();
                cursor.write_f32::<LittleEndian>(z).unwrap();

                // Write normal bytes (12-23)
                // If we have VPX bytes from OBJ, use them, otherwise encode the floats
                if let Some(vpx_bytes) = &vn.vpx_bytes {
                    bytes[12..24].copy_from_slice(vpx_bytes);
                } else {
                    // Encode normals as floats
                    let mut cursor = std::io::Cursor::new(&mut bytes[12..24]);
                    cursor.write_f32::<LittleEndian>(vn.x).unwrap();
                    cursor.write_f32::<LittleEndian>(vn.y).unwrap();
                    cursor.write_f32::<LittleEndian>(nz).unwrap();
                }

                // Write texcoord bytes (24-31)
                let mut cursor = std::io::Cursor::new(&mut bytes[24..32]);
                cursor.write_f32::<LittleEndian>(vt.0).unwrap();
                cursor.write_f32::<LittleEndian>(tv).unwrap();

                VertexWrapper {
                    vpx_encoded_vertex: bytes,
                    vertex: Vertex3dNoTex2 {
                        x: v.0,
                        y: v.1,
                        z,
                        nx: vn.x,
                        ny: vn.y,
                        nz,
                        tu: vt.0,
                        tv,
                    },
                }
            })
            .collect();

        let memory_fs = MemoryFileSystem::default();
        let written_obj_path = Path::new("screw.obj");
        write_obj(
            &obj_data.name,
            &vertices,
            &obj_data.indices,
            written_obj_path,
            &memory_fs,
        )?;

        // compare both files as strings
        let mut original = String::from_utf8(VPIN_SCREW2_OBJ_BYTES.to_vec())?;
        // When on Windows the original file will be checked out with \r\n line endings.
        if cfg!(windows) {
            original = original.replace("\r\n", "\n")
        }
        // The obj file will always be written with \n line endings.
        let written = memory_fs.read_to_string(written_obj_path)?;
        assert_eq!(original, written);
        Ok(())
    }

    #[test]
    fn test_read_write_obj_fs() -> TestResult {
        use crate::vpx::gameitem::primitive::VertexWrapper;
        use crate::vpx::obj::{read_obj_from_reader, write_obj};

        let fs = MemoryFileSystem::default();
        let obj_path = Path::new("test.obj");

        // put the bytes in the memory filesystem
        fs.write_file(obj_path, VPIN_SCREW2_OBJ_BYTES)?;

        let obj_data = fs.read_file(obj_path)?;
        let mut reader = BufReader::new(Cursor::new(obj_data));
        let read_result = read_obj_from_reader(&mut reader)?;

        // FIXME we don't have the encoded vertex data, so we just put zeros here
        //   the read/write is not symmetrical because of this
        let wrapped_vertices = read_result
            .final_vertices
            .iter()
            .map(|v| VertexWrapper {
                vpx_encoded_vertex: [0; 32],
                vertex: (*v).clone(),
            })
            .collect::<Vec<VertexWrapper>>();

        write_obj(
            &read_result.name,
            &wrapped_vertices,
            &read_result.indices,
            obj_path,
            &fs,
        )?;

        let mut original_string = String::from_utf8(VPIN_SCREW2_OBJ_BYTES.to_vec())?;
        // on windows obj files are written with \r\n line endings
        if cfg!(windows) {
            original_string = original_string.replace("\r\n", "\n");
        }

        let written_bytes = fs.read_file(obj_path)?;
        let written_string = String::from_utf8(written_bytes)?;

        assert_eq!(original_string, written_string);

        Ok(())
    }

    #[test]
    fn test_write_read_vpx_comment() {
        let bytes: VpxNormalBytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let comment = obj_vpx_comment(&bytes);
        let parsed = obj_parse_vpx_comment(&comment).unwrap();
        assert_eq!(bytes, parsed);
    }

    #[test]
    fn test_read_obj_invalid() {
        use crate::vpx::obj::read_obj_from_reader;

        let fs = MemoryFileSystem::default();
        let obj_path = Path::new("invalid.obj");

        // put invalid bytes in the memory filesystem
        fs.write_file(obj_path, b"this is not a valid obj file")
            .unwrap();

        let obj_data = fs.read_file(obj_path).unwrap();
        let mut reader = BufReader::new(Cursor::new(obj_data));
        let read_result = read_obj_from_reader(&mut reader);
        assert!(read_result.is_err());
        // message
        assert_eq!(
            read_result.unwrap_err().to_string(),
            "Error reading obj: line 1: Unknown line prefix: this"
        );
    }

    /// Reading a Blender-format OBJ (n-gons + mismatched `v/vt/vn` indices)
    /// must succeed: faces get fan-triangulated and corners get deduplicated
    /// in the same way vpinball's `ObjLoader::Load` would.
    #[test]
    fn test_read_blender_square_directly() -> TestResult {
        let blender = include_bytes!("../../testdata/blender_square.obj");
        let mut reader = BufReader::new(Cursor::new(blender));
        let result = read_obj_from_reader(&mut reader)?;

        // Blender default cube: 6 quads -> 12 triangles, with 4 distinct
        // (pos, uv, normal) corners per face -> 24 unique combined entries.
        assert_eq!(result.indices.len(), 12);
        assert_eq!(result.final_vertices.len(), 24);
        Ok(())
    }

    /// Verify the lenient read of `blender_square.obj` produces vpx data
    /// equivalent to what reading the vpinball-exported reference
    /// (`vpinball_square.obj`) produces - i.e., the strict path on a real
    /// vpinball OBJ and the lenient path on the Blender source agree on
    /// the resulting mesh.
    #[test]
    fn test_lenient_and_strict_paths_agree_on_cube() -> TestResult {
        let blender_bytes = include_bytes!("../../testdata/blender_square.obj");
        let vpinball_bytes = include_bytes!("../../testdata/vpinball_square.obj");

        let mut blender_reader = BufReader::new(Cursor::new(blender_bytes));
        let lenient = read_obj_from_reader(&mut blender_reader)?;

        let mut vpinball_reader = BufReader::new(Cursor::new(vpinball_bytes));
        let strict = read_obj_from_reader(&mut vpinball_reader)?;

        assert_eq!(lenient.final_vertices.len(), strict.final_vertices.len());
        assert_eq!(lenient.indices.len(), strict.indices.len());

        // Quantize floats to 6 decimals (vpinball's `%f` precision) and
        // build a canonical-rotation triangle set per side. If both meshes
        // describe the same surface, these sets must match.
        fn q(v: f32) -> i64 {
            (v as f64 * 1_000_000.0).round() as i64
        }
        type CornerQ = ((i64, i64, i64), (i64, i64), (i64, i64, i64));
        fn canonical_rotation(t: &[CornerQ; 3]) -> [CornerQ; 3] {
            let r0 = [t[0], t[1], t[2]];
            let r1 = [t[1], t[2], t[0]];
            let r2 = [t[2], t[0], t[1]];
            [r0, r1, r2].into_iter().min().unwrap()
        }
        fn triangle_set(r: &ReadObjResult) -> std::collections::BTreeSet<[CornerQ; 3]> {
            r.indices
                .iter()
                .map(|f| {
                    let corners: [CornerQ; 3] = [f.i0, f.i1, f.i2].map(|idx| {
                        let v = &r.final_vertices[idx as usize];
                        (
                            (q(v.x), q(v.y), q(v.z)),
                            (q(v.tu), q(v.tv)),
                            (q(v.nx), q(v.ny), q(v.nz)),
                        )
                    });
                    canonical_rotation(&corners)
                })
                .collect()
        }
        assert_eq!(triangle_set(&lenient), triangle_set(&strict));
        Ok(())
    }
}
