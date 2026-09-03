use std::cell::RefCell;
use std::path::Path;
use wasm_bindgen::prelude::*;

use crate::filesystem::{FileSystem, MemoryFileSystem};
use crate::vpx;
use crate::vpx::expanded::{ExpandOptions, PrimitiveMeshFormat, read_fs, write_fs};
use crate::vpx::units::AxisConvention;

thread_local! {
    static PROGRESS_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

fn set_progress_callback(callback: Option<js_sys::Function>) {
    PROGRESS_CALLBACK.with(|cb| {
        *cb.borrow_mut() = callback;
    });
}

fn emit_progress(message: &str) {
    PROGRESS_CALLBACK.with(|cb| {
        if let Some(callback) = cb.borrow().as_ref() {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(message));
        }
    });
}

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn extract(data: &[u8], callback: Option<js_sys::Function>) -> Result<js_sys::Object, JsError> {
    set_progress_callback(callback);

    emit_progress("Parsing VPX file...");
    let vpx_data = vpx::from_bytes(data).map_err(|e| {
        set_progress_callback(None);
        JsError::new(&e.to_string())
    })?;

    let fs = MemoryFileSystem::new();
    let root_dir = "/vpx".to_string();

    emit_progress(&format!("Extracting {} images...", vpx_data.images.len()));
    emit_progress(&format!("Extracting {} sounds...", vpx_data.sounds.len()));
    emit_progress(&format!(
        "Extracting {} game items...",
        vpx_data.gameitems.len()
    ));

    let expand_options = ExpandOptions::new()
        .mesh_format(PrimitiveMeshFormat::Obj)
        .generate_derived_meshes(false);
    write_fs(&vpx_data, &root_dir, &expand_options, &fs).map_err(|e| {
        set_progress_callback(None);
        JsError::new(&format!("Failed to extract VPX: {}", e))
    })?;

    emit_progress("Building file map...");
    let result = js_sys::Object::new();
    for path in fs.list_files() {
        if let Some(data) = fs.get_file(&path) {
            let key = JsValue::from_str(&path);
            let value = js_sys::Uint8Array::from(data.as_slice());
            js_sys::Reflect::set(&result, &key, &value).map_err(|e| {
                set_progress_callback(None);
                JsError::new(&format!("Failed to set file in result: {:?}", e))
            })?;
        }
    }

    emit_progress("Extraction complete");
    set_progress_callback(None);

    Ok(result)
}

#[wasm_bindgen]
pub fn assemble(
    files: js_sys::Object,
    callback: Option<js_sys::Function>,
) -> Result<Vec<u8>, JsError> {
    set_progress_callback(callback);

    emit_progress("Reading files...");
    let fs = MemoryFileSystem::new();
    let keys = js_sys::Object::keys(&files);

    for i in 0..keys.length() {
        let key = keys.get(i);
        let path = key
            .as_string()
            .ok_or_else(|| JsError::new("Invalid file path"))?;

        let value = js_sys::Reflect::get(&files, &key).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&format!("Failed to get file: {:?}", e))
        })?;

        let array = js_sys::Uint8Array::from(value);
        let data = array.to_vec();

        fs.write_file(Path::new(&path), &data).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&format!("Failed to write file to memory: {}", e))
        })?;
    }

    emit_progress("Assembling VPX...");
    let root_dir = "/vpx".to_string();
    let vpx_data = read_fs(&root_dir, &fs).map_err(|e| {
        set_progress_callback(None);
        JsError::new(&format!("Failed to assemble VPX: {}", e))
    })?;

    emit_progress(&format!("Assembling {} images...", vpx_data.images.len()));
    emit_progress(&format!("Assembling {} sounds...", vpx_data.sounds.len()));
    emit_progress(&format!(
        "Assembling {} game items...",
        vpx_data.gameitems.len()
    ));

    emit_progress("Writing VPX data...");
    let bytes = vpx::to_bytes(&vpx_data).map_err(|e| {
        set_progress_callback(None);
        JsError::new(&e.to_string())
    })?;

    emit_progress("Assembly complete");
    set_progress_callback(None);

    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Mesh I/O surface for vpx-editor and other wasm consumers.
//
// `obj_to_mesh` parses an OBJ into renderer-ready typed arrays;
// `mesh_to_obj` is its symmetric inverse. Both take an optional options
// object (a plain JS literal, see `MeshIoOptions`) with an
// `AxisConvention` naming the convention of the OBJ side (the mesh side
// is always vpx-internal) plus a `unitScale` multiplier for positions:
// - `ZDownRightHanded` (default): vpinball's exported OBJ convention
//   (Z negate / V flip / winding reverse). This is what the old
//   `convert_to_left_handed = true` did, what vpinball's "Convert
//   coordinate system" mesh-import checkbox does, and what `extract` /
//   `assemble` write and read.
// - `YUpRightHanded`: the Blender / DCC default (Y-Z swap / V flip /
//   winding reverse), so a mesh opens upright in Blender with default
//   import settings.
// - `ZUpLeftHanded`: vpx-internal values verbatim, no transforms. This
//   is what the old `convert_to_left_handed = false` did.
// ---------------------------------------------------------------------------

#[wasm_bindgen(typescript_custom_section)]
const MESH_IO_OPTIONS_TS: &'static str = r#"
/**
 * Options for `obj_to_mesh` / `mesh_to_obj`. Pass a plain object
 * literal; every field is optional.
 */
export interface MeshIoOptions {
    /**
     * Axis convention of the OBJ side of the conversion (the mesh side
     * is always vpx-internal). Default: `AxisConvention.ZDownRightHanded`,
     * matching what `extract` writes and `assemble` reads.
     */
    axes?: AxisConvention;
    /**
     * Multiplier applied to positions (on the vpx side for
     * `obj_to_mesh`, before the axis mapping for `mesh_to_obj`).
     * Normals and texture coordinates are never scaled. An OBJ written
     * with scale `k` reads back with `1 / k`. Default: `1.0`.
     */
    unitScale?: number;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// Duck-typed options object for [`obj_to_mesh`] / [`mesh_to_obj`];
    /// the shape is declared by the `MeshIoOptions` TypeScript interface
    /// above.
    #[wasm_bindgen(typescript_type = "MeshIoOptions")]
    pub type MeshIoOptions;
}

/// The `MeshIoOptions` TypeScript interface as seen by serde. Field
/// names go through `rename_all`, so they must match the interface
/// above. Unknown fields on the JS object are ignored, per JS options
/// convention (serde-wasm-bindgen reads known fields by name and never
/// sees the others, so `deny_unknown_fields` would have no effect).
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MeshIoOptionsData {
    axes: Option<AxisConvention>,
    unit_scale: Option<f32>,
}

/// `MeshIoOptions` with defaults applied.
struct ResolvedMeshIoOptions {
    axes: AxisConvention,
    unit_scale: f32,
}

fn parse_mesh_io_options(options: Option<MeshIoOptions>) -> Result<ResolvedMeshIoOptions, JsError> {
    let data = match &options {
        Some(options) => {
            let js: &JsValue = options.as_ref();
            if js.is_undefined() || js.is_null() {
                MeshIoOptionsData::default()
            } else {
                serde_wasm_bindgen::from_value(js.clone())
                    .map_err(|e| JsError::new(&format!("Invalid options: {}", e)))?
            }
        }
        None => MeshIoOptionsData::default(),
    };
    let unit_scale = data.unit_scale.unwrap_or(1.0);
    if !unit_scale.is_finite() {
        return Err(JsError::new("options.unitScale must be a finite number"));
    }
    Ok(ResolvedMeshIoOptions {
        axes: data.axes.unwrap_or(AxisConvention::ZDownRightHanded),
        unit_scale,
    })
}

/// Mesh data for a single primitive: positions, texture coordinates,
/// normals and triangle indices, packed as flat typed arrays for direct
/// upload into a WebGL / Three.js / GPU buffer.
///
/// All vertex data is aligned: `positions[3*i..3*i+3]`, `tex_coords[2*i..2*i+2]`
/// and `normals[3*i..3*i+3]` describe corner `i`. Triangles are 0-based
/// indices into that aligned array.
///
/// Coordinates are in vpx-internal convention (the same form `read_fs`
/// produces and `write_fs` consumes), not raw OBJ values - see
/// [`obj_to_mesh`] / [`mesh_to_obj`] for the transform details.
///
/// The published wasm package is built with `wasm-bindgen --weak-refs`,
/// so the Rust-owned vectors backing this struct are reclaimed
/// automatically via `FinalizationRegistry` when the JS wrapper is
/// garbage-collected. Calling `.free()` manually is still allowed for
/// deterministic cleanup of large meshes.
#[wasm_bindgen]
pub struct PrimitiveMesh {
    name: String,
    positions: Vec<f32>,
    tex_coords: Vec<f32>,
    normals: Vec<f32>,
    indices: Vec<u32>,
}

#[wasm_bindgen]
impl PrimitiveMesh {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn positions(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.positions.as_slice())
    }

    #[wasm_bindgen(getter, js_name = texCoords)]
    pub fn tex_coords(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.tex_coords.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn normals(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.normals.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> js_sys::Uint32Array {
        js_sys::Uint32Array::from(self.indices.as_slice())
    }

    /// Bounding-box midpoint of the mesh's positions, in the same
    /// coordinate space as `positions` (vpx-internal). Returns
    /// `[mid_x, mid_y, mid_z]`. Used by editor flows that center the
    /// mesh on origin or move the primitive to the mesh's absolute
    /// position - both need to know the midpoint to shift vertices
    /// (and, for the absolute-position case, to set the primitive's
    /// `vPosition` field). Mirrors vpinball's `Mesh::middlePoint`,
    /// which is used by `IDC_CENTER_MESH` / `IDC_ABS_POSITION_RADIO`
    /// in the mesh-import dialog (`primitive.cpp:1729-1745`).
    ///
    /// Returns `[0, 0, 0]` for an empty mesh.
    #[wasm_bindgen(getter)]
    pub fn midpoint(&self) -> js_sys::Float32Array {
        if self.positions.is_empty() {
            return js_sys::Float32Array::from([0.0_f32, 0.0, 0.0].as_slice());
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for chunk in self.positions.as_chunks::<3>().0 {
            for axis in 0..3 {
                if chunk[axis] < min[axis] {
                    min[axis] = chunk[axis];
                }
                if chunk[axis] > max[axis] {
                    max[axis] = chunk[axis];
                }
            }
        }
        let mid = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        js_sys::Float32Array::from(mid.as_slice())
    }
}

/// Parse a Wavefront OBJ into a [`PrimitiveMesh`].
///
/// Accepts any OBJ flavor (vpinball-format from `extract`, Blender-format,
/// anything in between): n-gons are fan-triangulated and `(position, uv,
/// normal)` corners are deduplicated so the result is renderer-ready.
///
/// `options.axes` names the convention the OBJ data is in; the returned
/// mesh is always in vpx-internal convention:
///
/// - [`AxisConvention::ZDownRightHanded`] (the default; matches
///   `assemble`'s read path and vpinball's mesh-import dialog with
///   "Convert coordinate system" checked, the old `convert_to_left_handed
///   = true`): vertex and normal Z are negated, V is flipped (`vpx_tv =
///   1 - obj_v`) and the per-triangle corner order is reversed. This path
///   also honors the `# vpx <hex>` byte-preservation comments, so
///   vpinball-format OBJs from `extract` reproduce the original vpx
///   values bit-for-bit.
/// - [`AxisConvention::YUpRightHanded`] (Blender / DCC default export
///   settings): Y and Z are swapped, V is flipped and winding is
///   reversed.
/// - [`AxisConvention::ZUpLeftHanded`] (the old `convert_to_left_handed
///   = false`): the input is assumed to already hold vpx-internal values
///   (e.g. produced by a previous `mesh_to_obj` with the same
///   convention). No transforms; values pass through verbatim.
///
/// `options.unitScale` (default `1.0`) multiplies positions after the
/// axis mapping. Normals and texture coordinates are never scaled. A
/// mesh exported through `mesh_to_obj` with scale `k` reads back with
/// scale `1.0 / k`.
#[wasm_bindgen]
pub fn obj_to_mesh(data: &[u8], options: Option<MeshIoOptions>) -> Result<PrimitiveMesh, JsError> {
    use crate::vpx::obj::read_obj_from_reader_with_options;
    use std::io::BufReader;

    let ResolvedMeshIoOptions { axes, unit_scale } = parse_mesh_io_options(options)?;

    // ZDownRightHanded is the reader's own built-in conversion; it must go
    // through that path because only there the `# vpx <hex>`
    // byte-preservation sidecars are applied (see the notes on
    // `read_obj_from_reader_with_options`). The other conventions read raw
    // and transform below.
    let built_in_convert = axes == AxisConvention::ZDownRightHanded;
    let mut reader = BufReader::new(data);
    let result = read_obj_from_reader_with_options(&mut reader, built_in_convert)
        .map_err(|e| JsError::new(&format!("OBJ parse failed: {}", e)))?;

    // When the reader already converted, the data is in vpx space and the
    // remaining mapping is the identity.
    let map_axes = if built_in_convert {
        AxisConvention::ZUpLeftHanded
    } else {
        axes
    };
    let flip_v = map_axes != AxisConvention::ZUpLeftHanded;
    let reverse_winding = map_axes.flips_handedness(AxisConvention::ZUpLeftHanded);

    let mut positions = Vec::with_capacity(result.final_vertices.len() * 3);
    let mut tex_coords = Vec::with_capacity(result.final_vertices.len() * 2);
    let mut normals = Vec::with_capacity(result.final_vertices.len() * 3);
    for v in &result.final_vertices {
        let [x, y, z] = map_axes.to_vpx(v.x, v.y, v.z);
        positions.push(x * unit_scale);
        positions.push(y * unit_scale);
        positions.push(z * unit_scale);
        tex_coords.push(v.tu);
        tex_coords.push(if flip_v { 1.0 - v.tv } else { v.tv });
        let [nx, ny, nz] = map_axes.to_vpx(v.nx, v.ny, v.nz);
        normals.push(nx);
        normals.push(ny);
        normals.push(nz);
    }
    let mut indices = Vec::with_capacity(result.indices.len() * 3);
    for face in &result.indices {
        if reverse_winding {
            indices.push(face.i2 as u32);
            indices.push(face.i1 as u32);
            indices.push(face.i0 as u32);
        } else {
            indices.push(face.i0 as u32);
            indices.push(face.i1 as u32);
            indices.push(face.i2 as u32);
        }
    }

    Ok(PrimitiveMesh {
        name: result.name,
        positions,
        tex_coords,
        normals,
        indices,
    })
}

/// Serialize a mesh as a Wavefront OBJ.
///
/// `name` becomes the `o` directive; pass an empty string to use
/// `"object"`. Vertex / texcoord / normal arrays must have aligned
/// lengths (`positions.len() / 3 == tex_coords.len() / 2 ==
/// normals.len() / 3`); index values must be valid 0-based offsets into
/// that vertex array.
///
/// `options.axes` is the symmetric inverse of the same option on
/// [`obj_to_mesh`]: the input is always vpx-internal data, `axes` names
/// the convention to write the OBJ in.
///
/// - [`AxisConvention::ZDownRightHanded`] (the default; matches
///   `extract`'s write path, the old `convert_to_left_handed = true`):
///   vertex and normal Z are negated, V is flipped (`obj_v = 1 -
///   vpx_tv`) and per-triangle corner order is reversed. The result is a
///   vpinball-format OBJ that `assemble` (or `obj_to_mesh` with the same
///   convention) reads back identically.
/// - [`AxisConvention::YUpRightHanded`]: Y and Z are swapped, V is
///   flipped and winding is reversed; the OBJ opens upright in Blender
///   with default import settings.
/// - [`AxisConvention::ZUpLeftHanded`] (the old `convert_to_left_handed
///   = false`): the vpx-internal data is written out verbatim, no
///   transforms applied.
///
/// `options.unitScale` (default `1.0`) multiplies positions before the
/// axis mapping. Normals and texture coordinates are never scaled.
/// Round-trips through `obj_to_mesh` with the same convention and scale
/// `1.0 / unitScale`.
#[wasm_bindgen]
pub fn mesh_to_obj(
    name: &str,
    positions: &[f32],
    tex_coords: &[f32],
    normals: &[f32],
    indices: &[u32],
    options: Option<MeshIoOptions>,
) -> Result<Vec<u8>, JsError> {
    use wavefront_obj_io::{IoObjWriter, ObjWriter};

    let ResolvedMeshIoOptions { axes, unit_scale } = parse_mesh_io_options(options)?;

    if !positions.len().is_multiple_of(3) {
        return Err(JsError::new("positions length must be a multiple of 3"));
    }
    if !tex_coords.len().is_multiple_of(2) {
        return Err(JsError::new("tex_coords length must be a multiple of 2"));
    }
    if !normals.len().is_multiple_of(3) {
        return Err(JsError::new("normals length must be a multiple of 3"));
    }
    if !indices.len().is_multiple_of(3) {
        return Err(JsError::new("indices length must be a multiple of 3"));
    }
    let vert_count = positions.len() / 3;
    if tex_coords.len() / 2 != vert_count || normals.len() / 3 != vert_count {
        return Err(JsError::new(
            "positions / tex_coords / normals must describe the same vertex count",
        ));
    }

    let mut buffer = Vec::with_capacity(positions.len() * 4);
    {
        let mut writer: IoObjWriter<&mut Vec<u8>, f32> = IoObjWriter::new(&mut buffer);
        writer
            .write_comment(format!(
                "numVerts: {} numFaces: {}",
                vert_count,
                indices.len() / 3
            ))
            .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        let object_name = if name.is_empty() { "object" } else { name };
        writer
            .write_object_name(object_name)
            .map_err(|e| JsError::new(&format!("write failed: {e}")))?;

        let flip_v = axes != AxisConvention::ZUpLeftHanded;
        let reverse_winding = axes.flips_handedness(AxisConvention::ZUpLeftHanded);

        for chunk in positions.as_chunks::<3>().0 {
            let [x, y, z] = axes.from_vpx(
                chunk[0] * unit_scale,
                chunk[1] * unit_scale,
                chunk[2] * unit_scale,
            );
            writer
                .write_vertex(x, y, z, None)
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
        if flip_v {
            // The flipped V value may need more precision than the f32 obj
            // writer can provide, so these lines are written manually, see
            // flipped_v_text. Round-trips with `obj_to_mesh` at the same
            // convention.
            drop(writer);
            for chunk in tex_coords.as_chunks::<2>().0 {
                use std::io::Write;
                writeln!(
                    buffer,
                    "vt {} {}",
                    chunk[0],
                    crate::vpx::obj::flipped_v(chunk[1]).text
                )
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
            }
            writer = IoObjWriter::new(&mut buffer);
        } else {
            for chunk in tex_coords.as_chunks::<2>().0 {
                writer
                    .write_texture_coordinate(chunk[0], Some(chunk[1]), None)
                    .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
            }
        }
        for chunk in normals.as_chunks::<3>().0 {
            let [nx, ny, nz] = axes.from_vpx(chunk[0], chunk[1], chunk[2]);
            writer
                .write_normal(nx, ny, nz)
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
        // OBJ indices are 1-based. When the conversion flips handedness
        // (both right-handed conventions) we reverse the per-triangle
        // corner order (matching vpinball's `WriteFaceInfoLong`); for
        // ZUpLeftHanded we keep source winding so round-trips preserve
        // indices.
        for tri in indices.as_chunks::<3>().0 {
            for &idx in tri {
                if idx as usize >= vert_count {
                    return Err(JsError::new(&format!(
                        "triangle index {idx} out of range (have {vert_count} vertices)"
                    )));
                }
            }
            let (a, b, c) = if reverse_winding {
                (
                    (tri[2] + 1) as usize,
                    (tri[1] + 1) as usize,
                    (tri[0] + 1) as usize,
                )
            } else {
                (
                    (tri[0] + 1) as usize,
                    (tri[1] + 1) as usize,
                    (tri[2] + 1) as usize,
                )
            };
            writer
                .write_face(&[
                    (a, Some(a), Some(a)),
                    (b, Some(b), Some(b)),
                    (c, Some(c), Some(c)),
                ])
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
    }

    Ok(buffer)
}

/// Generate the procedural mesh used by vpinball primitives that
/// don't load a `.obj` file (`use_3d_mesh = false`). Mirrors
/// `Primitive::CalculateBuiltinOriginal` from vpinball: a regular
/// polygon prism with `sides` faces, top and bottom caps, fitting
/// in `[-r, r] x [-r, r] x [-0.5, 0.5]`.
///
/// Use this to render the placeholder shape for a primitive that
/// has `use_3d_mesh = false`, or to seed the editor's "Add
/// Primitive" workflow.
///
/// `sides` must be at least 3; otherwise the call errors out
/// (vpinball clamps to 3 in its own editor).
///
/// `draw_textures_inside = true` doubles the index count so back
/// faces are also rendered (matches vpinball's flag of the same
/// name on `Primitive`). Vertex / texcoord / normal arrays are
/// unaffected.
#[wasm_bindgen]
pub fn generate_builtin_primitive(
    sides: u32,
    draw_textures_inside: bool,
) -> Result<PrimitiveMesh, JsError> {
    use crate::vpx::mesh::builtin_primitive::build_builtin_primitive_mesh;

    let (vertices, faces) = build_builtin_primitive_mesh(sides, draw_textures_inside)
        .ok_or_else(|| JsError::new(&format!("sides must be >= 3 (got {sides})")))?;

    let mut positions = Vec::with_capacity(vertices.len() * 3);
    let mut tex_coords = Vec::with_capacity(vertices.len() * 2);
    let mut normals = Vec::with_capacity(vertices.len() * 3);
    for vw in &vertices {
        let v = &vw.vertex;
        positions.push(v.x);
        positions.push(v.y);
        positions.push(v.z);
        tex_coords.push(v.tu);
        tex_coords.push(v.tv);
        normals.push(v.nx);
        normals.push(v.ny);
        normals.push(v.nz);
    }
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for face in &faces {
        indices.push(face.i0 as u32);
        indices.push(face.i1 as u32);
        indices.push(face.i2 as u32);
    }

    Ok(PrimitiveMesh {
        name: String::from("primitive"),
        positions,
        tex_coords,
        normals,
        indices,
    })
}

#[cfg(all(test, target_family = "wasm"))]
mod tests {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;

    /// Build a `MeshIoOptions` object the way a JS caller would: a plain
    /// object literal with optional fields.
    fn options(axes: Option<AxisConvention>, unit_scale: Option<f32>) -> MeshIoOptions {
        let obj = js_sys::Object::new();
        if let Some(axes) = axes {
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("axes"),
                &JsValue::from_f64(axes as u32 as f64),
            )
            .unwrap();
        }
        if let Some(scale) = unit_scale {
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("unitScale"),
                &JsValue::from_f64(scale as f64),
            )
            .unwrap();
        }
        obj.unchecked_into()
    }

    #[wasm_bindgen_test]
    fn test_extract_with_invalid_data() {
        let invalid_data = b"invalid vpx data";
        let result = extract(invalid_data, None);
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_assemble_with_empty_files() {
        let files = js_sys::Object::new();
        let result = assemble(files, None);
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_obj_to_mesh_blender_cube() {
        // Blender's default cube exported as OBJ. 6 quads -> 12 triangles,
        // 8 unique positions but 24 unique combined corners after dedup
        // (each quad has its own normal, so no corner can be reused
        // across adjacent faces).
        let blender = include_bytes!("../testdata/blender_square.obj");
        // No options: defaults to ZDownRightHanded / scale 1.0.
        let mesh = obj_to_mesh(blender, None).expect("parse should succeed");
        assert_eq!(mesh.name(), "Cube");

        let positions = mesh.positions();
        let tex_coords = mesh.tex_coords();
        let normals = mesh.normals();
        let indices = mesh.indices();

        // 24 combined corners across 12 triangles.
        assert_eq!(positions.length(), 24 * 3);
        assert_eq!(tex_coords.length(), 24 * 2);
        assert_eq!(normals.length(), 24 * 3);
        assert_eq!(indices.length(), 12 * 3);
    }

    #[wasm_bindgen_test]
    fn test_obj_to_mesh_rejects_unparseable_input() {
        let result = obj_to_mesh(b"this is not an obj", None);
        // The lenient reader skips unknown lines; this fails on the
        // post-parse "no vertices" check.
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_mesh_to_obj_round_trip() {
        // obj_to_mesh -> mesh_to_obj -> obj_to_mesh: structure preserved.
        let blender = include_bytes!("../testdata/blender_square.obj");
        let mesh = obj_to_mesh(blender, None).expect("parse should succeed");

        let positions: Vec<f32> = mesh.positions().to_vec();
        let tex_coords: Vec<f32> = mesh.tex_coords().to_vec();
        let normals: Vec<f32> = mesh.normals().to_vec();
        let indices: Vec<u32> = mesh.indices().to_vec();

        let obj_bytes = mesh_to_obj("Cube", &positions, &tex_coords, &normals, &indices, None)
            .expect("write should succeed");

        let round_tripped = obj_to_mesh(&obj_bytes, None).expect("reparse should succeed");
        assert_eq!(round_tripped.positions().length(), positions.len() as u32);
        assert_eq!(round_tripped.tex_coords().length(), tex_coords.len() as u32);
        assert_eq!(round_tripped.normals().length(), normals.len() as u32);
        assert_eq!(round_tripped.indices().length(), indices.len() as u32);
    }

    #[wasm_bindgen_test]
    fn test_mesh_to_obj_round_trip_y_up() {
        // YUpRightHanded round trip: vpx-internal data written out as a
        // Blender-convention OBJ and read back. Values chosen so the V
        // flip (1 - v) is exact in f32.
        let positions: Vec<f32> = vec![0.0, 0.0, 0.5, 1.0, 0.0, 0.5, 0.0, 1.0, 0.5];
        let tex_coords: Vec<f32> = vec![0.0, 0.25, 1.0, 0.25, 0.0, 0.75];
        let normals: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices: Vec<u32> = vec![0, 1, 2];

        let obj_bytes = mesh_to_obj(
            "tri",
            &positions,
            &tex_coords,
            &normals,
            &indices,
            Some(options(Some(AxisConvention::YUpRightHanded), None)),
        )
        .expect("write should succeed");

        // The written OBJ is in Y-up space: vpx (0, 1, 0.5) -> obj (0, 0.5, 1).
        let text = String::from_utf8(obj_bytes.clone()).expect("obj should be utf-8");
        assert!(text.contains("v 0 0.5 1"), "obj should be Y-up:\n{text}");

        let parsed = obj_to_mesh(
            &obj_bytes,
            Some(options(Some(AxisConvention::YUpRightHanded), None)),
        )
        .expect("reparse should succeed");
        assert_eq!(parsed.positions().to_vec(), positions);
        assert_eq!(parsed.tex_coords().to_vec(), tex_coords);
        assert_eq!(parsed.normals().to_vec(), normals);
        assert_eq!(parsed.indices().to_vec(), indices);
    }

    #[wasm_bindgen_test]
    fn test_unit_scale_round_trip() {
        // Export with scale k, import with 1/k: positions restored;
        // normals and tex coords never scaled.
        let positions: Vec<f32> = vec![0.0, 0.0, 0.5, 1.0, 0.0, 0.5, 0.0, 1.0, 0.5];
        let tex_coords: Vec<f32> = vec![0.0, 0.25, 1.0, 0.25, 0.0, 0.75];
        let normals: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices: Vec<u32> = vec![0, 1, 2];

        let obj_bytes = mesh_to_obj(
            "tri",
            &positions,
            &tex_coords,
            &normals,
            &indices,
            Some(options(Some(AxisConvention::YUpRightHanded), Some(4.0))),
        )
        .expect("write should succeed");

        let text = String::from_utf8(obj_bytes.clone()).expect("obj should be utf-8");
        assert!(
            text.contains("v 0 2 4"),
            "positions should be scaled:\n{text}"
        );

        let parsed = obj_to_mesh(
            &obj_bytes,
            Some(options(Some(AxisConvention::YUpRightHanded), Some(0.25))),
        )
        .expect("reparse should succeed");
        assert_eq!(parsed.positions().to_vec(), positions);
        assert_eq!(parsed.tex_coords().to_vec(), tex_coords);
        assert_eq!(parsed.normals().to_vec(), normals);
    }

    #[wasm_bindgen_test]
    fn test_mesh_to_obj_round_trip_no_convert() {
        // With `ZUpLeftHanded`, vpx-internal data passes through
        // verbatim - both vertex Z and triangle indices keep their
        // original values across a round trip.
        let positions: Vec<f32> = vec![0.0, 0.0, 0.5, 1.0, 0.0, 0.5, 0.0, 1.0, 0.5];
        let tex_coords: Vec<f32> = vec![0.0, 0.25, 1.0, 0.25, 0.0, 0.75];
        let normals: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices: Vec<u32> = vec![0, 1, 2];

        let obj_bytes = mesh_to_obj(
            "tri",
            &positions,
            &tex_coords,
            &normals,
            &indices,
            Some(options(Some(AxisConvention::ZUpLeftHanded), None)),
        )
        .expect("write should succeed");

        let parsed = obj_to_mesh(
            &obj_bytes,
            Some(options(Some(AxisConvention::ZUpLeftHanded), None)),
        )
        .expect("reparse should succeed");
        let parsed_positions: Vec<f32> = parsed.positions().to_vec();
        let parsed_tex_coords: Vec<f32> = parsed.tex_coords().to_vec();
        let parsed_indices: Vec<u32> = parsed.indices().to_vec();

        assert_eq!(parsed_positions, positions);
        assert_eq!(parsed_tex_coords, tex_coords);
        assert_eq!(parsed_indices, indices);
    }

    #[wasm_bindgen_test]
    fn test_mesh_to_obj_validates_aligned_arrays() {
        // 3 positions but only 2 tex coords - should error.
        let result = mesh_to_obj(
            "bad",
            &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            &[0, 1, 2],
            None,
        );
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_options_validation() {
        let data = include_bytes!("../testdata/blender_square.obj");

        // An axes value outside the enum is rejected.
        let bad = js_sys::Object::new();
        js_sys::Reflect::set(&bad, &JsValue::from_str("axes"), &JsValue::from_f64(99.0)).unwrap();
        assert!(obj_to_mesh(data, Some(bad.unchecked_into())).is_err());

        // An empty options object means all defaults.
        let empty = js_sys::Object::new();
        assert!(obj_to_mesh(data, Some(empty.unchecked_into())).is_ok());
    }

    #[wasm_bindgen_test]
    fn test_generate_builtin_primitive_shape() {
        // A 4-sided builtin primitive: 4*4+2 = 18 vertices,
        // 12*4 = 48 indices = 16 triangles. With
        // draw_textures_inside, indices double to 32 triangles.
        let mesh = generate_builtin_primitive(4, false).expect("should succeed");
        assert_eq!(mesh.positions().length(), 18 * 3);
        assert_eq!(mesh.tex_coords().length(), 18 * 2);
        assert_eq!(mesh.normals().length(), 18 * 3);
        assert_eq!(mesh.indices().length(), 16 * 3);

        let mesh = generate_builtin_primitive(4, true).expect("should succeed");
        assert_eq!(mesh.indices().length(), 32 * 3);
    }

    #[wasm_bindgen_test]
    fn test_generate_builtin_primitive_rejects_too_few_sides() {
        for sides in 0..3 {
            let result = generate_builtin_primitive(sides, false);
            assert!(result.is_err(), "sides={sides} should error");
        }
    }

    #[wasm_bindgen_test]
    fn test_generate_builtin_primitive_round_trips_via_mesh_to_obj() {
        // The builtin mesh feeds straight into mesh_to_obj; the
        // resulting OBJ parses back to the same vertex/index counts.
        let mesh = generate_builtin_primitive(8, false).expect("should succeed");
        let positions: Vec<f32> = mesh.positions().to_vec();
        let tex_coords: Vec<f32> = mesh.tex_coords().to_vec();
        let normals: Vec<f32> = mesh.normals().to_vec();
        let indices: Vec<u32> = mesh.indices().to_vec();

        let obj_bytes = mesh_to_obj(
            "octa",
            &positions,
            &tex_coords,
            &normals,
            &indices,
            Some(options(Some(AxisConvention::ZUpLeftHanded), None)),
        )
        .expect("write should succeed");
        let parsed = obj_to_mesh(
            &obj_bytes,
            Some(options(Some(AxisConvention::ZUpLeftHanded), None)),
        )
        .expect("reparse should succeed");
        assert_eq!(parsed.positions().to_vec(), positions);
        assert_eq!(parsed.indices().to_vec(), indices);
    }

    #[wasm_bindgen_test]
    fn test_extract() {
        let original_data = include_bytes!("../testdata/completely_blank_table_10_7_4.vpx");
        let extract_result = extract(original_data, None).expect("Extraction failed");
        assert_eq!(95, js_sys::Object::keys(&extract_result).length());
        // print all keys
        // to see the results use:
        // cargo test --target wasm32-unknown-unknown --features wasm -- --nocapture
        let keys = js_sys::Object::keys(&extract_result);
        for i in 0..keys.length() {
            let key = keys.get(i);
            let key_str = key.as_string().unwrap();
            web_sys::console::log_1(&JsValue::from_str(&key_str));
        }
        let version_key = JsValue::from_str("/vpx/version.txt");
        let version_value = js_sys::Reflect::get(&extract_result, &version_key).unwrap();
        let version_array = js_sys::Uint8Array::from(version_value);
        let version_str = String::from_utf8(version_array.to_vec()).unwrap();
        assert_eq!("1072", version_str);
    }

    #[wasm_bindgen_test]
    fn test_assemble() {
        let original_data = include_bytes!("../testdata/completely_blank_table_10_7_4.vpx");
        let extract_result = extract(original_data, None).expect("Extraction failed");

        let assemble_result = assemble(extract_result.clone(), None).expect("Assembly failed");

        let extract_result2 = extract(&assemble_result, None).expect("Re-extraction failed");
        // compare key count
        assert_eq!(
            js_sys::Object::keys(&extract_result).length(),
            js_sys::Object::keys(&extract_result2).length()
        );
        // compare all keys and values one by one
        let keys = js_sys::Object::keys(&extract_result);
        for i in 0..keys.length() {
            let key = keys.get(i);
            let original_value = js_sys::Reflect::get(&extract_result, &key).unwrap();
            let reassembled_value = js_sys::Reflect::get(&extract_result2, &key).unwrap();
            let original_array = js_sys::Uint8Array::from(original_value);
            let reassembled_array = js_sys::Uint8Array::from(reassembled_value);
            assert_eq!(
                original_array.length(),
                reassembled_array.length(),
                "Mismatched length for key {:?}",
                key
            );
            let original_bytes = original_array.to_vec();
            let reassembled_bytes = reassembled_array.to_vec();
            assert_eq!(
                original_bytes, reassembled_bytes,
                "Mismatched content for key {:?}",
                key
            );
        }
    }
}
