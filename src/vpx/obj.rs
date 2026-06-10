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
//!
//! The Z negations are sign-bit flips and thus exact, but the V flip is
//! arithmetic: in f32 `1 - (1 - tv)` shifts most tv values in (0, 0.5) by
//! an ulp, which broke the bit-exact extract/assemble round-trip. The flip
//! is therefore done at f64 precision on both sides, and the written `vt`
//! V value gets just enough extra decimal digits that the read side
//! recovers the original f32 tv bit-for-bit (see [`flipped_v`]).

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
// Some vpx files contain NaN values in their vertex data. NaN payload bits
// cannot survive a text format (every "NaN" parses back as the canonical
// quiet NaN), so the original vpx bytes are stored in a comment preceding
// the vertex's vn line.
// Older versions only covered NaN normals with a 12 byte comment; NaN
// texture coordinates or positions now get a comment with the full
// 32 byte encoded vertex. Both forms are understood when reading.

/// The original vpx bytes for a vertex with NaN values, carried in a
/// `# vpx <hex>` comment.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum VpxCommentBytes {
    /// Legacy form: only the 12 normal bytes (offsets 12..24).
    Normal([u8; 12]),
    /// The full 32 byte encoded vertex.
    Vertex([u8; 32]),
}

fn obj_vpx_comment(bytes: &[u8]) -> String {
    // a comment with the vertex bytes as hex string
    let hex = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<String>>()
        .join(" ");
    format!("vpx {hex}")
}

fn obj_parse_vpx_comment(comment: &str) -> Option<VpxCommentBytes> {
    if let Some(hex) = comment.strip_prefix("vpx ") {
        let bytes = hex
            .split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).unwrap())
            .collect::<Vec<u8>>();
        match bytes.len() {
            12 => {
                let mut result = [0; 12];
                result.copy_from_slice(&bytes);
                Some(VpxCommentBytes::Normal(result))
            }
            32 => {
                let mut result = [0; 32];
                result.copy_from_slice(&bytes);
                Some(VpxCommentBytes::Vertex(result))
            }
            _ => None,
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
        wavefront_obj_io::IoObjWriter::new(&mut *writer);

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
    drop(obj_writer);
    let mut tv_exact = Vec::with_capacity(vpx_vertices.len());
    for VertexWrapper { vertex, .. } in vpx_vertices {
        // vpinball's WriteVertexInfo flips V on write (tv -> 1 - tv).
        // These lines are written manually as the V value may need more
        // precision than the f32 obj writer can provide, see flipped_v.
        let v = flipped_v(vertex.tv);
        writeln!(writer, "vt {} {}", vertex.tu, v.text)?;
        tv_exact.push(v.exact);
    }
    let mut obj_writer: wavefront_obj_io::IoObjWriter<_, f32> =
        wavefront_obj_io::IoObjWriter::new(&mut *writer);
    let mut unrepresentable_vertices = 0u32;
    for (
        VertexWrapper {
            vpx_encoded_vertex,
            vertex,
        },
        tv_exact,
    ) in vpx_vertices.iter().zip(tv_exact)
    {
        // If one of the values is NaN (payload bits do not survive the text
        // format) or tv cannot be recovered through the V flip, we write a
        // special comment with the full encoded vertex bytes. The read side
        // restores the bytes verbatim.
        let has_nan = [
            vertex.x, vertex.y, vertex.z, vertex.nx, vertex.ny, vertex.nz, vertex.tu, vertex.tv,
        ]
        .iter()
        .any(|v| v.is_nan());
        if has_nan || !tv_exact {
            unrepresentable_vertices += 1;
            let content = obj_vpx_comment(vpx_encoded_vertex);
            obj_writer.write_comment(content)?;
        }
        let x = if vertex.nx.is_nan() { 0.0 } else { vertex.nx };
        let y = if vertex.ny.is_nan() { 0.0 } else { vertex.ny };
        let z = if vertex.nz.is_nan() { 0.0 } else { -vertex.nz };
        obj_writer.write_normal(x, y, z)?;
    }
    if unrepresentable_vertices > 0 {
        warn!(
            "{unrepresentable_vertices} of {} vertices in {name} have NaN or otherwise \
            unrepresentable values, their vpx bytes are preserved in comments",
            vpx_vertices.len()
        );
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

/// Formats the obj V texture coordinate for a vpx `tv` value.
///
/// vpinball's obj dialect flips V (`obj_v = 1 - tv`). That subtraction
/// rounds in f32, so writing `1.0f32 - tv` makes the read side recover a
/// slightly different tv for most values in (0, 0.5). Instead the flip is
/// done in f64 and the value is printed with the fewest decimal digits
/// that still recover tv bit-for-bit through the read side's f64 flip.
/// Typical UV values keep their usual short form; only values that need
/// extra digits get them.
///
/// Limits, inherent to the flipped representation (vpinball has them too):
/// * `0 < |tv| < ~2^-30`: `1 - tv` falls in (0.5, 2) where even f64 spacing
///   is coarser than such a tv needs, so no text of any length can recover
///   it; the closest representable value is written instead.
/// * `tv = -0.0` is recovered as `0.0` (`1 - v` cannot produce a negative
///   zero) and NaN tv loses its payload bits, like every value in the
///   text-based obj format.
///
/// All these cases are reported via [`FlippedV::exact`] so the writer can
/// preserve the original bytes in a `# vpx <hex>` comment instead.
pub(crate) struct FlippedV {
    pub(crate) text: String,
    /// True when the text recovers tv bit-for-bit through the read side's
    /// f64 flip, false for the unrecoverable cases listed above.
    pub(crate) exact: bool,
}

pub(crate) fn flipped_v(tv: f32) -> FlippedV {
    if tv.is_nan() {
        // 1 - NaN stays NaN, no text recovers the payload bits
        return FlippedV {
            text: "NaN".to_string(),
            exact: false,
        };
    }
    let flipped = 1.0 - f64::from(tv);
    // For tv in [0.5, 2] the f32 flip is already exact (Sterbenz lemma),
    // the shortest f32 text recovers tv and needs no verification.
    if (0.5..=2.0).contains(&tv) {
        return FlippedV {
            text: format!("{}", flipped as f32),
            exact: true,
        };
    }
    let recovers =
        |s: &str| matches!(s.parse::<f64>(), Ok(v) if ((1.0 - v) as f32).to_bits() == tv.to_bits());
    // The shortest f32 text (what a plain f32 flip would write) is optimal
    // and usually enough.
    let short = format!("{}", flipped as f32);
    if recovers(&short) {
        return FlippedV {
            text: short,
            exact: true,
        };
    }
    if let Some(text) = shortest_recovering_text(flipped, tv) {
        return FlippedV { text, exact: true };
    }
    // Unrecoverable tv (see the limits above), write the exact f64 flip.
    FlippedV {
        text: format!("{flipped}"),
        exact: false,
    }
}

/// Finds the decimal text with the fewest fractional digits that still
/// recovers `tv` exactly when the read side parses it and flips it back
/// (`(1.0 - text.parse::<f64>()) as f32 == tv`, compared by bits).
/// `flipped` must be `1.0 - f64::from(tv)`.
///
/// Returns `None` when no precision up to 17 digits recovers tv; that
/// only happens for the unrecoverable values listed on [`flipped_v`].
///
/// Instead of scanning all precisions from 1 (which wastes 6-10
/// format+parse rounds, as the answer is typically 7-11 digits), the
/// search starts at [`estimated_flip_precision`] and walks down while
/// shorter texts still recover, or up if the estimate is not enough.
/// The recovering precisions form a contiguous range ending at 17, so
/// this finds the same fewest-digits text as a full scan, usually in one
/// or two probes.
fn shortest_recovering_text(flipped: f64, tv: f32) -> Option<String> {
    let probe = |precision: usize| {
        let s = format!("{flipped:.precision$}");
        let recovered =
            matches!(s.parse::<f64>(), Ok(v) if ((1.0 - v) as f32).to_bits() == tv.to_bits());
        recovered.then_some(s)
    };
    let estimate = estimated_flip_precision(tv);
    if let Some(mut text) = probe(estimate) {
        // walk down to the fewest digits that still recover
        let mut precision = estimate;
        while precision > 1 {
            match probe(precision - 1) {
                Some(shorter) => {
                    text = shorter;
                    precision -= 1;
                }
                None => break,
            }
        }
        Some(text)
    } else {
        // the estimate was too low (tv with an unusually long decimal
        // expansion, or close to the representation limits), walk up
        (estimate + 1..=17).find_map(probe)
    }
}

/// Estimates how many fractional decimal digits the flipped V text needs
/// so that the read side recovers `tv` exactly.
///
/// Recovery requires the parsed value to land within about half an f32
/// ulp of the exact `1 - tv`. An f32 ulp near tv is `2^(exponent - 23)`
/// and a decimal with `p` fractional digits resolves steps of `10^-p`,
/// so equating the two gives `p ~= (23 - exponent) * log10(2)`. For
/// example tv around 0.001 (exponent -10) needs about 10 digits, tv
/// around 0.3 (exponent -2) about 8.
///
/// This is an estimate, not a bound: a tv whose decimal expansion happens
/// to be short needs fewer digits, and for tv below ~2^-30 (where only
/// lucky f64 grid coincidences can be recovered at all, see [`flipped_v`])
/// the formula overshoots. It only positions the search in
/// [`shortest_recovering_text`], which probes in both directions.
fn estimated_flip_precision(tv: f32) -> usize {
    let exponent = ((tv.to_bits() >> 23) & 0xff) as i32 - 127;
    let estimate = ((23 - exponent) as f64 * std::f64::consts::LOG10_2).ceil() as usize;
    estimate.clamp(1, 17)
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

pub(crate) fn read_obj_from_reader<R: BufRead>(reader: &mut R) -> io::Result<ReadObjResult> {
    read_obj_from_reader_with_options(reader, true)
}

/// Like [`read_obj_from_reader`] but lets the caller pick whether
/// vpinball's right-handed -> left-handed conversion should be applied
/// (Z negate on positions and normals, V flip on texture coordinates,
/// per-triangle corner reverse). Mirrors vpinball's
/// `ObjLoader::Load`'s `convertToLeftHanded` flag.
///
/// `convert_to_left_handed = true` matches the existing assemble path
/// (input is in vpinball's exported convention - i.e. the same form
/// `extract` writes - and we convert to vpx-internal coordinates).
/// `false` skips the conversion: the input is assumed to already be in
/// vpx-internal convention so its values pass through unchanged.
pub(crate) fn read_obj_from_reader_with_options<R: BufRead>(
    mut reader: &mut R,
    convert_to_left_handed: bool,
) -> io::Result<ReadObjResult> {
    let ObjData {
        name,
        vertices,
        texture_coordinates,
        normals,
        indices,
    } = read_obj_with_options(&mut reader, convert_to_left_handed)
        .map_err(|e| io::Error::other(format!("Error reading obj: {}", e)))?;

    let mut final_vertices = Vec::with_capacity(vertices.len());
    let mut vpx_encoded_vertices = BytesMut::with_capacity(vertices.len() * 32);
    for ((v, vt), vn) in vertices
        .iter()
        .zip(texture_coordinates.iter())
        .zip(normals.iter())
    {
        // vpinball's ObjLoader::Load (with `convertToLeftHanded=true`)
        // negates Z on vertices and normals and flips V on texcoords.
        // With the flag false those transforms are skipped.
        // The V flip is done in f64, recovering the f32 tv the write side
        // flipped bit-for-bit (see flipped_v_text).
        let (z, nz, tv) = if convert_to_left_handed {
            (-v.2, -vn.z, (1.0 - vt.1.unwrap_or(0.0)) as f32)
        } else {
            (v.2, vn.z, vt.1.unwrap_or(0.0) as f32)
        };

        let vertext = Vertex3dNoTex2 {
            x: v.0,
            y: v.1,
            z,
            nx: vn.x,
            ny: vn.y,
            nz,
            tu: vt.0,
            tv,
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

fn write_vertex(buff: &mut BytesMut, vertex: &Vertex3dNoTex2, vpx_bytes: &Option<VpxCommentBytes>) {
    if let Some(VpxCommentBytes::Vertex(bytes)) = vpx_bytes {
        // the full original vertex was preserved in a comment
        buff.put_slice(bytes);
        return;
    }
    buff.put_f32_le(vertex.x);
    buff.put_f32_le(vertex.y);
    buff.put_f32_le(vertex.z);
    // normals
    if let Some(VpxCommentBytes::Normal(bytes)) = vpx_bytes {
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
    /// V is kept at f64 precision so the `1 - v` flip back to vpx tv is exact.
    texture_coordinates: Vec<(f32, Option<f64>, Option<f32>)>,
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
    /// list is built directly. With `reverse_corners=true` the per-triangle
    /// corner order is reversed to undo vpinball's `WriteFaceInfoLong`
    /// reversal; with `false` the source winding is preserved.
    ///
    /// Otherwise (Blender-style n-gons or mismatched corners), the
    /// [`triangulate_and_dedup`] step runs over the collected data; the
    /// same `reverse_corners` flag controls whether faces are reversed
    /// before fan-triangulation. The v/vt/vn arrays are rebuilt to be
    /// aligned (same length, one entry per combined corner).
    fn read<R: io::Read>(mut self, reader: &mut R, reverse_corners: bool) -> io::Result<ObjData> {
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
                reverse_corners,
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
            // matching v/vt/vn. With `reverse_corners=true` we flip the
            // per-triangle corner order to undo vpinball's
            // `WriteFaceInfoLong` reversal; with `false` we preserve the
            // source winding for callers importing OBJ files already in
            // vpinball-internal convention.
            let indices = self
                .raw_faces
                .iter()
                .map(|face| {
                    if reverse_corners {
                        VpxFace {
                            i0: face[2].v as i64 - 1,
                            i1: face[1].v as i64 - 1,
                            i2: face[0].v as i64 - 1,
                        }
                    } else {
                        VpxFace {
                            i0: face[0].v as i64 - 1,
                            i1: face[1].v as i64 - 1,
                            i2: face[2].v as i64 - 1,
                        }
                    }
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

// Floats are parsed at f64 precision: narrowing an f64 parsed from the
// shortest round-trip text of an f32 yields that f32 again, and the V
// texture coordinate needs the extra precision for the exact flip back
// to vpx tv (see flipped_v_text).
impl ObjReader<f64> for VpxObjReader {
    fn read_comment(&mut self, comment: &str) {
        self.previous_comment = Some(comment.to_string());
    }

    fn read_object_name(&mut self, name: &str) {
        self.object_count += 1;
        self.name = name.to_string();
        self.previous_comment = None;
    }

    fn read_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) {
        self.vertices
            .push((x as f32, y as f32, z as f32, w.map(|w| w as f32)));
        self.previous_comment = None;
    }

    fn read_texture_coordinate(&mut self, u: f64, v: Option<f64>, w: Option<f64>) {
        self.texture_coordinates
            .push((u as f32, v, w.map(|w| w as f32)));
        self.previous_comment = None;
    }

    fn read_normal(&mut self, nx: f64, ny: f64, nz: f64) {
        let (nx, ny, nz) = (nx as f32, ny as f32, nz as f32);
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
    read_obj_with_options(&mut reader, true)
}

/// Like [`read_obj`] but lets the caller decide whether per-triangle
/// face winding gets reversed. `reverse_corners=true` matches vpinball's
/// `ObjLoader::Load` (which always reverses); `false` preserves the
/// source's winding for callers that import OBJ files that are already
/// in vpinball-internal convention.
pub(crate) fn read_obj_with_options<R: BufRead>(
    reader: &mut R,
    reverse_corners: bool,
) -> std::io::Result<ObjData> {
    VpxObjReader::new().read(reader, reverse_corners)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VpxObjNormal {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    // in case the vertex had NaN or otherwise unrepresentable values, the
    // original vpx bytes from the preceding `# vpx <hex>` comment go here
    vpx_bytes: Option<VpxCommentBytes>,
}
impl VpxObjNormal {
    fn new(x: f32, y: f32, z: f32, vpx_bytes: Option<VpxCommentBytes>) -> Self {
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
    /// V is kept at f64 precision so the `1 - v` flip back to vpx tv is exact.
    pub texture_coordinates: Vec<(f32, Option<f64>, Option<f32>)>,
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
            texture_coordinates: vec![(2.0f32, Some(4.0f64), None)],
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
            texture_coordinates: vec![(2.0f32, Some(4.0f64), None)],
            normals: vec![
                VpxObjNormal::new(
                    f32::NAN,
                    1.0f32,
                    0.0f32,
                    Some(VpxCommentBytes::Normal([
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                    ])),
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

    /// Vertices with values the obj text cannot represent (NaN payload bits
    /// anywhere, tv values that cannot survive the V flip) must round-trip
    /// bit-exact via the full-vertex `# vpx <hex>` comment.
    #[test]
    fn test_unrepresentable_vertex_bytes_roundtrip() -> TestResult {
        fn encode(v: &Vertex3dNoTex2) -> [u8; 32] {
            let mut b = [0u8; 32];
            for (i, f) in [v.x, v.y, v.z, v.nx, v.ny, v.nz, v.tu, v.tv]
                .iter()
                .enumerate()
            {
                b[i * 4..i * 4 + 4].copy_from_slice(&f.to_le_bytes());
            }
            b
        }
        let base = Vertex3dNoTex2 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            nx: 0.0,
            ny: 1.0,
            nz: 0.0,
            tu: 0.5,
            tv: 0.25,
        };
        let vertices: Vec<VertexWrapper> = [
            // negative zero tv (1 - v can never produce it)
            Vertex3dNoTex2 { tv: -0.0, ..base },
            // tv below the flipped representation's information limit
            Vertex3dNoTex2 {
                tv: f32::from_bits(0x22c00000), // 5.2e-18
                ..base
            },
            // NaN with non-canonical payload bits in tu
            Vertex3dNoTex2 {
                tu: f32::from_bits(0xffc00123),
                ..base
            },
            // NaN with non-canonical payload bits in tv
            Vertex3dNoTex2 {
                tv: f32::from_bits(0x7fa00001),
                ..base
            },
            // NaN position
            Vertex3dNoTex2 {
                x: f32::from_bits(0xffa00042),
                ..base
            },
            // control vertex, needs no preservation
            base,
        ]
        .into_iter()
        .map(|vertex| VertexWrapper {
            vpx_encoded_vertex: encode(&vertex),
            vertex,
        })
        .collect();
        let indices = vec![VpxFace::new(0, 1, 2)];

        let mut buffer = Vec::new();
        write_obj_to_writer("nan_test", &vertices, &indices, &mut buffer)?;
        let obj_text = String::from_utf8(buffer.clone())?;
        assert_eq!(
            obj_text.matches("# vpx ").count(),
            5,
            "expected a byte comment for each unrepresentable vertex:\n{obj_text}"
        );

        let mut reader = BufReader::new(buffer.as_slice());
        let read_result = read_obj_from_reader(&mut reader)?;
        let original_bytes: Vec<u8> = vertices.iter().flat_map(|v| v.vpx_encoded_vertex).collect();
        assert_eq!(
            read_result.vpx_encoded_vertices.as_ref(),
            original_bytes.as_slice()
        );
        Ok(())
    }

    /// The V flip (`obj_v = 1 - tv`) must recover tv bit-for-bit through a
    /// write/read cycle. In plain f32 most tv values in (0, 0.5) come back
    /// an ulp off, which broke bit-exact extract/assemble round-trips.
    #[test]
    fn test_tv_flip_roundtrip_bit_exact() -> TestResult {
        let mut tvs: Vec<f32> = vec![
            0.0,
            1.0,
            0.5,
            0.25,
            0.75,
            // values that do not survive a plain f32 double flip
            0.001,
            0.123_456_79,
            1.0e-3,
            4.2e-7,
            // information limit: smallest tv the flipped format can hold
            2.0f32.powi(-30),
            // UV tiling values outside [0, 1]
            -0.265_023,
            5.3,
            -4.2,
        ];
        // a dense sweep through the f32 range (0, 1)
        let mut tv = 1.0e-9f32;
        while tv < 1.0 {
            tvs.push(tv);
            tv = f32::from_bits(tv.to_bits() + 99_991);
        }

        let vertices: Vec<VertexWrapper> = tvs
            .iter()
            .map(|&tv| {
                let vertex = Vertex3dNoTex2 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    nx: 0.0,
                    ny: 1.0,
                    nz: 0.0,
                    tu: 0.5,
                    tv,
                };
                let mut bytes = BytesMut::new();
                write_vertex(&mut bytes, &vertex, &None);
                VertexWrapper {
                    vpx_encoded_vertex: bytes.as_ref().try_into().unwrap(),
                    vertex,
                }
            })
            .collect();
        let indices = vec![VpxFace::new(0, 1, 2)];

        let mut buffer = Vec::new();
        write_obj_to_writer("tv_test", &vertices, &indices, &mut buffer)?;
        let mut reader = BufReader::new(buffer.as_slice());
        let read_result = read_obj_from_reader(&mut reader)?;

        assert_eq!(read_result.final_vertices.len(), tvs.len());
        for (read, &tv) in read_result.final_vertices.iter().zip(tvs.iter()) {
            assert_eq!(
                read.tv.to_bits(),
                tv.to_bits(),
                "tv {tv:e} not recovered bit-exact, got {:e}",
                read.tv
            );
        }
        Ok(())
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
                let tv = (1.0 - vt.1.unwrap_or(0.0)) as f32;

                // Write position bytes (0-11)
                let mut cursor = std::io::Cursor::new(&mut bytes[0..12]);
                cursor.write_f32::<LittleEndian>(v.0).unwrap();
                cursor.write_f32::<LittleEndian>(v.1).unwrap();
                cursor.write_f32::<LittleEndian>(z).unwrap();

                // Write normal bytes (12-23)
                // If we have VPX bytes from OBJ, use them, otherwise encode the floats
                if let Some(VpxCommentBytes::Vertex(vpx_bytes)) = &vn.vpx_bytes {
                    bytes.copy_from_slice(vpx_bytes);
                } else if let Some(VpxCommentBytes::Normal(vpx_bytes)) = &vn.vpx_bytes {
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
        let bytes: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let comment = obj_vpx_comment(&bytes);
        let parsed = obj_parse_vpx_comment(&comment).unwrap();
        assert_eq!(VpxCommentBytes::Normal(bytes), parsed);
    }

    #[test]
    fn test_write_read_vpx_comment_full_vertex() {
        let bytes: [u8; 32] = std::array::from_fn(|i| i as u8);
        let comment = obj_vpx_comment(&bytes);
        let parsed = obj_parse_vpx_comment(&comment).unwrap();
        assert_eq!(VpxCommentBytes::Vertex(bytes), parsed);
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
