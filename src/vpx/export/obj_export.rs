//! Wavefront OBJ export of an entire VPX table.
//!
//! Mirrors VPinball's `File -> Export -> OBJ Mesh`: produces a single `.obj`
//! file with one `o` block per item (playfield + every visible game item),
//! a single `.mtl` file with one `newmtl` block per unique
//! `(material, texture)` pair encountered, and an `images/` sibling folder
//! containing the texture files referenced from the `.mtl`.
//!
//! Output layout, given `path/to/<stem>.obj`:
//!
//! ```text
//! path/to/
//! +-- <stem>.obj
//! +-- <stem>.mtl
//! +-- images/
//!     +-- <texture-name>.<ext>
//! ```
//!
//! Coordinate convention matches VPinball's `ObjLoader`:
//!
//! - `obj_x = vpx_x + tx`, `obj_y = vpx_y + ty`, `obj_z = -(vpx_z + tz)`
//! - `obj_v = 1 - vpx_tv`
//! - normals' Z is negated (`obj_nz = -vpx_nz`)
//! - triangle winding is reversed (`(i0, i1, i2)` is emitted as `f i2 i1 i0`)
//! - NaN normals/UVs are written as 0
//!
//! The walk only includes the items that VPinball's `Item::ExportMesh`
//! implementations cover (primitive, wall, ramp, rubber, bumper, flipper,
//! gate, kicker, spinner, hittarget, trigger). Lights, decals, plungers,
//! flashers and reels are skipped, matching VPinball.

use crate::filesystem::FileSystem;
use crate::vpx::TableDimensions;
use crate::vpx::VPX;
use crate::vpx::color::Color;
use crate::vpx::expanded::util::sanitize_filename;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::gameitem::primitive::{Primitive, VertexWrapper};
use crate::vpx::image::ImageData;
use crate::vpx::material::MaterialType;
use crate::vpx::math::{Matrix3D, Vec3, Vertex3D};
use crate::vpx::mesh::bumpers::build_bumper_meshes;
use crate::vpx::mesh::flippers::build_flipper_meshes_unchecked;
use crate::vpx::mesh::gates::build_gate_meshes_unchecked;
use crate::vpx::mesh::hittargets::build_hit_target_mesh_unchecked;
use crate::vpx::mesh::kickers::build_kicker_meshes;
use crate::vpx::mesh::playfields::build_playfield_mesh;
use crate::vpx::mesh::ramps::build_ramp_mesh;
use crate::vpx::mesh::rubbers::build_rubber_mesh;
use crate::vpx::mesh::spinners::build_spinner_meshes;
use crate::vpx::mesh::triggers::build_trigger_mesh;
use crate::vpx::mesh::walls::build_wall_meshes;
use crate::vpx::obj::VpxFace;
pub use crate::vpx::units::ExportUnits;
use log::{info, warn};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use wavefront_obj_io::{IoMtlWriter, IoObjWriter, MapKind, MtlWriter, ObjWriter, SmoothingGroup};

/// Options controlling OBJ export behaviour. Defaults match VPinball's
/// own `File -> Export -> OBJ Mesh` byte for byte.
#[derive(Debug, Clone, Default)]
pub struct ObjExportOptions {
    /// Deduplicate `newmtl` blocks in the MTL file by `(material,
    /// texture)` pair.
    ///
    /// - **`false` (default)**: emit one `newmtl` block per `usemtl`,
    ///   matching VPinball's output exactly. The MTL contains one entry
    ///   per item-block, which can mean many duplicates for tables that
    ///   share materials across items.
    /// - **`true`**: only the first occurrence of each `(material,
    ///   texture)` pair gets a `newmtl` block. The `usemtl` references
    ///   in the OBJ still resolve correctly (they all point at the same
    ///   sanitized name). Produces a smaller MTL but diverges from
    ///   VPinball's reference output.
    pub dedup_mtl_blocks: bool,

    /// Output unit for vertex positions. Default is [`ExportUnits::Vpu`]
    /// (no scaling) for vpinball parity. Use [`ExportUnits::Mm`] or
    /// [`ExportUnits::M`] when loading the result into a DCC tool.
    pub units: ExportUnits,
}

/// Export the entire VPX table as a Wavefront OBJ + companion MTL + images
/// folder.
///
/// Writes `<obj_path>` (the OBJ), `<obj_path>.with_extension("mtl")` (the
/// material library), and `<obj_path-parent>/images/<image>.<ext>` for every
/// texture referenced by the exported items.
///
/// See [`ObjExportOptions`] for what's tunable. `&ObjExportOptions::default()`
/// produces VPinball-faithful output.
///
/// # Example
/// ```no_run
/// use std::path::Path;
/// use vpin::filesystem::RealFileSystem;
/// use vpin::vpx;
/// use vpin::vpx::export::obj_export::{export_obj, ObjExportOptions};
///
/// let vpx = vpx::read(Path::new("table.vpx")).unwrap();
/// export_obj(
///     &vpx,
///     Path::new("table_export/table.obj"),
///     &RealFileSystem,
///     &ObjExportOptions::default(),
/// ).unwrap();
/// ```
pub fn export_obj(
    vpx: &VPX,
    obj_path: &Path,
    fs: &dyn FileSystem,
    options: &ObjExportOptions,
) -> io::Result<()> {
    let dir = obj_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "obj_path has no parent"))?;
    fs.create_dir_all(dir)?;

    let mtl_path = obj_path.with_extension("mtl");
    let mtl_filename = mtl_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "mtl path has no filename"))?
        .to_string();

    let images_dir = dir.join("images");

    let mut obj_buf: Vec<u8> = Vec::new();
    let mut mtl_buf: Vec<u8> = Vec::new();
    {
        let mut obj_writer: IoObjWriter<_, f32> = IoObjWriter::new(&mut obj_buf);
        let mut mtl_writer: IoMtlWriter<_, f32> = IoMtlWriter::new(&mut mtl_buf);

        obj_writer.write_comment("VPin table OBJ file")?;
        obj_writer.write_material_lib(&[mtl_filename.as_str()])?;
        mtl_writer.write_comment("VPin table mat file")?;

        let mut state = WriterState::new(vpx, &images_dir, options);

        // 1. Implicit playfield quad (always emitted, matches VPinball
        //    PinTable::ExportMesh).
        write_playfield(&mut obj_writer, &mut mtl_writer, &mut state, fs)?;

        // 2. Walk gameitems in storage order (mirrors VPinball's m_vedit
        //    iteration). Visibility filter only - the m_desktopBackdrop bit
        //    is not yet parsed by vpin and is irrelevant for items with
        //    geometry (they all assert !m_desktopBackdrop in VPinball).
        for gameitem in &vpx.gameitems {
            write_gameitem(&mut obj_writer, &mut mtl_writer, &mut state, fs, gameitem)?;
        }
    }

    fs.write_file(obj_path, &obj_buf)?;
    fs.write_file(&mtl_path, &mtl_buf)?;
    info!(
        "Exported OBJ to {} ({} bytes), MTL to {} ({} bytes)",
        obj_path.display(),
        obj_buf.len(),
        mtl_path.display(),
        mtl_buf.len(),
    );
    Ok(())
}

/// Material fields consumed by the MTL writer. Sourced from either the
/// new `MATR` chunk (`Material`) or the legacy `MATE` chunk
/// (`SaveMaterial`), so the OBJ exporter doesn't have to care which
/// version of vpinball saved the table.
struct MaterialView {
    base_color: Color,
    glossy_color: Color,
    opacity: f32,
    opacity_active: bool,
    is_metal: bool,
}

// ---------------------------------------------------------------------------
// State threaded through the walk.
// ---------------------------------------------------------------------------

struct WriterState<'a> {
    vpx: &'a VPX,
    images_dir: PathBuf,
    table_dims: TableDimensions,
    /// VPinball's `GetDetailLevel()` - either the table's
    /// `user_detail_level` or the editor default. Drives ramp/rubber
    /// segment counts via `vpinball_ring_segments`.
    detail_level: u32,
    /// Running vertex offset for face indices. VPinball calls this
    /// `m_faceIndexOffset` and bumps it after each item by the number of
    /// vertices written.
    face_offset: u32,
    /// Mapping from VPX image name (case-preserving) to the on-disk file
    /// name inside `images/`. Populated lazily as textures are referenced.
    image_filenames: HashMap<String, String>,
    /// Lowercased image filenames already taken inside `images/`. Used to
    /// detect collisions on case-insensitive filesystems and append a
    /// `_dedup<n>` suffix, mirroring `expanded::images::write_images`.
    used_lower_filenames: HashSet<String>,
    image_dedup_counter: u32,
    /// VPX image names already written to disk. Skip subsequent encounters.
    images_written: HashSet<String>,
    /// Whether to dedup `newmtl` blocks in the MTL file. See
    /// [`ObjExportOptions::dedup_mtl_blocks`].
    dedup_mtl_blocks: bool,
    /// `(material_name, texture_name)` pairs already emitted as a
    /// `newmtl` block. Only consulted when `dedup_mtl_blocks` is true.
    seen_mtl_pairs: HashSet<(String, String)>,
    /// Multiplier applied to VPU vertex positions on write. Derived
    /// once from `ObjExportOptions::units`.
    position_scale: f32,
}

impl<'a> WriterState<'a> {
    fn new(vpx: &'a VPX, images_dir: &Path, options: &ObjExportOptions) -> Self {
        Self {
            vpx,
            images_dir: images_dir.to_path_buf(),
            table_dims: TableDimensions::new(
                vpx.gamedata.left,
                vpx.gamedata.top,
                vpx.gamedata.right,
                vpx.gamedata.bottom,
            ),
            detail_level: vpx.gamedata.effective_detail_level(),
            face_offset: 0,
            image_filenames: HashMap::new(),
            used_lower_filenames: HashSet::new(),
            image_dedup_counter: 0,
            images_written: HashSet::new(),
            dedup_mtl_blocks: options.dedup_mtl_blocks,
            seen_mtl_pairs: HashSet::new(),
            position_scale: options.units.scale(),
        }
    }

    /// Look up a material by name, transparently falling back from the
    /// new (10.8+) `MATR` chunk to the legacy `MATE` chunk used by older
    /// tables. Without the legacy fallback, pre-10.8 VPX files appear to
    /// have no materials at all and every `Kd`/`Ks` ends up at the
    /// "material missing" defaults (white / black) instead of the
    /// authored colours.
    fn material_view_by_name(&self, name: &str) -> Option<MaterialView> {
        if name.is_empty() {
            return None;
        }
        if let Some(ref mats) = self.vpx.gamedata.materials
            && let Some(m) = mats.iter().find(|m| m.name.eq_ignore_ascii_case(name))
        {
            return Some(MaterialView {
                base_color: m.base_color,
                glossy_color: m.glossy_color,
                opacity: m.opacity,
                opacity_active: m.opacity_active,
                is_metal: m.type_ == MaterialType::Metal,
            });
        }
        for m in &self.vpx.gamedata.materials_old {
            if m.name.eq_ignore_ascii_case(name) {
                return Some(MaterialView {
                    base_color: m.base_color,
                    glossy_color: m.glossy_color,
                    opacity: m.opacity,
                    // Bit 0 of `opacity_active_edge_alpha` is the
                    // active flag, the upper 7 bits are the edge alpha.
                    opacity_active: (m.opacity_active_edge_alpha & 1) != 0,
                    is_metal: m.is_metal,
                });
            }
        }
        None
    }

    fn image_by_name(&self, name: &str) -> Option<&ImageData> {
        if name.is_empty() {
            return None;
        }
        self.vpx
            .images
            .iter()
            .find(|img| img.name.eq_ignore_ascii_case(name))
    }

    fn surface_height(&self, surface_name: &str, x: f32, y: f32) -> f32 {
        if surface_name.is_empty() {
            return 0.0;
        }
        for item in &self.vpx.gameitems {
            match item {
                GameItemEnum::Wall(wall) if wall.name.eq_ignore_ascii_case(surface_name) => {
                    return wall.height_top;
                }
                GameItemEnum::Ramp(ramp) if ramp.name.eq_ignore_ascii_case(surface_name) => {
                    return crate::vpx::mesh::ramps::get_ramp_surface_height(ramp, x, y);
                }
                _ => {}
            }
        }
        0.0
    }
}

// ---------------------------------------------------------------------------
// Per-item dispatch.
// ---------------------------------------------------------------------------

fn write_playfield<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
) -> io::Result<()> {
    let (vertices, indices) = build_playfield_mesh(
        state.vpx.gamedata.left,
        state.vpx.gamedata.top,
        state.vpx.gamedata.right,
        state.vpx.gamedata.bottom,
    );

    // VPinball's `PinTable::ExportMesh` always emits `WriteMaterial(m_szPlayfieldMaterial, ...)`
    // and `UseTexture(m_szPlayfieldMaterial)` - so we always emit a `usemtl`
    // and a corresponding `newmtl` block, even if the playfield material
    // name is empty.
    let material_name = state.vpx.gamedata.playfield_material.clone();
    let texture_name = if state.vpx.gamedata.image.is_empty() {
        None
    } else {
        Some(state.vpx.gamedata.image.clone())
    };

    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            // VPinball's PinTable::ExportMesh emits `o <m_wzName>` for the
            // playfield quad (default "Table1" if the user never renamed
            // the table).
            name: &state.vpx.gamedata.name,
            vertices: &vertices,
            indices: &indices,
            translation: Vec3::new(0.0, 0.0, 0.0),
            material_name: Some(&material_name),
            texture_name: texture_name.as_deref(),
            smoothing: true,
        },
    )
}

fn write_gameitem<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    item: &GameItemEnum,
) -> io::Result<()> {
    match item {
        GameItemEnum::Primitive(primitive) => write_primitive(obj, mtl, state, fs, primitive),
        GameItemEnum::Wall(wall) => write_wall(obj, mtl, state, fs, wall),
        GameItemEnum::Ramp(ramp) => write_ramp(obj, mtl, state, fs, ramp),
        GameItemEnum::Rubber(rubber) => write_rubber(obj, mtl, state, fs, rubber),
        GameItemEnum::Bumper(bumper) => write_bumper(obj, mtl, state, fs, bumper),
        GameItemEnum::Flipper(flipper) => write_flipper(obj, mtl, state, fs, flipper),
        GameItemEnum::Gate(gate) => write_gate(obj, mtl, state, fs, gate),
        GameItemEnum::Kicker(kicker) => write_kicker(obj, mtl, state, fs, kicker),
        GameItemEnum::Spinner(spinner) => write_spinner(obj, mtl, state, fs, spinner),
        GameItemEnum::HitTarget(hit_target) => write_hittarget(obj, mtl, state, fs, hit_target),
        GameItemEnum::Trigger(trigger) => write_trigger(obj, mtl, state, fs, trigger),
        // Items VPinball does not include in OBJ export.
        GameItemEnum::Light(_)
        | GameItemEnum::Decal(_)
        | GameItemEnum::Plunger(_)
        | GameItemEnum::Flasher(_)
        | GameItemEnum::Reel(_)
        | GameItemEnum::Timer(_)
        | GameItemEnum::TextBox(_)
        | GameItemEnum::LightSequencer(_)
        | GameItemEnum::PartGroup(_)
        | GameItemEnum::Ball(_)
        | GameItemEnum::Generic(_, _) => Ok(()),
    }
}

fn write_primitive<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    primitive: &Primitive,
) -> io::Result<()> {
    if !primitive.is_visible {
        return Ok(());
    }
    let read = match primitive.read_mesh() {
        Ok(Some(read)) => read,
        Ok(None) => return Ok(()),
        Err(e) => {
            warn!(
                "Skipping primitive '{}' due to mesh read error: {e}",
                primitive.name
            );
            return Ok(());
        }
    };

    let world_matrix = primitive_world_matrix(primitive);
    let vertices: Vec<VertexWrapper> = read
        .vertices
        .into_iter()
        .map(|mut vw| {
            let v = Vertex3D::new(vw.vertex.x, vw.vertex.y, vw.vertex.z);
            let transformed = world_matrix.transform_vertex(v);
            vw.vertex.x = transformed.x;
            vw.vertex.y = transformed.y;
            vw.vertex.z = transformed.z;

            if !vw.vertex.nx.is_nan() && !vw.vertex.ny.is_nan() && !vw.vertex.nz.is_nan() {
                let n = world_matrix.transform_normal(vw.vertex.nx, vw.vertex.ny, vw.vertex.nz);
                let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
                if len > 0.0 {
                    vw.vertex.nx = n.x / len;
                    vw.vertex.ny = n.y / len;
                    vw.vertex.nz = n.z / len;
                }
            }
            vw
        })
        .collect();

    // Playfield primitives in VPinball pull material/image from gamedata,
    // not from the primitive's own fields.
    let (material_name, texture_name) = if primitive.is_playfield() {
        let t = if state.vpx.gamedata.image.is_empty() {
            None
        } else {
            Some(state.vpx.gamedata.image.clone())
        };
        (state.vpx.gamedata.playfield_material.clone(), t)
    } else {
        let t = if primitive.image.is_empty() {
            None
        } else {
            Some(primitive.image.clone())
        };
        (primitive.material.clone(), t)
    };

    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &primitive.name,
            vertices: &vertices,
            indices: &read.indices,
            translation: Vec3::new(0.0, 0.0, 0.0),
            material_name: Some(&material_name),
            texture_name: texture_name.as_deref(),
            smoothing: false,
        },
    )
}

/// Compose the full vpx-space world matrix for a primitive, mirroring
/// VPinball's `Primitive::RecalculateMatrices`:
///
/// ```text
/// RT       = Translate(tra) * RotZ(rot[2]) * RotY(rot[1]) * RotX(rot[0])
///                           * RotZ(rot[8]) * RotY(rot[7]) * RotX(rot[6])
/// fullMat  = Scale(size) * RT * Translate(pos)
/// ```
///
/// Order matters: position is applied *after* scale + rotation so it
/// doesn't get rotated/scaled along with the mesh. Got bitten by this -
/// see the regression test in this file.
fn primitive_world_matrix(primitive: &Primitive) -> Matrix3D {
    let pos = &primitive.position;
    let size = &primitive.size;
    let rot = &primitive.rot_and_tra;

    let rt = Matrix3D::translate(rot[3], rot[4], rot[5])
        * Matrix3D::rotate_z(rot[2].to_radians())
        * Matrix3D::rotate_y(rot[1].to_radians())
        * Matrix3D::rotate_x(rot[0].to_radians())
        * Matrix3D::rotate_z(rot[8].to_radians())
        * Matrix3D::rotate_y(rot[7].to_radians())
        * Matrix3D::rotate_x(rot[6].to_radians());

    Matrix3D::scale(size.x, size.y, size.z) * rt * Matrix3D::translate(pos.x, pos.y, pos.z)
}

fn write_wall<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    wall: &crate::vpx::gameitem::wall::Wall,
) -> io::Result<()> {
    let Some(meshes) = build_wall_meshes(wall, &state.table_dims) else {
        return Ok(());
    };

    // VPinball's `Surface::ExportMesh` (surface.cpp:675) has three branches:
    //
    //   - top-only:  one object `<name>`, top material, no smoothing group
    //   - side-only: one object `<name>`, side material, no smoothing group
    //   - both:      one object `<name>` with sideBuf*4 followed by topBuf*1,
    //                top material + `s 1`, top face indices shifted by 4*N
    //
    // We mirror the same dispatch.
    match (
        wall.is_top_bottom_visible,
        wall.is_side_visible,
        meshes.top,
        meshes.side,
    ) {
        (true, false, Some((vertices, indices)), _) => {
            // VPinball top-only special case (surface.cpp:690-707):
            // when an image is set, the OBJ material name is the image
            // name (not the top material) and the MTL receives the
            // texture file path. When no image, material name is "none".
            let (material_name, texture_name) = if wall.image.is_empty() {
                ("none".to_string(), None)
            } else {
                (wall.image.clone(), Some(wall.image.clone()))
            };
            write_block(
                obj,
                mtl,
                state,
                fs,
                Block {
                    name: &wall.name,
                    vertices: &vertices,
                    indices: &indices,
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    material_name: Some(&material_name),
                    texture_name: texture_name.as_deref(),
                    smoothing: false,
                },
            )?;
        }
        (false, true, _, Some((vertices, indices))) => {
            let material_name = wall.side_material.clone();
            write_block(
                obj,
                mtl,
                state,
                fs,
                Block {
                    name: &wall.name,
                    vertices: &vertices,
                    indices: &indices,
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    material_name: Some(&material_name),
                    texture_name: None,
                    smoothing: false,
                },
            )?;
        }
        (true, true, Some((top_v, top_i)), Some((side_v, side_i))) => {
            let side_count = side_v.len() as i64;
            let mut combined_v = side_v;
            combined_v.extend(top_v);
            let mut combined_i = side_i;
            combined_i.extend(top_i.into_iter().map(|f| VpxFace {
                i0: f.i0 + side_count,
                i1: f.i1 + side_count,
                i2: f.i2 + side_count,
            }));
            let material_name = wall.top_material.clone();
            write_block(
                obj,
                mtl,
                state,
                fs,
                Block {
                    name: &wall.name,
                    vertices: &combined_v,
                    indices: &combined_i,
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    material_name: Some(&material_name),
                    texture_name: None,
                    smoothing: true,
                },
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn write_ramp<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    ramp: &crate::vpx::gameitem::ramp::Ramp,
) -> io::Result<()> {
    if !ramp.is_visible {
        return Ok(());
    }
    // VPinball's `Ramp::GenerateWireMesh` uses max-precision wire
    // segments when the material is opaque (`!mat->m_bOpacityActive`).
    // Missing/empty material falls through to opaque (vpinball's dummy
    // material defaults `m_bOpacityActive = false`).
    let material_opacity_active = state
        .material_view_by_name(&ramp.material)
        .is_some_and(|m| m.opacity_active);
    let Some((vertices, indices)) = build_ramp_mesh(
        ramp,
        &state.table_dims,
        state.detail_level,
        material_opacity_active,
    ) else {
        return Ok(());
    };
    let material_name = ramp.material.clone();
    let texture_name = if ramp.image.is_empty() {
        None
    } else {
        Some(ramp.image.clone())
    };
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &ramp.name,
            vertices: &vertices,
            indices: &indices,
            translation: Vec3::new(0.0, 0.0, 0.0),
            material_name: Some(&material_name),
            texture_name: texture_name.as_deref(),
            smoothing: true,
        },
    )
}

fn write_rubber<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    rubber: &crate::vpx::gameitem::rubber::Rubber,
) -> io::Result<()> {
    if !rubber.is_visible {
        return Ok(());
    }
    let Some((vertices, indices, center)) = build_rubber_mesh(rubber, state.detail_level) else {
        return Ok(());
    };
    let material_name = rubber.material.clone();
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &rubber.name,
            vertices: &vertices,
            indices: &indices,
            translation: center,
            material_name: Some(&material_name),
            texture_name: None,
            smoothing: true,
        },
    )
}

fn write_bumper<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    bumper: &crate::vpx::gameitem::bumper::Bumper,
) -> io::Result<()> {
    let surface_height = state.surface_height(&bumper.surface, bumper.center.x, bumper.center.y);
    let translation = Vec3::new(bumper.center.x, bumper.center.y, surface_height);
    let meshes = build_bumper_meshes(bumper);

    if let Some((vertices, indices)) = meshes.base {
        let material_name = bumper.base_material.clone();
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Base", bumper.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    if let Some((vertices, indices)) = meshes.ring {
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Ring", bumper.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                // VPinball calls WriteFaceInfoList without WriteMaterial for
                // the ring (carries over the previous material). We don't
                // emit usemtl here either.
                material_name: None,
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    if let Some((vertices, indices)) = meshes.socket {
        let material_name = bumper.socket_material.clone();
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Skirt", bumper.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    if let Some((vertices, indices)) = meshes.cap {
        let material_name = bumper.cap_material.clone();
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Cap", bumper.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    Ok(())
}

fn write_flipper<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    flipper: &crate::vpx::gameitem::flipper::Flipper,
) -> io::Result<()> {
    // VPinball's `Flipper::ExportMesh` has no `m_d.m_visible` guard.
    let Some(meshes) = build_flipper_meshes_unchecked(flipper, 0.0) else {
        return Ok(());
    };
    let translation = meshes.center;

    let (base_vertices, base_indices) = meshes.base;
    let base_material = flipper.material.clone();
    let base_texture = flipper.image.as_ref().filter(|s| !s.is_empty()).cloned();
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &format!("{}Base", flipper.name),
            vertices: &base_vertices,
            indices: &base_indices,
            translation,
            material_name: Some(&base_material),
            texture_name: base_texture.as_deref(),
            smoothing: true,
        },
    )?;

    if let Some((rubber_vertices, rubber_indices)) = meshes.rubber {
        let rubber_material = flipper.rubber_material.clone();
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Rubber", flipper.name),
                vertices: &rubber_vertices,
                indices: &rubber_indices,
                translation,
                material_name: Some(&rubber_material),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    Ok(())
}

fn write_gate<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    gate: &crate::vpx::gameitem::gate::Gate,
) -> io::Result<()> {
    // VPinball's `Gate::ExportMesh` has no `m_d.m_visible` guard.
    let surface_height = state.surface_height(&gate.surface, gate.center.x, gate.center.y);
    let translation = Vec3::new(gate.center.x, gate.center.y, surface_height + gate.height);
    let Some(meshes) = build_gate_meshes_unchecked(gate) else {
        return Ok(());
    };
    let material_name = gate.material.clone();
    if let Some((vertices, indices)) = meshes.bracket {
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Bracket", gate.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    let (vertices, indices) = meshes.wire;
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &format!("{}Wire", gate.name),
            vertices: &vertices,
            indices: &indices,
            translation,
            material_name: Some(&material_name),
            texture_name: None,
            smoothing: true,
        },
    )
}

fn write_kicker<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    kicker: &crate::vpx::gameitem::kicker::Kicker,
) -> io::Result<()> {
    if matches!(
        kicker.kicker_type,
        crate::vpx::gameitem::kicker::KickerType::Invisible
    ) {
        return Ok(());
    }
    let surface_height = state.surface_height(&kicker.surface, kicker.center.x, kicker.center.y);
    let translation = Vec3::new(kicker.center.x, kicker.center.y, surface_height);
    let meshes = build_kicker_meshes(kicker);
    let material_name = kicker.material.clone();
    if let Some((vertices, indices)) = meshes.plate {
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Plate", kicker.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    if let Some((vertices, indices)) = meshes.kicker {
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &kicker.name,
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    Ok(())
}

fn write_spinner<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    spinner: &crate::vpx::gameitem::spinner::Spinner,
) -> io::Result<()> {
    // VPinball's `Spinner::ExportMesh` has no `m_d.m_visible` guard.
    let surface_height = state.surface_height(&spinner.surface, spinner.center.x, spinner.center.y);
    let translation = Vec3::new(
        spinner.center.x,
        spinner.center.y,
        surface_height + spinner.height,
    );
    let meshes = build_spinner_meshes(spinner);
    let material_name = spinner.material.clone();
    if let Some((vertices, indices)) = meshes.bracket {
        // VPinball's `Spinner::ExportMesh` (spinner.cpp:273) emits
        // `WriteMaterial(m_szMaterial)` and `UseTexture(m_szMaterial)`
        // for the bracket.
        write_block(
            obj,
            mtl,
            state,
            fs,
            Block {
                name: &format!("{}Bracket", spinner.name),
                vertices: &vertices,
                indices: &indices,
                translation,
                material_name: Some(&material_name),
                texture_name: None,
                smoothing: true,
            },
        )?;
    }
    let (vertices, indices) = meshes.plate;
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &format!("{}Plate", spinner.name),
            vertices: &vertices,
            indices: &indices,
            translation,
            // VPinball does NOT emit `WriteMaterial`/`UseTexture` for the
            // spinner plate (spinner.cpp:286-291). The plate inherits the
            // bracket's material in the OBJ.
            material_name: None,
            texture_name: None,
            smoothing: true,
        },
    )
}

fn write_hittarget<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    target: &crate::vpx::gameitem::hittarget::HitTarget,
) -> io::Result<()> {
    // VPinball's `HitTarget::ExportMesh` has no `m_d.m_visible` guard.
    let Some((vertices, indices)) = build_hit_target_mesh_unchecked(target) else {
        return Ok(());
    };
    let translation = Vec3::new(target.position.x, target.position.y, target.position.z);
    let material_name = target.material.clone();
    let texture_name = if target.image.is_empty() {
        None
    } else {
        Some(target.image.clone())
    };
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &target.name,
            vertices: &vertices,
            indices: &indices,
            translation,
            material_name: Some(&material_name),
            texture_name: texture_name.as_deref(),
            smoothing: true,
        },
    )
}

fn write_trigger<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    trigger: &crate::vpx::gameitem::trigger::Trigger,
) -> io::Result<()> {
    if !trigger.is_visible {
        return Ok(());
    }
    let surface_height = state.surface_height(&trigger.surface, trigger.center.x, trigger.center.y);
    let translation = Vec3::new(trigger.center.x, trigger.center.y, surface_height);
    let Some((vertices, indices)) = build_trigger_mesh(trigger) else {
        return Ok(());
    };
    let material_name = trigger.material.clone();
    write_block(
        obj,
        mtl,
        state,
        fs,
        Block {
            name: &trigger.name,
            vertices: &vertices,
            indices: &indices,
            translation,
            material_name: Some(&material_name),
            texture_name: None,
            smoothing: true,
        },
    )
}

// ---------------------------------------------------------------------------
// Block writer + MTL/image emission.
// ---------------------------------------------------------------------------

struct Block<'a> {
    name: &'a str,
    vertices: &'a [VertexWrapper],
    indices: &'a [VpxFace],
    translation: Vec3,
    material_name: Option<&'a str>,
    texture_name: Option<&'a str>,
    smoothing: bool,
}

fn write_block<O: ObjWriter<f32>, M: MtlWriter<f32>>(
    obj: &mut O,
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    block: Block,
) -> io::Result<()> {
    if block.vertices.is_empty() || block.indices.is_empty() {
        return Ok(());
    }

    obj.write_object_name(block.name)?;

    // Positions: world = local + translation; obj_z = -world_z; scale to chosen unit.
    let s = state.position_scale;
    for vw in block.vertices {
        let v = &vw.vertex;
        let x = (v.x + block.translation.x) * s;
        let y = (v.y + block.translation.y) * s;
        let z = (v.z + block.translation.z) * s;
        obj.write_vertex(x, y, -z, None)?;
    }
    // UVs: tv -> 1 - tv, NaN -> 0
    for vw in block.vertices {
        let tu = if vw.vertex.tu.is_nan() {
            0.0
        } else {
            vw.vertex.tu
        };
        let tv = if vw.vertex.tv.is_nan() {
            0.0
        } else {
            1.0 - vw.vertex.tv
        };
        obj.write_texture_coordinate(tu, Some(tv), None)?;
    }
    // Normals: nz -> -nz, NaN -> 0
    for vw in block.vertices {
        let nx = if vw.vertex.nx.is_nan() {
            0.0
        } else {
            vw.vertex.nx
        };
        let ny = if vw.vertex.ny.is_nan() {
            0.0
        } else {
            vw.vertex.ny
        };
        let nz = if vw.vertex.nz.is_nan() {
            0.0
        } else {
            -vw.vertex.nz
        };
        obj.write_normal(nx, ny, nz)?;
    }

    // VPinball calls `WriteMaterial` + `UseTexture` for every item in
    // `ExportMesh`, even when the material name is empty - so the MTL ends
    // up with one `newmtl` block per `usemtl`, with duplicates. We emit
    // exactly the same way (no dedup) for parity.
    if let Some(material_name) = block.material_name {
        let mtl_name = emit_mtl_block(mtl, state, fs, material_name, block.texture_name)?;
        obj.write_use_material(&mtl_name)?;
    }

    if block.smoothing {
        obj.write_smoothing_group(SmoothingGroup::Group(1))?;
    }

    // Faces: 1-based, +face_offset, reversed winding.
    for face in block.indices {
        let v1 = (face.i2 as u32 + 1 + state.face_offset) as usize;
        let v2 = (face.i1 as u32 + 1 + state.face_offset) as usize;
        let v3 = (face.i0 as u32 + 1 + state.face_offset) as usize;
        obj.write_face(&[
            (v1, Some(v1), Some(v1)),
            (v2, Some(v2), Some(v2)),
            (v3, Some(v3), Some(v3)),
        ])?;
    }

    state.face_offset = state
        .face_offset
        .saturating_add(block.vertices.len() as u32);
    Ok(())
}

/// Emit a `newmtl` block and write the referenced image to disk on
/// first use. Returns the material name as written (with spaces erased -
/// VPinball does the same in `WriteMaterial` for both the MTL block and
/// the `usemtl`).
///
/// By default vpinball-style: emits one `newmtl` block per call, with
/// duplicates. When `state.dedup_mtl_blocks` is set, only the first
/// occurrence of each `(material, texture)` pair is written; subsequent
/// calls still resolve the on-disk image (so it gets extracted) but
/// skip the MTL block. The `usemtl` reference in the OBJ remains the
/// same sanitized name and resolves to the first-emitted block.
fn emit_mtl_block<M: MtlWriter<f32>>(
    mtl: &mut M,
    state: &mut WriterState,
    fs: &dyn FileSystem,
    material_name: &str,
    texture_name: Option<&str>,
) -> io::Result<String> {
    let mtl_name = sanitize_material_name(material_name);

    // Resolve the on-disk image filename and ensure the file is written.
    // We do this whether or not we're about to skip the MTL block, so
    // the `images/` folder stays complete in dedup mode too.
    let map_path = if let Some(name) = texture_name {
        ensure_image_written(state, fs, name)?
    } else {
        None
    };

    if state.dedup_mtl_blocks {
        let key = (
            material_name.to_string(),
            texture_name.unwrap_or("").to_string(),
        );
        if !state.seen_mtl_pairs.insert(key) {
            // Pair already emitted - skip this newmtl block.
            return Ok(mtl_name);
        }
    }

    write_mtl_block(mtl, state, &mtl_name, material_name, map_path.as_deref())?;
    Ok(mtl_name)
}

/// Match VPinball's `WriteMaterial` (`std::erase(' ')`).
fn sanitize_material_name(name: &str) -> String {
    name.chars().filter(|c| *c != ' ').collect()
}

fn write_mtl_block<M: MtlWriter<f32>>(
    mtl: &mut M,
    state: &WriterState,
    mtl_name: &str,
    material_name: &str,
    image_relative_path: Option<&str>,
) -> io::Result<()> {
    let material = state.material_view_by_name(material_name);

    // Defaults match VPinball's WriteMaterial when no Material is found.
    let (kd, ks, opacity) = match material {
        Some(m) => (
            color_to_kd(&m),
            color_to_ks(&m),
            if m.opacity_active { m.opacity } else { 1.0 },
        ),
        None => ([1.0, 1.0, 1.0], [0.0, 0.0, 0.0], 1.0),
    };

    mtl.write_new_material(mtl_name)?;
    mtl.write_specular_exponent(7.843137)?;
    mtl.write_ambient(0.0, Some(0.0), Some(0.0))?;
    mtl.write_diffuse(kd[0], Some(kd[1]), Some(kd[2]))?;
    mtl.write_specular(ks[0], Some(ks[1]), Some(ks[2]))?;
    mtl.write_optical_density(1.5)?;
    mtl.write_dissolve(opacity)?;
    mtl.write_illumination_model(5)?;
    if let Some(path) = image_relative_path {
        mtl.write_map(MapKind::Diffuse, path)?;
        mtl.write_map(MapKind::Ambient, path)?;
    }
    Ok(())
}

fn color_to_kd(m: &MaterialView) -> [f32; 3] {
    [
        m.base_color.r as f32 / 255.0,
        m.base_color.g as f32 / 255.0,
        m.base_color.b as f32 / 255.0,
    ]
}

fn color_to_ks(m: &MaterialView) -> [f32; 3] {
    if m.is_metal {
        // Match VPinball's `m_cGlossy` for metals (uses base color).
        color_to_kd(m)
    } else {
        [
            m.glossy_color.r as f32 / 255.0,
            m.glossy_color.g as f32 / 255.0,
            m.glossy_color.b as f32 / 255.0,
        ]
    }
}

/// Look up the image by `name`, write it to `images/` if not already
/// written, and return the relative path for the `.mtl` (or None if the
/// image is missing or has no data).
fn ensure_image_written(
    state: &mut WriterState,
    fs: &dyn FileSystem,
    name: &str,
) -> io::Result<Option<String>> {
    if state.images_written.contains(name) {
        return Ok(state
            .image_filenames
            .get(name)
            .map(|f| format!("images/{}", f)));
    }
    // Clone the bits we need from the image before we mutably borrow state.
    let (image_name, image_ext, payload) = {
        let Some(image) = state.image_by_name(name) else {
            warn!("Texture '{name}' referenced but not found in vpx.images");
            return Ok(None);
        };
        let payload = if let Some(jpeg) = &image.jpeg {
            ImagePayload::Raw(jpeg.data.clone())
        } else if let Some(bits) = &image.bits {
            ImagePayload::Bmp {
                lzw: bits.lzw_compressed_data.clone(),
                width: image.width,
                height: image.height,
            }
        } else {
            warn!("Texture '{name}' has no data; skipping");
            return Ok(None);
        };
        (image.name.clone(), image.ext(), payload)
    };

    let on_disk = sanitized_image_filename(state, &image_name, &image_ext);
    let path = state.images_dir.join(&on_disk);
    fs.create_dir_all(&state.images_dir)?;

    match payload {
        ImagePayload::Raw(bytes) => fs.write_file(&path, &bytes)?,
        ImagePayload::Bmp { lzw, width, height } => {
            write_image_bmp(&path, &lzw, width, height, fs)?
        }
    }

    state.images_written.insert(name.to_string());
    state
        .image_filenames
        .insert(name.to_string(), on_disk.clone());
    Ok(Some(format!("images/{}", on_disk)))
}

enum ImagePayload {
    Raw(Vec<u8>),
    Bmp {
        lzw: Vec<u8>,
        width: u32,
        height: u32,
    },
}

fn sanitized_image_filename(state: &mut WriterState, name: &str, ext: &str) -> String {
    let base = sanitize_filename(name);
    let mut filename = format!("{}.{}", base, ext);
    let mut lower = filename.to_lowercase();
    while state.used_lower_filenames.contains(&lower) {
        state.image_dedup_counter += 1;
        filename = format!("{}_dedup{}.{}", base, state.image_dedup_counter, ext);
        lower = filename.to_lowercase();
    }
    state.used_lower_filenames.insert(lower);
    filename
}

fn write_image_bmp(
    path: &Path,
    lzw_compressed: &[u8],
    width: u32,
    height: u32,
    fs: &dyn FileSystem,
) -> io::Result<()> {
    use crate::vpx::image::vpx_image_to_dynamic_image;
    use std::io::Cursor;
    let dynamic = vpx_image_to_dynamic_image(lzw_compressed, width, height);
    let mut buffer = Cursor::new(Vec::new());
    dynamic
        .write_to(&mut buffer, image::ImageFormat::Bmp)
        .map_err(|e| io::Error::other(format!("Failed to encode BMP {}: {e}", path.display())))?;
    fs.write_file(path, buffer.get_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::MemoryFileSystem;

    fn export_to_memory(options: &ObjExportOptions) -> (String, String) {
        let vpx = VPX::default();
        let fs = MemoryFileSystem::default();
        let obj_path = Path::new("out/test.obj");
        export_obj(&vpx, obj_path, &fs, options).unwrap();
        let obj = String::from_utf8(fs.read_file(obj_path).unwrap()).unwrap();
        let mtl =
            String::from_utf8(fs.read_file(&obj_path.with_extension("mtl")).unwrap()).unwrap();
        (obj, mtl)
    }

    #[test]
    fn primitive_world_matrix_does_not_scale_position() {
        // Regression: the previous order `Translate(pos) * Scale * RT`
        // applied scale/rotation to the position itself. With size != 1
        // and a non-zero rotation, primitives ended up far from where
        // vpinball places them. Verify the world matrix matches the
        // vpinball convention `Scale * RT * Translate(pos)`.
        //
        // Setup:
        //   - local vertex at (1, 0, 0)
        //   - size  = (2, 2, 2)         (would be doubled if scale leaked into pos)
        //   - rot[0] = 90 deg around X  (would rotate pos if order is wrong)
        //   - pos = (10, 20, 30)
        //
        // Expected (vpinball): scale -> rotate -> translate(pos)
        //   v_local = (1, 0, 0)
        //   after scale(2): (2, 0, 0)
        //   after RotX(90): (2, 0, 0)   (X axis is invariant)
        //   after Translate(pos): (12, 20, 30)
        use crate::vpx::gameitem::vertex3d::Vertex3D as ItemVertex3D;
        let primitive = Primitive {
            position: ItemVertex3D::new(10.0, 20.0, 30.0),
            size: ItemVertex3D::new(2.0, 2.0, 2.0),
            rot_and_tra: [90.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            ..Primitive::default()
        };

        let m = primitive_world_matrix(&primitive);
        let out = m.transform_vertex(Vertex3D::new(1.0, 0.0, 0.0));
        assert!(
            (out.x - 12.0).abs() < 1e-4
                && (out.y - 20.0).abs() < 1e-4
                && (out.z - 30.0).abs() < 1e-4,
            "expected (12, 20, 30), got ({}, {}, {})",
            out.x,
            out.y,
            out.z,
        );
    }

    #[test]
    fn default_options_emit_one_newmtl_per_usemtl() {
        // VPinball-faithful default: each `usemtl` gets a `newmtl`.
        let (obj, mtl) = export_to_memory(&ObjExportOptions::default());
        let usemtl = obj.matches("usemtl ").count();
        let newmtl = mtl.matches("newmtl ").count();
        assert!(usemtl > 0);
        assert_eq!(usemtl, newmtl);
    }

    /// Find the vertex with the largest |x| in an OBJ string, returning
    /// (index, [x, y, z]). Picks a non-zero coordinate so unit-scale
    /// ratios are stable.
    fn pick_extreme_vertex(obj: &str) -> (usize, [f32; 3]) {
        let mut best: Option<(usize, [f32; 3])> = None;
        for (idx, line) in obj.lines().filter_map(|l| l.strip_prefix("v ")).enumerate() {
            let coords: Vec<f32> = line
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            assert_eq!(coords.len(), 3, "unexpected vertex line: {line:?}");
            let v = [coords[0], coords[1], coords[2]];
            match best {
                None => best = Some((idx, v)),
                Some((_, b)) if v[0].abs() > b[0].abs() => best = Some((idx, v)),
                _ => {}
            }
        }
        best.expect("no `v ` position line found in OBJ")
    }

    #[test]
    fn units_scale_positions() {
        // Same default table exported in each unit; the first vertex
        // should differ by the documented VPU scale factors.
        let (vpu_obj, _) = export_to_memory(&ObjExportOptions {
            units: ExportUnits::Vpu,
            ..ObjExportOptions::default()
        });
        let (mm_obj, _) = export_to_memory(&ObjExportOptions {
            units: ExportUnits::Mm,
            ..ObjExportOptions::default()
        });
        let (cm_obj, _) = export_to_memory(&ObjExportOptions {
            units: ExportUnits::Cm,
            ..ObjExportOptions::default()
        });
        let (m_obj, _) = export_to_memory(&ObjExportOptions {
            units: ExportUnits::M,
            ..ObjExportOptions::default()
        });

        // Use the same vertex index across all four exports - they
        // share topology, only the scale differs.
        let (idx, vpu) = pick_extreme_vertex(&vpu_obj);
        assert!(vpu[0].abs() > 1.0, "expected non-trivial VPU coord");
        let pick_at = |obj: &str| -> [f32; 3] {
            let line = obj
                .lines()
                .filter_map(|l| l.strip_prefix("v "))
                .nth(idx)
                .expect("vertex index out of range");
            let coords: Vec<f32> = line
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            [coords[0], coords[1], coords[2]]
        };
        let mm = pick_at(&mm_obj);
        let cm = pick_at(&cm_obj);
        let m = pick_at(&m_obj);

        let mm_ratio = mm[0] / vpu[0];
        let cm_ratio = cm[0] / vpu[0];
        let m_ratio = m[0] / vpu[0];

        let expected_mm = (25.4 * 1.0625) / 50.0;
        let expected_cm = expected_mm / 10.0;
        let expected_m = expected_mm / 1000.0;

        assert!(
            (mm_ratio - expected_mm).abs() < 1e-5,
            "mm ratio {mm_ratio} != {expected_mm}",
        );
        assert!(
            (cm_ratio - expected_cm).abs() < 1e-6,
            "cm ratio {cm_ratio} != {expected_cm}",
        );
        assert!(
            (m_ratio - expected_m).abs() < 1e-7,
            "m ratio {m_ratio} != {expected_m}",
        );
    }

    #[test]
    fn dedup_option_collapses_newmtl_blocks() {
        // Same export with dedup on: at most one `newmtl` per unique
        // material name, never more than `usemtl`. For the default VPX
        // (single playfield material), both should collapse to 1.
        let (obj, mtl) = export_to_memory(&ObjExportOptions {
            dedup_mtl_blocks: true,
            ..ObjExportOptions::default()
        });
        let usemtl = obj.matches("usemtl ").count();
        let newmtl = mtl.matches("newmtl ").count();
        assert!(usemtl >= 1);
        assert!(
            newmtl <= usemtl,
            "newmtl ({newmtl}) should not exceed usemtl ({usemtl}) when deduped"
        );
    }
}
