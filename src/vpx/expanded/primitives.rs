//! Primitive mesh reading and writing for expanded VPX format

use super::{
    GAMEITEMS_DIR, Output, PrimitiveMeshFormat, WriteError, generated_mesh_file_name,
    mesh_file_extension,
};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive;
use crate::vpx::gameitem::primitive::{
    MAX_VERTICES_FOR_2_BYTE_INDEX, ReadMesh, VertData, VertexWrapper, read_vpx_animation_frame,
    write_animation_vertex_data,
};
use crate::vpx::gltf::{GltfContainer, read_gltf};
use crate::vpx::model::Vertex3dNoTex2;
use crate::vpx::obj::{
    ObjData, ReadObjResult, VpxFace, read_obj as obj_read_obj, read_obj_from_reader,
    write_vertex_index_for_vpx,
};
use log::warn;

use crate::vpx::TableDimensions;
use crate::vpx::gameitem::bumper::Bumper;
use crate::vpx::gameitem::flasher::Flasher;
use crate::vpx::gameitem::flipper::Flipper;
use crate::vpx::gameitem::gate::Gate;
use crate::vpx::gameitem::hittarget::HitTarget;
use crate::vpx::gameitem::light::Light;
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
use crate::vpx::mesh::lights::build_light_meshes;
use crate::vpx::mesh::plungers::build_plunger_meshes;
use crate::vpx::mesh::ramps::build_ramp_mesh;
use crate::vpx::mesh::rubbers::build_rubber_mesh;
use crate::vpx::mesh::spinners::build_spinner_meshes;
use crate::vpx::mesh::triggers::build_trigger_mesh;
use crate::vpx::mesh::walls::build_wall_mesh;
use bytes::{BufMut, BytesMut};
use std::io;
use std::iter::Zip;
use std::path::{Path, PathBuf};
use std::slice::Iter;
use tracing::instrument;

struct MeshReadResult {
    vertices_len: usize,
    indices_len: usize,
    /// The vertices and indices as the vpx stores them, uncompressed
    vertices: Vec<u8>,
    indices: Vec<u8>,
    compressed_vertices: Vec<u8>,
    compressed_indices: Vec<u8>,
}

pub(super) fn write_gameitem_binaries(
    gameitem: &GameItemEnum,
    json_file_name: &str,
    table_dims: &TableDimensions,
    out: &Output,
) -> Result<(), WriteError> {
    let options = out.options();
    let mesh_format = options.get_mesh_format();
    if let GameItemEnum::Primitive(primitive) = gameitem {
        let mesh_path = Path::new(GAMEITEMS_DIR).join(format!(
            "{json_file_name}.{}",
            mesh_file_extension(mesh_format)
        ));
        // checked before the mesh is decompressed so a filtered out mesh costs nothing
        if out.wants(&mesh_path)
            && let Some(ReadMesh { vertices, indices }) = &primitive.read_mesh()?
        {
            out.write_mesh(&mesh_path, gameitem.name(), vertices, indices)?;

            if let Some(animation_frames) = &primitive.compressed_animation_vertices_data {
                if let Some(compressed_lengths) = &primitive.compressed_animation_vertices_len {
                    let zipped = animation_frames.iter().zip(compressed_lengths.iter());
                    write_animation_frames_to_meshes(
                        out,
                        gameitem.name(),
                        json_file_name,
                        vertices,
                        indices,
                        zipped,
                        mesh_format,
                    )?;
                } else {
                    return Err(WriteError::Io(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "Animation frames should always come with counts: {json_file_name}"
                        ),
                    )));
                }
            }
        }
    }
    // Generate derived meshes for walls, ramps, rubbers, and flashers (optional)
    if options.should_generate_derived_meshes() {
        match gameitem {
            GameItemEnum::Wall(wall) => {
                write_wall_meshes(out, wall, json_file_name, mesh_format)?;
            }
            GameItemEnum::Ramp(ramp) => {
                write_ramp_meshes(out, ramp, json_file_name, mesh_format, table_dims)?;
            }
            GameItemEnum::Rubber(rubber) => {
                write_rubber_meshes(out, rubber, json_file_name, mesh_format)?;
            }
            GameItemEnum::Flasher(flasher) => {
                write_flasher_meshes(out, flasher, json_file_name, mesh_format, table_dims)?;
            }
            GameItemEnum::Flipper(flipper) => {
                write_flipper_meshes(out, flipper, json_file_name, mesh_format)?;
            }
            GameItemEnum::Spinner(spinner) => {
                write_spinner_meshes(out, spinner, json_file_name, mesh_format)?;
            }
            GameItemEnum::Bumper(bumper) => {
                write_bumper_meshes(out, bumper, json_file_name, mesh_format)?;
            }
            GameItemEnum::HitTarget(hit_target) => {
                write_hit_target_meshes(out, hit_target, json_file_name, mesh_format)?;
            }
            GameItemEnum::Gate(gate) => {
                write_gate_meshes(out, gate, json_file_name, mesh_format)?;
            }
            GameItemEnum::Trigger(trigger) => {
                write_trigger_mesh(out, trigger, json_file_name, mesh_format)?;
            }
            GameItemEnum::Plunger(plunger) => {
                write_plunger_meshes(out, plunger, json_file_name, mesh_format)?;
            }
            GameItemEnum::Light(light) => {
                write_light_meshes(out, light, json_file_name, mesh_format)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The mesh files one item's derived geometry can produce, named once:
/// the writer asks whether any of them is wanted before building the
/// geometry, and writes through the same list, so the two cannot drift
struct DerivedMeshFiles<'a> {
    out: &'a Output<'a>,
    files: Vec<(&'static str, PathBuf)>,
}

impl<'a> DerivedMeshFiles<'a> {
    /// Files named `<stem>-generated.<ext>`, or `<stem>-<part>.json-generated.<ext>`
    /// for the named parts of a multi part item
    fn generated(
        out: &'a Output<'a>,
        json_file_name: &str,
        parts: &[&'static str],
        mesh_format: PrimitiveMeshFormat,
    ) -> Self {
        let base = json_file_name.trim_end_matches(".json");
        let files = parts
            .iter()
            .map(|part| {
                let name = if part.is_empty() {
                    json_file_name.to_string()
                } else {
                    format!("{base}-{part}.json")
                };
                let path =
                    Path::new(GAMEITEMS_DIR).join(generated_mesh_file_name(&name, mesh_format));
                (*part, path)
            })
            .collect();
        Self { out, files }
    }

    /// Files named `<stem>-<part>.<ext>`, the light mesh convention
    fn plain(
        out: &'a Output<'a>,
        json_file_name: &str,
        parts: &[&'static str],
        mesh_format: PrimitiveMeshFormat,
    ) -> Self {
        let base = json_file_name.trim_end_matches(".json");
        let extension = mesh_file_extension(mesh_format);
        let files = parts
            .iter()
            .map(|part| {
                let path = Path::new(GAMEITEMS_DIR).join(format!("{base}-{part}.{extension}"));
                (*part, path)
            })
            .collect();
        Self { out, files }
    }

    /// Whether any of the files is wanted; asked before the geometry is
    /// built so filtered out meshes cost nothing
    fn wanted(&self) -> bool {
        self.files.iter().any(|(_, path)| self.out.wants(path))
    }

    /// Writes one part, `""` for the single mesh of an item
    fn write(
        &self,
        part: &str,
        name: &str,
        vertices: &[VertexWrapper],
        indices: &[VpxFace],
    ) -> Result<(), WriteError> {
        let Some((_, path)) = self.files.iter().find(|(known, _)| *known == part) else {
            return Err(WriteError::Io(io::Error::other(format!(
                "derived mesh part {part:?} was not named up front"
            ))));
        };
        self.out.write_mesh(path, name, vertices, indices)
    }
}

fn write_gate_meshes(
    out: &Output,
    gate: &Gate,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &["bracket", "wire"], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some(gate_meshes) = build_gate_meshes(gate) else {
        return Ok(());
    };
    if let Some((vertices, indices)) = gate_meshes.bracket {
        files.write(
            "bracket",
            &format!("{}Bracket", gate.name),
            &vertices,
            &indices,
        )?;
    }
    let (vertices, indices) = gate_meshes.wire;
    files.write("wire", &format!("{}Wire", gate.name), &vertices, &indices)
}

fn write_bumper_meshes(
    out: &Output,
    bumper: &Bumper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(
        out,
        json_file_name,
        &["base", "socket", "ring", "cap"],
        mesh_format,
    );
    if !files.wanted() {
        return Ok(());
    }
    let meshes = build_bumper_meshes(bumper);
    let parts = [
        ("base", "Base", meshes.base),
        ("socket", "Socket", meshes.socket),
        ("ring", "Ring", meshes.ring),
        ("cap", "Cap", meshes.cap),
    ];
    for (part, suffix, mesh) in parts {
        if let Some((vertices, indices)) = mesh {
            files.write(
                part,
                &format!("{}{suffix}", bumper.name),
                &vertices,
                &indices,
            )?;
        }
    }
    Ok(())
}

fn write_flipper_meshes(
    out: &Output,
    flipper: &Flipper,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices)) = build_flipper_mesh(flipper, 0.0) else {
        return Ok(());
    };
    files.write("", &flipper.name, &vertices, &indices)
}

fn write_hit_target_meshes(
    out: &Output,
    hit_target: &HitTarget,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices)) = build_hit_target_mesh(hit_target) else {
        return Ok(());
    };
    files.write("", &hit_target.name, &vertices, &indices)
}

fn write_plunger_meshes(
    out: &Output,
    plunger: &Plunger,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(
        out,
        json_file_name,
        &["flat", "rod", "spring", "ring", "tip"],
        mesh_format,
    );
    if !files.wanted() {
        return Ok(());
    }
    let meshes = build_plunger_meshes(plunger);
    let parts = [
        ("flat", "Flat", meshes.flat_rod),
        ("rod", "Rod", meshes.rod),
        ("spring", "Spring", meshes.spring),
        ("ring", "Ring", meshes.ring),
        ("tip", "Tip", meshes.tip),
    ];
    for (part, suffix, mesh) in parts {
        if let Some((vertices, indices)) = mesh {
            files.write(
                part,
                &format!("{}{suffix}", plunger.name),
                &vertices,
                &indices,
            )?;
        }
    }
    Ok(())
}

fn write_spinner_meshes(
    out: &Output,
    spinner: &Spinner,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files =
        DerivedMeshFiles::generated(out, json_file_name, &["bracket", "plate"], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    // TODO: get surface height from the table
    let meshes = build_spinner_meshes(spinner);
    if let Some((vertices, indices)) = meshes.bracket {
        files.write(
            "bracket",
            &format!("{}Bracket", spinner.name),
            &vertices,
            &indices,
        )?;
    }
    let (vertices, indices) = meshes.plate;
    files.write(
        "plate",
        &format!("{}Plate", spinner.name),
        &vertices,
        &indices,
    )
}

fn write_trigger_mesh(
    out: &Output,
    trigger: &Trigger,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices)) = build_trigger_mesh(trigger) else {
        return Ok(());
    };
    files.write("", &trigger.name, &vertices, &indices)
}

fn write_ramp_meshes(
    out: &Output,
    ramp: &Ramp,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    table_dims: &TableDimensions,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    // The expanded format is a portable representation - it doesn't
    // have access to the table's `user_detail_level` or the ramp's
    // material here, so we use the editor default for detail level
    // and assume the material is opaque (vpinball's dummy-material
    // default; the typical case for ramps).
    let Some((vertices, indices)) = build_ramp_mesh(
        ramp,
        table_dims,
        crate::vpx::gamedata::DEFAULT_DETAIL_LEVEL,
        false, // material_opacity_active
    ) else {
        return Ok(());
    };
    files.write("", &ramp.name, &vertices, &indices)
}

fn write_rubber_meshes(
    out: &Output,
    rubber: &Rubber,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices, _center)) =
        build_rubber_mesh(rubber, crate::vpx::gamedata::DEFAULT_DETAIL_LEVEL)
    else {
        return Ok(());
    };
    files.write("", &rubber.name, &vertices, &indices)
}

fn write_wall_meshes(
    out: &Output,
    wall: &Wall,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices)) = build_wall_mesh(wall) else {
        return Ok(());
    };
    files.write("", &wall.name, &vertices, &indices)
}

fn write_flasher_meshes(
    out: &Output,
    flasher: &Flasher,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
    table_dims: &TableDimensions,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::generated(out, json_file_name, &[""], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some((vertices, indices, _center)) = build_flasher_mesh(flasher, table_dims) else {
        return Ok(());
    };
    files.write("", &flasher.name, &vertices, &indices)
}

fn write_light_meshes(
    out: &Output,
    light: &Light,
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    let files = DerivedMeshFiles::plain(out, json_file_name, &["bulb", "socket"], mesh_format);
    if !files.wanted() {
        return Ok(());
    }
    let Some(meshes) = build_light_meshes(light) else {
        return Ok(());
    };
    let base = json_file_name.trim_end_matches(".json");
    for (part, mesh) in [("bulb", meshes.bulb), ("socket", meshes.socket)] {
        if let Some((vertices, indices)) = mesh {
            files.write(part, &format!("{base}-{part}"), &vertices, &indices)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_animation_frames_to_meshes(
    out: &Output,
    name: &str,
    json_file_name: &str,
    vertices: &[VertexWrapper],
    vpx_indices: &[VpxFace],
    zipped: Zip<Iter<Vec<u8>>, Iter<u32>>,
    mesh_format: PrimitiveMeshFormat,
) -> Result<(), WriteError> {
    for (i, (compressed_frame, compressed_length)) in zipped.enumerate() {
        let file_name_without_ext = json_file_name.trim_end_matches(".json");
        let file_name = animation_frame_file_name(file_name_without_ext, i, mesh_format);
        let mesh_path = Path::new(GAMEITEMS_DIR).join(&file_name);
        // checked before the frame is decompressed so a filtered out frame costs nothing
        if !out.wants(&mesh_path) {
            continue;
        }
        let animation_frame_vertices =
            read_vpx_animation_frame(compressed_frame, compressed_length);
        let full_vertices = replace_vertices(vertices, animation_frame_vertices)?;
        out.write_mesh(&mesh_path, name, &full_vertices, vpx_indices)?;
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
            // a primitive that declares a mesh must have a sidecar file,
            // otherwise the mesh would silently be dropped
            if primitive.use_3d_mesh {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "Primitive {:?} uses a 3D mesh but no mesh file was found, expected {}",
                        primitive.name,
                        obj_path.display()
                    ),
                ));
            }
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
            // the mesh file is authoritative, but warn when the json claimed a
            // different count so a stale hand edit does not pass silently, the
            // same way stale image dimensions are reported
            let vertices_from_file = result.vertices_len as u32;
            let indices_from_file = result.indices_len as u32;
            if let Some(json_vertices) = primitive.num_vertices
                && json_vertices != vertices_from_file
            {
                warn!(
                    "Stale vertex count for primitive {:?} in json {json_vertices} vs in mesh {vertices_from_file}",
                    primitive.name
                );
            }
            if let Some(json_indices) = primitive.num_indices
                && json_indices != indices_from_file
            {
                warn!(
                    "Stale index count for primitive {:?} in json {json_indices} vs in mesh {indices_from_file}",
                    primitive.name
                );
            }
            primitive.num_vertices = Some(vertices_from_file);
            primitive.num_indices = Some(indices_from_file);
            if primitive.vertices_data.is_some() {
                // the json marked a mesh the file stored uncompressed
                primitive.vertices_data = Some(result.vertices);
                primitive.indices_data = Some(result.indices);
            } else {
                primitive.compressed_vertices_len = Some(result.compressed_vertices.len() as u32);
                primitive.compressed_vertices_data = Some(result.compressed_vertices);
                primitive.compressed_indices_len = Some(result.compressed_indices.len() as u32);
                primitive.compressed_indices_data = Some(result.compressed_indices);
            }
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
        vertices: read_result.vpx_encoded_vertices.to_vec(),
        indices: vpx_encoded_indices.to_vec(),
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
        vertices: vpx_vertices.to_vec(),
        indices: vpx_indices.to_vec(),
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
