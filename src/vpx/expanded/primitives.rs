//! Primitive mesh reading and writing for expanded VPX format

use super::{
    ExpandOptions, PrimitiveMeshFormat, WriteError, generated_mesh_file_name, write_mesh_to_file,
};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive;
use crate::vpx::gameitem::primitive::{
    MAX_VERTICES_FOR_2_BYTE_INDEX, ReadMesh, VertData, VertexWrapper, read_vpx_animation_frame,
    write_animation_vertex_data,
};
use crate::vpx::gltf::{GltfContainer, read_gltf, write_gltf};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::{
    ObjData, ReadObjResult, VpxFace, read_obj as obj_read_obj, read_obj_from_reader, write_obj,
    write_vertex_index_for_vpx,
};

use crate::vpx::TableDimensions;
use crate::vpx::gameitem::bumper::Bumper;
use crate::vpx::gameitem::flasher::Flasher;
use crate::vpx::gameitem::flipper::Flipper;
use crate::vpx::gameitem::gate::Gate;
use crate::vpx::gameitem::hittarget::HitTarget;
use crate::vpx::gameitem::plunger::Plunger;
use crate::vpx::gameitem::ramp::Ramp;
use crate::vpx::gameitem::rubber::Rubber;
use crate::vpx::gameitem::spinner::Spinner;
use crate::vpx::gameitem::trigger::Trigger;
use crate::vpx::gameitem::wall::Wall;
use crate::vpx::mesh::bumpers::build_bumper_meshes;
use crate::vpx::mesh::flashers::build_flasher_mesh;
use crate::vpx::mesh::flippers::build_flipper_mesh;
use crate::vpx::mesh::gates::build_gate_meshes;
use crate::vpx::mesh::hittargets::build_hit_target_mesh;
use crate::vpx::mesh::lights::write_light_meshes;
use crate::vpx::mesh::plungers::build_plunger_meshes;
use crate::vpx::mesh::ramps::build_ramp_mesh;
use crate::vpx::mesh::rubbers::build_rubber_mesh;
use crate::vpx::mesh::spinners::build_spinner_meshes;
use crate::vpx::mesh::triggers::build_trigger_mesh;
use crate::vpx::mesh::walls::build_wall_mesh;
use bytes::{BufMut, BytesMut};
use std::io;
use std::iter::Zip;
use std::path::Path;
use std::slice::Iter;
use tracing::instrument;

struct MeshReadResult {
    vertices_len: usize,
    indices_len: usize,
    compressed_vertices: Vec<u8>,
    compressed_indices: Vec<u8>,
}

pub(super) fn write_gameitem_binaries(
    gameitems_dir: &Path,
    gameitem: &GameItemEnum,
    json_file_name: &str,
    options: &ExpandOptions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let mesh_format = options.get_mesh_format();
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
                let name = gameitem.name();
                write_gltf(name, vertices, indices, &glb_path, GltfContainer::Glb, fs)
                    .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            PrimitiveMeshFormat::Gltf => {
                let gltf_path = gameitems_dir.join(format!("{json_file_name}.gltf"));
                write_gltf(
                    gameitem.name(),
                    vertices,
                    indices,
                    &gltf_path,
                    GltfContainer::Gltf,
                    fs,
                )
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
    // Generate derived meshes for walls, ramps, rubbers, and flashers (optional)
    if options.should_generate_derived_meshes() {
        // TODO: Pass actual table dimensions for correct world-aligned textures
        // For now, use a default that works for most tables (standard playfield size)
        let default_table_dims = TableDimensions::new(0.0, 0.0, 952.0, 2162.0);

        match gameitem {
            GameItemEnum::Wall(wall) => {
                write_wall_meshes(gameitems_dir, wall, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Ramp(ramp) => {
                write_ramp_meshes(
                    gameitems_dir,
                    ramp,
                    json_file_name,
                    mesh_format,
                    &default_table_dims,
                    fs,
                )?;
            }
            GameItemEnum::Rubber(rubber) => {
                write_rubber_meshes(gameitems_dir, rubber, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Flasher(flasher) => {
                write_flasher_meshes(
                    gameitems_dir,
                    flasher,
                    json_file_name,
                    mesh_format,
                    &default_table_dims,
                    fs,
                )?;
            }
            GameItemEnum::Flipper(flipper) => {
                write_flipper_meshes(gameitems_dir, flipper, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Spinner(spinner) => {
                write_spinner_meshes(gameitems_dir, spinner, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Bumper(bumper) => {
                write_bumper_meshes(gameitems_dir, bumper, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::HitTarget(hit_target) => {
                write_hit_target_meshes(
                    gameitems_dir,
                    hit_target,
                    json_file_name,
                    mesh_format,
                    fs,
                )?;
            }
            GameItemEnum::Gate(gate) => {
                write_gate_meshes(gameitems_dir, gate, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Trigger(trigger) => {
                write_trigger_mesh(gameitems_dir, trigger, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Plunger(plunger) => {
                write_plunger_meshes(gameitems_dir, plunger, json_file_name, mesh_format, fs)?;
            }
            GameItemEnum::Light(light) => {
                write_light_meshes(gameitems_dir, light, json_file_name, mesh_format, fs)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn write_gate_meshes(
    gameitems_dir: &Path,
    gate: &Gate,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some(gate_meshes) = build_gate_meshes(gate) else {
        return Ok(());
    };

    let file_name_base = json_file_name.trim_end_matches(".json");

    // Write bracket mesh if visible
    if let Some((vertices, indices)) = gate_meshes.bracket {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-bracket.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Bracket", gate.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write wire/plate mesh
    let (vertices, indices) = gate_meshes.wire;
    let mesh_path = gameitems_dir.join(generated_mesh_file_name(
        &format!("{file_name_base}-wire.json"),
        mesh_format,
    ));
    write_mesh_to_file(
        &mesh_path,
        &format!("{}Wire", gate.name),
        &vertices,
        &indices,
        mesh_format,
        fs,
    )?;

    Ok(())
}

fn write_bumper_meshes(
    gameitems_dir: &Path,
    bumper: &Bumper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let bumper_meshes = build_bumper_meshes(bumper);
    let file_name_base = json_file_name.trim_end_matches(".json");

    // Write base mesh
    if let Some((vertices, indices)) = bumper_meshes.base {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-base.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Base", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write socket mesh
    if let Some((vertices, indices)) = bumper_meshes.socket {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-socket.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Socket", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write ring mesh
    if let Some((vertices, indices)) = bumper_meshes.ring {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-ring.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Ring", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write cap mesh
    if let Some((vertices, indices)) = bumper_meshes.cap {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-cap.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Cap", bumper.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    Ok(())
}
fn write_flipper_meshes(
    gameitems_dir: &Path,
    flipper: &Flipper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_flipper_mesh(flipper, 0.0) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &flipper.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}
fn write_hit_target_meshes(
    gameitems_dir: &Path,
    hit_target: &HitTarget,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_hit_target_mesh(hit_target) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &hit_target.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}

fn write_plunger_meshes(
    gameitems_dir: &Path,
    plunger: &Plunger,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let plunger_meshes = build_plunger_meshes(plunger);
    let file_name_base = json_file_name.trim_end_matches(".json");

    // Write flat rod mesh
    if let Some((vertices, indices)) = plunger_meshes.flat_rod {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-flat.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Flat", plunger.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write rod mesh
    if let Some((vertices, indices)) = plunger_meshes.rod {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-rod.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Rod", plunger.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write spring mesh
    if let Some((vertices, indices)) = plunger_meshes.spring {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-spring.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Spring", plunger.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write ring mesh
    if let Some((vertices, indices)) = plunger_meshes.ring {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-ring.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Ring", plunger.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    // Write tip mesh
    if let Some((vertices, indices)) = plunger_meshes.tip {
        let mesh_path = gameitems_dir.join(generated_mesh_file_name(
            &format!("{file_name_base}-tip.json"),
            mesh_format,
        ));
        write_mesh_to_file(
            &mesh_path,
            &format!("{}Tip", plunger.name),
            &vertices,
            &indices,
            mesh_format,
            fs,
        )?;
    }

    Ok(())
}

fn write_spinner_meshes(
    gameitems_dir: &Path,
    spinner: &Spinner,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    // TODO: get surface height from the table
    let meshes = build_spinner_meshes(spinner);

    // Write bracket mesh if present
    if let Some((bracket_vertices, bracket_indices)) = meshes.bracket {
        let bracket_mesh_name = format!("{}-bracket", json_file_name.trim_end_matches(".json"));
        let bracket_mesh_path =
            gameitems_dir.join(generated_mesh_file_name(&bracket_mesh_name, mesh_format));
        write_mesh_to_file(
            &bracket_mesh_path,
            &format!("{}Bracket", spinner.name),
            &bracket_vertices,
            &bracket_indices,
            mesh_format,
            fs,
        )?;
    }

    // Write plate mesh
    let (plate_vertices, plate_indices) = meshes.plate;
    let plate_mesh_name = format!("{}-plate", json_file_name.trim_end_matches(".json"));
    let plate_mesh_path =
        gameitems_dir.join(generated_mesh_file_name(&plate_mesh_name, mesh_format));
    write_mesh_to_file(
        &plate_mesh_path,
        &format!("{}Plate", spinner.name),
        &plate_vertices,
        &plate_indices,
        mesh_format,
        fs,
    )
}

fn write_trigger_mesh(
    gameitems_dir: &Path,
    trigger: &Trigger,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_trigger_mesh(trigger) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &trigger.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )?;

    Ok(())
}

fn write_ramp_meshes(
    gameitems_dir: &Path,
    ramp: &Ramp,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    table_dims: &TableDimensions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_ramp_mesh(ramp, table_dims) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(&mesh_path, &ramp.name, &vertices, &indices, mesh_format, fs)
}

fn write_rubber_meshes(
    gameitems_dir: &Path,
    rubber: &Rubber,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices, _center)) = build_rubber_mesh(rubber) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &rubber.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
}

fn write_wall_meshes(
    gameitems_dir: &Path,
    wall: &Wall,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices)) = build_wall_mesh(wall) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(&mesh_path, &wall.name, &vertices, &indices, mesh_format, fs)
}

fn write_flasher_meshes(
    gameitems_dir: &Path,
    flasher: &Flasher,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    table_dims: &TableDimensions,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    let Some((vertices, indices, _center)) = build_flasher_mesh(flasher, table_dims) else {
        return Ok(());
    };

    let mesh_path = gameitems_dir.join(generated_mesh_file_name(json_file_name, mesh_format));
    write_mesh_to_file(
        &mesh_path,
        &flasher.name,
        &vertices,
        &indices,
        mesh_format,
        fs,
    )
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
                write_gltf(
                    name,
                    &full_vertices,
                    vpx_indices,
                    &mesh_path,
                    GltfContainer::Glb,
                    fs,
                )
                .map_err(|e| WriteError::Io(io::Error::other(format!("{e}"))))?;
            }
            PrimitiveMeshFormat::Gltf => {
                write_gltf(
                    name,
                    &full_vertices,
                    vpx_indices,
                    &mesh_path,
                    GltfContainer::Gltf,
                    fs,
                )
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
        let gltf_path = gameitems_dir.join(format!("{gameitem_file_name}.gltf"));

        let mesh_format = if fs.exists(&obj_path) {
            Some(PrimitiveMeshFormat::Obj)
        } else if fs.exists(&glb_path) {
            Some(PrimitiveMeshFormat::Glb)
        } else if fs.exists(&gltf_path) {
            Some(PrimitiveMeshFormat::Gltf)
        } else {
            None
        };

        if let Some(format) = mesh_format {
            let result = match format {
                PrimitiveMeshFormat::Obj => read_obj_and_compress(fs, &obj_path)?,
                PrimitiveMeshFormat::Glb => {
                    read_gltf_and_compress(&glb_path, fs, GltfContainer::Glb)?
                }
                PrimitiveMeshFormat::Gltf => {
                    read_gltf_and_compress(&gltf_path, fs, GltfContainer::Gltf)?
                }
            };
            primitive.num_vertices = Some(result.vertices_len as u32);
            primitive.compressed_vertices_len = Some(result.compressed_vertices.len() as u32);
            primitive.compressed_vertices_data = Some(result.compressed_vertices);
            primitive.num_indices = Some(result.indices_len as u32);
            primitive.compressed_indices_len = Some(result.compressed_indices.len() as u32);
            primitive.compressed_indices_data = Some(result.compressed_indices);
        }

        // Check for animation frames - try OBJ first, then GLB
        let frame0_obj = animation_frame_file_name(gameitem_file_name, 0, PrimitiveMeshFormat::Obj);
        let frame0_glb = animation_frame_file_name(gameitem_file_name, 0, PrimitiveMeshFormat::Glb);
        let frame0_gltf =
            animation_frame_file_name(gameitem_file_name, 0, PrimitiveMeshFormat::Gltf);
        let frame0_obj_path = gameitems_dir.join(&frame0_obj);
        let frame0_glb_path = gameitems_dir.join(&frame0_glb);
        let frame0_gltf_path = gameitems_dir.join(&frame0_gltf);

        let animation_format = if fs.exists(&frame0_obj_path) {
            Some(PrimitiveMeshFormat::Obj)
        } else if fs.exists(&frame0_glb_path) {
            Some(PrimitiveMeshFormat::Glb)
        } else if fs.exists(&frame0_gltf_path) {
            Some(PrimitiveMeshFormat::Gltf)
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
        PrimitiveMeshFormat::Gltf => "gltf",
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

fn read_obj_and_compress(fs: &dyn FileSystem, obj_path: &Path) -> io::Result<MeshReadResult> {
    let read_result = read_obj(obj_path, fs)?;
    let vertices_len = read_result.vertices.len();
    let indices_len = read_result.indices.len() * 3;

    let vpx_encoded_indices = vpx_encode_vertices(read_result.vertices.len(), &read_result.indices);

    let (compressed_vertices, compressed_indices) =
        compress_vertices_and_indices(&read_result.vpx_encoded_vertices, &vpx_encoded_indices)?;

    Ok(MeshReadResult {
        vertices_len,
        indices_len,
        compressed_vertices,
        compressed_indices,
    })
}

fn read_gltf_and_compress(
    gltf_path: &Path,
    fs: &dyn FileSystem,
    container: GltfContainer,
) -> io::Result<MeshReadResult> {
    let (vertices, indices) = read_gltf(gltf_path, container, fs)?;

    let mut vpx_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for VertexWrapper {
        vpx_encoded_vertex, ..
    } in &vertices
    {
        vpx_vertices.put_slice(vpx_encoded_vertex);
    }

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

    Ok(MeshReadResult {
        vertices_len,
        indices_len,
        compressed_vertices,
        compressed_indices,
    })
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
        PrimitiveMeshFormat::Glb => read_gltf_as_frame(mesh_path, GltfContainer::Glb, fs),
        PrimitiveMeshFormat::Gltf => read_gltf_as_frame(mesh_path, GltfContainer::Gltf, fs),
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

fn read_gltf_as_frame(
    gltf_path: &Path,
    container: GltfContainer,
    fs: &dyn FileSystem,
) -> io::Result<Vec<VertData>> {
    let (vertices, _) = read_gltf(gltf_path, container, fs)?;
    let mut frames = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        frames.push(VertData {
            x: vertex.vertex.x,
            y: vertex.vertex.y,
            z: vertex.vertex.z,
            nx: vertex.vertex.nx,
            ny: vertex.vertex.ny,
            nz: vertex.vertex.nz,
        });
    }
    Ok(frames)
}
