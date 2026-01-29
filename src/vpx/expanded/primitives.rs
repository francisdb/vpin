//! Primitive mesh reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive;
use crate::vpx::gameitem::primitive::{
    MAX_VERTICES_FOR_2_BYTE_INDEX, ReadMesh, VertData, VertexWrapper, read_vpx_animation_frame,
    write_animation_vertex_data,
};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::{
    ObjData, ReadObjResult, VpxFace, read_obj as obj_read_obj, read_obj_from_reader, write_obj,
    write_vertex_index_for_vpx,
};
use byteorder::{LittleEndian, ReadBytesExt};
use bytes::{BufMut, BytesMut};
use std::io::{self, Read};
use std::iter::Zip;
use std::path::Path;
use std::slice::Iter;
use tracing::instrument;

use super::{PrimitiveMeshFormat, WriteError};

pub(super) fn write_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem: &GameItemEnum,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    if let GameItemEnum::Primitive(primitive) = gameitem
        && let Some(ReadMesh { vertices, indices }) = &primitive.read_mesh()?
    {
        match mesh_format {
            PrimitiveMeshFormat::Obj => {
                let obj_path = gameitems_dir.join(format!("{json_file_name}.obj"));
                write_obj(gameitem.name(), vertices, indices, &obj_path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            PrimitiveMeshFormat::Glb => {
                let glb_path = gameitems_dir.join(format!("{json_file_name}.glb"));
                crate::vpx::gltf::write_glb(gameitem.name(), vertices, indices, &glb_path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
        }

        if let Some(animation_frames) = &primitive.compressed_animation_vertices_data {
            if let Some(compressed_lengths) = &primitive.compressed_animation_vertices_len {
                let zipped = animation_frames.iter().zip(compressed_lengths.iter());
                write_animation_frames_to_meshes(
                    gameitems_dir,
                    gameitem.name(),
                    json_file_name,
                    vertices,
                    indices,
                    zipped,
                    mesh_format,
                    fs,
                )?;
            } else {
                return Err(WriteError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Animation frames should always come with counts: {json_file_name}"),
                )));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_animation_frames_to_meshes(
    gameitems_dir: &Path,
    name: &str,
    json_file_name: &str,
    vertices: &[VertexWrapper],
    vpx_indices: &[VpxFace],
    zipped: Zip<Iter<Vec<u8>>, Iter<u32>>,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    for (i, (compressed_frame, compressed_length)) in zipped.enumerate() {
        let animation_frame_vertices =
            read_vpx_animation_frame(compressed_frame, compressed_length);
        let full_vertices = replace_vertices(vertices, animation_frame_vertices)?;
        let file_name_without_ext = json_file_name.trim_end_matches(".json");
        let file_name = animation_frame_file_name(file_name_without_ext, i, mesh_format);
        let mesh_path = gameitems_dir.join(&file_name);

        match mesh_format {
            PrimitiveMeshFormat::Obj => {
                write_obj(name, &full_vertices, vpx_indices, &mesh_path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            PrimitiveMeshFormat::Glb => {
                crate::vpx::gltf::write_glb(name, &full_vertices, vpx_indices, &mesh_path, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
        }
    }
    Ok(())
}

fn replace_vertices(
    vertices: &[VertexWrapper],
    animation_frame_vertices: Result<Vec<VertData>, WriteError>,
) -> Result<Vec<VertexWrapper>, WriteError> {
    // combine animation_vertices with the vertices and indices from the mesh
    let full_vertices = vertices
        .iter()
        .zip(animation_frame_vertices?.iter())
        .map(|(VertexWrapper { vertex, .. }, animation_vertex)| {
            let mut full_vertex: Vertex3dNoTex2 = (*vertex).clone();
            full_vertex.x = animation_vertex.x;
            full_vertex.y = animation_vertex.y;
            full_vertex.z = animation_vertex.z;
            full_vertex.nx = animation_vertex.nx;
            full_vertex.ny = animation_vertex.ny;
            full_vertex.nz = animation_vertex.nz;
            // TODO we don't have a full representation of the vertex
            VertexWrapper::new([0u8; 32], full_vertex)
        })
        .collect::<Vec<_>>();
    Ok(full_vertices)
}

pub trait BytesMutExt {
    fn put_f32_le_nan_as_zero(&mut self, value: f32);
}

impl BytesMutExt for BytesMut {
    fn put_f32_le_nan_as_zero(&mut self, value: f32) {
        if value.is_nan() {
            // DieHard_272.vpx primitive "BM_pAirDuctGate" has a NaN value for nx
            // with value like [113, 93, 209, 255] in the vpx.
            // NaN is translated to 0.0 when exporting in vpinball windows.
            self.put_f32_le(0.0);
        } else {
            self.put_f32_le(value);
        }
    }
}

pub(super) fn read_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem_file_name: String,
    mut item: GameItemEnum,
    fs: &dyn FileSystem,
) -> io::Result<GameItemEnum> {
    if let GameItemEnum::Primitive(primitive) = &mut item {
        let gameitem_file_name = gameitem_file_name.trim_end_matches(".json");

        // Check for OBJ first (backward compatibility), then GLB
        let obj_path = gameitems_dir.join(format!("{gameitem_file_name}.obj"));
        let glb_path = gameitems_dir.join(format!("{gameitem_file_name}.glb"));

        let mesh_format = if fs.exists(&obj_path) {
            Some(PrimitiveMeshFormat::Obj)
        } else if fs.exists(&glb_path) {
            Some(PrimitiveMeshFormat::Glb)
        } else {
            None
        };

        if let Some(format) = mesh_format {
            let (vertices_len, indices_len, compressed_vertices, compressed_indices) = match format
            {
                PrimitiveMeshFormat::Obj => {
                    let read_result = read_obj(&obj_path, fs)?;
                    let vertices_len = read_result.vertices.len();
                    let serialized_indices_len = read_result.indices.len() * 3;

                    let vpx_encoded_indices =
                        vpx_encode_vertices(read_result.vertices.len(), &read_result.indices);

                    let (compressed_vertices, compressed_indices) = compress_vertices_and_indices(
                        &read_result.vpx_encoded_vertices,
                        &vpx_encoded_indices,
                    )?;

                    (
                        vertices_len,
                        serialized_indices_len,
                        compressed_vertices,
                        compressed_indices,
                    )
                }
                PrimitiveMeshFormat::Glb => read_glb_and_compress(&glb_path, fs)?,
            };
            primitive.num_vertices = Some(vertices_len as u32);
            primitive.compressed_vertices_len = Some(compressed_vertices.len() as u32);
            primitive.compressed_vertices_data = Some(compressed_vertices);
            primitive.num_indices = Some(indices_len as u32);
            primitive.compressed_indices_len = Some(compressed_indices.len() as u32);
            primitive.compressed_indices_data = Some(compressed_indices);
        }

        // Check for animation frames - try OBJ first, then GLB
        let frame0_obj = animation_frame_file_name(gameitem_file_name, 0, PrimitiveMeshFormat::Obj);
        let frame0_glb = animation_frame_file_name(gameitem_file_name, 0, PrimitiveMeshFormat::Glb);
        let frame0_obj_path = gameitems_dir.join(&frame0_obj);
        let frame0_glb_path = gameitems_dir.join(&frame0_glb);

        let animation_format = if fs.exists(&frame0_obj_path) {
            Some(PrimitiveMeshFormat::Obj)
        } else if fs.exists(&frame0_glb_path) {
            Some(PrimitiveMeshFormat::Glb)
        } else {
            None
        };

        if let Some(format) = animation_format {
            let mut frame = 0;
            let mut frames = Vec::new();
            loop {
                let frame_file = animation_frame_file_name(gameitem_file_name, frame, format);
                let frame_path = gameitems_dir.join(&frame_file);
                if fs.exists(&frame_path) {
                    let animation_frame = read_mesh_as_frame(&frame_path, format, fs)?;
                    frames.push(animation_frame);
                    frame += 1;
                } else {
                    break;
                }
            }

            let mut compressed_lengths: Vec<u32> = Vec::with_capacity(frames.len());
            let mut compressed_animation_vertices: Vec<Vec<u8>> = Vec::with_capacity(frames.len());
            for animation_frame_vertices in frames {
                let mut buff = BytesMut::with_capacity(
                    animation_frame_vertices.len() * VertData::SERIALIZED_SIZE,
                );
                for vertex in animation_frame_vertices {
                    write_animation_vertex_data(&mut buff, &vertex);
                }
                let compressed_frame = primitive::compress_mesh_data(&buff)?;
                compressed_lengths.push(compressed_frame.len() as u32);
                compressed_animation_vertices.push(compressed_frame);
            }
            primitive.compressed_animation_vertices_len = Some(compressed_lengths);
            primitive.compressed_animation_vertices_data = Some(compressed_animation_vertices);
        }
    }
    Ok(item)
}

fn animation_frame_file_name(
    gameitem_file_name: &str,
    index: usize,
    mesh_format: PrimitiveMeshFormat,
) -> String {
    let extension = match mesh_format {
        PrimitiveMeshFormat::Obj => "obj",
        PrimitiveMeshFormat::Glb => "glb",
    };
    format!("{gameitem_file_name}_anim_{index}.{extension}")
}

#[instrument(skip(fs))]
fn read_obj(obj_path: &Path, fs: &dyn FileSystem) -> io::Result<ReadObjResult> {
    let obj_data = fs.read_file(obj_path)?;
    let mut reader = io::BufReader::new(io::Cursor::new(obj_data));
    read_obj_from_reader(&mut reader)
        .map_err(|e| io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e)))
}

fn read_glb_and_compress(
    glb_path: &Path,
    fs: &dyn FileSystem,
) -> io::Result<(usize, usize, Vec<u8>, Vec<u8>)> {
    // Read GLB file
    let (vertices, indices) = crate::vpx::gltf::read_glb(glb_path, fs)?;

    // Build BytesMut for vertices - just copy the 32-byte arrays
    let mut vpx_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for VertexWrapper {
        vpx_encoded_vertex, ..
    } in &vertices
    {
        vpx_vertices.put_slice(vpx_encoded_vertex);
    }

    // Build BytesMut for indices
    let bytes_per_index: u8 = if vertices.len() > MAX_VERTICES_FOR_2_BYTE_INDEX {
        4
    } else {
        2
    };
    let mut vpx_indices = BytesMut::with_capacity(indices.len() * bytes_per_index as usize);
    for idx in &indices {
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, idx.i0);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, idx.i1);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_indices, idx.i2);
    }

    let vertices_len = vertices.len();
    let indices_len = indices.len() * 3;
    let (compressed_vertices, compressed_indices) =
        compress_vertices_and_indices(&vpx_vertices, &vpx_indices)?;

    Ok((
        vertices_len,
        indices_len,
        compressed_vertices,
        compressed_indices,
    ))
}

#[instrument(skip(vpx_vertices, vpx_indices), fields(
    vertices_bytes = vpx_vertices.len(),
    indices_bytes = vpx_indices.len()
))]
fn compress_vertices_and_indices(
    vpx_vertices: &[u8],
    vpx_indices: &[u8],
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    #[cfg(feature = "parallel")]
    let (compressed_vertices, compressed_indices) = rayon::join(
        || primitive::compress_mesh_data(vpx_vertices),
        || primitive::compress_mesh_data(vpx_indices),
    );

    #[cfg(not(feature = "parallel"))]
    let (compressed_vertices, compressed_indices) = (
        primitive::compress_mesh_data(&vpx_vertices),
        primitive::compress_mesh_data(&vpx_indices),
    );

    let compressed_vertices = compressed_vertices?;
    let compressed_indices = compressed_indices?;
    Ok((compressed_vertices, compressed_indices))
}

fn vpx_encode_vertices(vertices_len: usize, indices: &[VpxFace]) -> BytesMut {
    let bytes_per_index: u8 = if vertices_len > MAX_VERTICES_FOR_2_BYTE_INDEX {
        4
    } else {
        2
    };
    let mut vpx_encoded_indices = BytesMut::with_capacity(indices.len() * bytes_per_index as usize);
    for face in indices {
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_encoded_indices, face.i0);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_encoded_indices, face.i1);
        write_vertex_index_for_vpx(bytes_per_index, &mut vpx_encoded_indices, face.i2);
    }
    vpx_encoded_indices
}

#[instrument(skip(fs))]
fn read_mesh_as_frame(
    mesh_path: &Path,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> io::Result<Vec<VertData>> {
    match mesh_format {
        PrimitiveMeshFormat::Obj => read_obj_as_frame(mesh_path, fs),
        PrimitiveMeshFormat::Glb => read_glb_as_frame(mesh_path, fs),
    }
}

fn read_obj_as_frame(obj_path: &Path, fs: &dyn FileSystem) -> io::Result<Vec<VertData>> {
    let obj_data = fs.read_file(obj_path)?;
    let mut reader = io::BufReader::new(io::Cursor::new(obj_data));
    let ObjData {
        name: _,
        vertices: obj_vertices,
        texture_coordinates: _,
        normals,
        indices: _,
    } = obj_read_obj(&mut reader).map_err(|e| {
        io::Error::other(format!("Error reading obj {}: {}", obj_path.display(), e))
    })?;
    let mut vertices: Vec<VertData> = Vec::with_capacity(obj_vertices.len());
    for (v, vn) in obj_vertices.iter().zip(normals.iter()) {
        let nx = vn.x;
        let ny = vn.y;
        let nz = -(vn.z);
        let vertext = VertData {
            x: v.0,
            y: v.1,
            z: -(v.2),
            nx,
            ny,
            nz,
        };
        vertices.push(vertext);
    }
    Ok(vertices)
}

fn read_glb_as_frame(glb_path: &Path, _fs: &dyn FileSystem) -> io::Result<Vec<VertData>> {
    let glb_data = _fs.read_file(glb_path)?;
    let mut cursor = io::Cursor::new(&glb_data);

    // Read GLB header
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != b"glTF" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid GLB magic",
        ));
    }

    cursor.set_position(cursor.position() + 8); // Skip version and length

    // Read JSON chunk
    let json_length = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.set_position(cursor.position() + 4); // Skip chunk type

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
    cursor.set_position(cursor.position() + 4); // Skip chunk type

    let bin_start = cursor.position() as usize;
    let bin_data = &glb_data[bin_start..bin_start + bin_length];

    // Parse GLTF structure
    let accessors = gltf_json["accessors"]
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing accessors"))?;
    let buffer_views = gltf_json["bufferViews"]
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing bufferViews"))?;

    // Read positions (accessor 0)
    let pos_accessor = &accessors[0];
    let pos_view_idx = pos_accessor["bufferView"].as_u64().unwrap() as usize;
    let pos_view = &buffer_views[pos_view_idx];
    let pos_offset = pos_view["byteOffset"].as_u64().unwrap() as usize;
    let pos_count = pos_accessor["count"].as_u64().unwrap() as usize;

    // Read normals (accessor 1)
    let norm_view_idx = accessors[1]["bufferView"].as_u64().unwrap() as usize;
    let norm_view = &buffer_views[norm_view_idx];
    let norm_offset = norm_view["byteOffset"].as_u64().unwrap() as usize;

    // Build VertData
    let mut vertices: Vec<VertData> = Vec::with_capacity(pos_count);

    for i in 0..pos_count {
        let mut pos_cursor = io::Cursor::new(&bin_data[pos_offset + i * 12..]);
        let x = pos_cursor.read_f32::<LittleEndian>()?;
        let y = pos_cursor.read_f32::<LittleEndian>()?;
        let z = pos_cursor.read_f32::<LittleEndian>()?;

        let mut norm_cursor = io::Cursor::new(&bin_data[norm_offset + i * 12..]);
        let nx = norm_cursor.read_f32::<LittleEndian>()?;
        let ny = norm_cursor.read_f32::<LittleEndian>()?;
        let nz = norm_cursor.read_f32::<LittleEndian>()?;

        vertices.push(VertData {
            x,
            y,
            z,
            nx,
            ny,
            nz,
        });
    }

    Ok(vertices)
}
