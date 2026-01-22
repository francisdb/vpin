//! Wavefront OBJ file reader and writer

use crate::filesystem::FileSystem;
use crate::vpx::expanded::BytesMutExt;
use crate::vpx::gameitem::primitive::MAX_VERTICES_FOR_2_BYTE_INDEX;
use crate::vpx::model::Vertex3dNoTex2;
use crate::wavefront_obj_io;
use crate::wavefront_obj_io::{ObjReader, ObjWriter};
use bytes::{BufMut, BytesMut};
use log::warn;
use std::error::Error;
use std::io;
use std::io::BufRead;
use std::path::Path;
use tracing::{info_span, instrument};
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
/// Somehow the z axis is inverted compared to the vpx file values,
/// so we have to negate the z values.
/// This also affects winding order of the faces.
/// TODO we should be reversing the winding order as well here instead of higher up in the code
pub(crate) fn write_obj_to_writer<W: io::Write>(
    name: &str,
    vertices: &[([u8; 32], Vertex3dNoTex2)],
    indices: &[i64],
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
        vertices.len(),
        indices.len()
    ))?;
    obj_writer.write_object_name(name)?;

    for (_, vertex) in vertices {
        obj_writer.write_vertex(vertex.x, vertex.y, -vertex.z, None)?;
    }
    for (_, vertex) in vertices {
        obj_writer.write_texture_coordinate(vertex.tu, Some(vertex.tv), None)?;
    }
    for (bytes, vertex) in vertices {
        // if one of the values is NaN we write a special comment with the bytes
        if vertex.nx.is_nan() || vertex.ny.is_nan() || vertex.nz.is_nan() {
            warn!("NaN found in vertex normal: {vertex:?}");
            let data = bytes[12..24].try_into()?;
            let content = obj_vpx_comment(&data);
            obj_writer.write_comment(content)?;
        }
        let x = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
        let y = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
        let z = if vertex.nz.is_nan() { 0.0 } else { -vertex.nz };
        obj_writer.write_normal(x, y, z)?;
    }
    // write all faces in groups of 3
    for chunk in indices.chunks(3) {
        // obj indices are 1 based
        let v1 = chunk[0] + 1;
        let v2 = chunk[1] + 1;
        let v3 = chunk[2] + 1;
        obj_writer.write_face(&[
            (v1 as usize, Some(v1 as usize), Some(v1 as usize)),
            (v2 as usize, Some(v2 as usize), Some(v2 as usize)),
            (v3 as usize, Some(v3 as usize), Some(v3 as usize)),
        ])?;
    }
    Ok(())
}

pub(crate) fn read_obj_from_reader<R: BufRead>(
    mut reader: &mut R,
) -> io::Result<(
    String,
    Vec<Vertex3dNoTex2>,
    Vec<(f32, f32, f32, Option<f32>)>,
    Vec<i64>,
    BytesMut,
    BytesMut,
)> {
    let ObjData {
        name,
        vertices,
        texture_coordinates,
        normals,
        indices,
    } = read_obj(&mut reader).map_err(|e| io::Error::other(format!("Error reading obj: {}", e)))?;

    let mut final_vertices = Vec::with_capacity(vertices.len());
    let mut vpx_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for ((v, vt), vn) in vertices
        .iter()
        .zip(texture_coordinates.iter())
        .zip(normals.iter())
    {
        let (normal, vpx_vertex_normal_data) = vn;
        let nx = normal.0;
        let ny = normal.1;
        let nz = -(normal.2);

        let vertext = Vertex3dNoTex2 {
            x: v.0,
            y: v.1,
            z: -(v.2),
            nx,
            ny,
            nz,
            tu: vt.0,
            tv: vt.1.unwrap_or(0.0),
        };
        write_vertex(&mut vpx_vertices, &vertext, vpx_vertex_normal_data);
        final_vertices.push(vertext);
    }

    let bytes_per_index: u8 = if vertices.len() > MAX_VERTICES_FOR_2_BYTE_INDEX {
        4
    } else {
        2
    };
    let mut vpx_indices = BytesMut::with_capacity(indices.len() * bytes_per_index as usize);
    for chunk in indices.chunks(3) {
        let v1 = chunk[0];
        let v2 = chunk[1];
        let v3 = chunk[2];
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v3);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v2);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, v1);
    }
    // TODO this should just return optional vpx bytes if they are overridden
    Ok((
        name,
        final_vertices,
        vertices,
        indices,
        vpx_vertices,
        vpx_indices,
    ))
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
    vertices: &[([u8; 32], Vertex3dNoTex2)],
    indices: &[i64],
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
    indices: Vec<i64>,
    vertices: Vec<(f32, f32, f32, Option<f32>)>,
    texture_coordinates: Vec<(f32, Option<f32>, Option<f32>)>,
    normals: Vec<ObjNormal>,
    object_count: usize,
    /// keeps the previous comment to be associated with the next normal
    previous_comment: Option<String>,
    name: String,
}

impl VpxObjReader {
    fn new() -> Self {
        Self {
            indices: Vec::with_capacity(8 * 1024),
            vertices: Vec::with_capacity(8 * 1024),
            texture_coordinates: Vec::with_capacity(8 * 1024),
            normals: Vec::with_capacity(8 * 1024),
            object_count: 0,
            previous_comment: None,
            name: String::new(),
        }
    }

    /// Reads the stream and returns the ObjData consuming the reader
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
        Ok(ObjData {
            name: self.name,
            vertices: self.vertices,
            texture_coordinates: self.texture_coordinates,
            normals: self.normals,
            indices: self.indices,
        })
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
                self.normals.push(((nx, ny, nz), Some(bytes)));
            } else {
                self.normals.push(((nx, ny, nz), None));
            }
        } else {
            self.normals.push(((nx, ny, nz), None));
        }
        self.previous_comment = None;
    }

    fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {
        self.indices.push(vertex_indices[0].0 as i64 - 1);
        self.indices.push(vertex_indices[1].0 as i64 - 1);
        self.indices.push(vertex_indices[2].0 as i64 - 1);
        self.previous_comment = None;
    }
}

#[instrument(skip(reader))]
pub(crate) fn read_obj<R: BufRead>(mut reader: &mut R) -> std::io::Result<ObjData> {
    let vpx_reader = VpxObjReader::new();
    vpx_reader.read(&mut reader)
}

pub type ObjNormal = ((f32, f32, f32), Option<VpxNormalBytes>);

#[derive(Debug, PartialEq)]
pub(crate) struct ObjData {
    pub name: String,
    pub vertices: Vec<(f32, f32, f32, Option<f32>)>,
    pub texture_coordinates: Vec<(f32, Option<f32>, Option<f32>)>,
    pub normals: Vec<ObjNormal>,
    /// Indices can also be relative, so they can be negative
    /// stored by three as vertex, texture, normal are all the same
    ///
    /// Here they are 0-based, in obj files they are 1-based
    pub indices: Vec<i64>,
}

impl ObjData {
    /// Converts ObjData to the vertex format used by write_glb and write_obj.
    ///
    /// This creates the 32-byte vertex arrays with proper VPX normal bytes.
    /// If VPX bytes are available from the OBJ comments, they are used directly.
    /// Otherwise, the normal float values are encoded as bytes.
    ///
    /// When `negate_z` is true, the z coordinates are negated for VPX format compatibility.
    pub(crate) fn to_vertices(&self, negate_z: bool) -> Vec<([u8; 32], Vertex3dNoTex2)> {
        use byteorder::{LittleEndian, WriteBytesExt};

        self.vertices
            .iter()
            .zip(&self.texture_coordinates)
            .zip(&self.normals)
            .map(|((v, vt), (vn, vpx_bytes_opt))| {
                let mut bytes = [0u8; 32];

                let z = if negate_z { -v.2 } else { v.2 };
                let nz = if negate_z { -vn.2 } else { vn.2 };

                // Write position bytes (0-11)
                let mut cursor = std::io::Cursor::new(&mut bytes[0..12]);
                cursor.write_f32::<LittleEndian>(v.0).unwrap();
                cursor.write_f32::<LittleEndian>(v.1).unwrap();
                cursor.write_f32::<LittleEndian>(z).unwrap();

                // Write normal bytes (12-23)
                // If we have VPX bytes from OBJ, use them, otherwise encode the floats
                if let Some(vpx_bytes) = vpx_bytes_opt {
                    bytes[12..24].copy_from_slice(vpx_bytes);
                } else {
                    // Encode normals as floats
                    let mut cursor = std::io::Cursor::new(&mut bytes[12..24]);
                    cursor.write_f32::<LittleEndian>(vn.0).unwrap();
                    cursor.write_f32::<LittleEndian>(vn.1).unwrap();
                    cursor.write_f32::<LittleEndian>(nz).unwrap();
                }

                // Write texcoord bytes (24-31)
                let mut cursor = std::io::Cursor::new(&mut bytes[24..32]);
                cursor.write_f32::<LittleEndian>(vt.0).unwrap();
                cursor
                    .write_f32::<LittleEndian>(vt.1.unwrap_or(0.0))
                    .unwrap();

                (
                    bytes,
                    Vertex3dNoTex2 {
                        x: v.0,
                        y: v.1,
                        z,
                        nx: vn.0,
                        ny: vn.1,
                        nz,
                        tu: vt.0,
                        tv: vt.1.unwrap_or(0.0),
                    },
                )
            })
            .collect()
    }
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
            normals: vec![((0.0f32, 1.0f32, 0.0f32), None)],
            indices: vec![0, 0, 0],
        };
        assert_eq!(read_data, expected);
        Ok(())
    }

    #[test]
    fn roundtrip_minimal_obj() -> TestResult {
        let obj_contents = r#"# VPXTOOL table OBJ file
# VPXTOOL OBJ file
# numVerts: 1 numFaces: 3
o minimal
v 1 2 3
vt 2 4
vn 0 1 0
f 1/1/1 1/1/1 1/1/1
"#;

        let mut reader = BufReader::new(obj_contents.as_bytes());
        let (name, vertices, _vertices2, indices, vpx_vertices, _read_data_vpx_indices) =
            read_obj_from_reader(&mut reader)?;

        // TODO optimize: we don't need to convert to vpx_vertices and back for this test

        let chunked_vertices = vpx_vertices
            .chunks(32)
            .map(|chunk| {
                let mut array = [0u8; 32];
                array.copy_from_slice(chunk);
                array
            })
            .collect::<Vec<[u8; 32]>>();
        let vertices = chunked_vertices
            .iter()
            .zip(vertices.iter())
            .map(|(b, v)| (*b, v.clone()))
            .collect::<Vec<([u8; 32], Vertex3dNoTex2)>>();

        let mut buffer = Vec::new();
        write_obj_to_writer(&name, &vertices, &indices, &mut buffer)?;

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
                (
                    (f32::NAN, 1.0f32, 0.0f32),
                    Some([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
                ),
                ((1.0f32, 2.0f32, 3.0f32), None),
            ],
            indices: vec![0, 0, 0],
        };
        // we can't compare a structure with NaN values
        assert_eq!(read_data.name, expected.name);
        assert_eq!(read_data.vertices, expected.vertices);
        assert_eq!(read_data.texture_coordinates, expected.texture_coordinates);
        assert_eq!(read_data.normals.len(), expected.normals.len());
        assert_eq!(
            read_data.normals.first().unwrap().1,
            expected.normals.first().unwrap().1
        );
        assert_eq!(read_data.normals[1].1, expected.normals[1].1);
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

    const SCREW_OBJ_BYTES: &[u8] = include_bytes!("../../testdata/screw_f32.obj");

    #[test]
    fn test_read_write_obj() -> TestResult {
        let mut reader = Cursor::new(SCREW_OBJ_BYTES);
        let obj_data = read_obj(&mut reader)?;

        // Convert to vertex format (no z-axis negation for OBJ-to-OBJ round-trip)
        let vertices = obj_data.to_vertices(true);

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
        let mut original = String::from_utf8(SCREW_OBJ_BYTES.to_vec())?;
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
    fn test_write_read_vpx_comment() {
        let bytes: VpxNormalBytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let comment = obj_vpx_comment(&bytes);
        let parsed = obj_parse_vpx_comment(&comment).unwrap();
        assert_eq!(bytes, parsed);
    }
}
