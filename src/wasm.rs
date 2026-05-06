use std::cell::RefCell;
use std::path::Path;
use wasm_bindgen::prelude::*;

use crate::filesystem::{FileSystem, MemoryFileSystem};
use crate::vpx;
use crate::vpx::expanded::{ExpandOptions, PrimitiveMeshFormat, read_fs, write_fs};

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
// Symmetric with `extract` / `assemble`: `obj_to_mesh` applies the same
// vpinball-format transforms that the read path applies (Z negate, V flip,
// winding reverse) so the returned mesh data is in vpx-internal convention.
// `mesh_to_obj` applies the inverse, producing an OBJ in vpinball-format
// suitable for `assemble`.
// ---------------------------------------------------------------------------

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
}

/// Parse a Wavefront OBJ into a [`PrimitiveMesh`]. Accepts any OBJ flavor
/// Parse a Wavefront OBJ into a [`PrimitiveMesh`].
///
/// Accepts any OBJ flavor (vpinball-format from `extract`, Blender-format,
/// anything in between): n-gons are fan-triangulated and `(position, uv,
/// normal)` corners are deduplicated so the result is renderer-ready.
///
/// Applies the same transforms as `assemble`'s read path: vertex Z is
/// negated, normal Z is negated, V coordinate is flipped (`vpx_tv = 1 -
/// obj_v`), and the per-triangle corner order is reversed. The returned
/// mesh data is in vpx-internal convention.
#[wasm_bindgen]
pub fn obj_to_mesh(data: &[u8]) -> Result<PrimitiveMesh, JsError> {
    use crate::vpx::obj::read_obj_from_reader;
    use std::io::BufReader;

    let mut reader = BufReader::new(data);
    let result = read_obj_from_reader(&mut reader)
        .map_err(|e| JsError::new(&format!("OBJ parse failed: {}", e)))?;

    let mut positions = Vec::with_capacity(result.final_vertices.len() * 3);
    let mut tex_coords = Vec::with_capacity(result.final_vertices.len() * 2);
    let mut normals = Vec::with_capacity(result.final_vertices.len() * 3);
    for v in &result.final_vertices {
        positions.push(v.x);
        positions.push(v.y);
        positions.push(v.z);
        tex_coords.push(v.tu);
        tex_coords.push(v.tv);
        normals.push(v.nx);
        normals.push(v.ny);
        normals.push(v.nz);
    }
    let mut indices = Vec::with_capacity(result.indices.len() * 3);
    for face in &result.indices {
        indices.push(face.i0 as u32);
        indices.push(face.i1 as u32);
        indices.push(face.i2 as u32);
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
/// Applies the same transforms as `extract`'s write path: vertex Z is
/// negated, normal Z is negated, V coordinate is flipped (`obj_v = 1 -
/// vpx_tv`), and per-triangle corner order is reversed. The result is a
/// vpinball-format OBJ that `assemble` can read back into the same vpx
/// data via the inverse transforms.
#[wasm_bindgen]
pub fn mesh_to_obj(
    name: &str,
    positions: &[f32],
    tex_coords: &[f32],
    normals: &[f32],
    indices: &[u32],
) -> Result<Vec<u8>, JsError> {
    use wavefront_obj_io::{IoObjWriter, ObjWriter};

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

        // Z negate on positions.
        for chunk in positions.chunks_exact(3) {
            writer
                .write_vertex(chunk[0], chunk[1], -chunk[2], None)
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
        // V flip on tex coords.
        for chunk in tex_coords.chunks_exact(2) {
            writer
                .write_texture_coordinate(chunk[0], Some(1.0 - chunk[1]), None)
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
        // Z negate on normals.
        for chunk in normals.chunks_exact(3) {
            writer
                .write_normal(chunk[0], chunk[1], -chunk[2])
                .map_err(|e| JsError::new(&format!("write failed: {e}")))?;
        }
        // Per-triangle corner reverse on faces; OBJ indices are 1-based.
        for tri in indices.chunks_exact(3) {
            for &idx in tri {
                if idx as usize >= vert_count {
                    return Err(JsError::new(&format!(
                        "triangle index {idx} out of range (have {vert_count} vertices)"
                    )));
                }
            }
            let a = (tri[2] + 1) as usize;
            let b = (tri[1] + 1) as usize;
            let c = (tri[0] + 1) as usize;
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

#[cfg(all(test, target_family = "wasm"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

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
        let mesh = obj_to_mesh(blender).expect("parse should succeed");
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
        let result = obj_to_mesh(b"this is not an obj");
        // The lenient reader skips unknown lines; this fails on the
        // post-parse "no vertices" check.
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_mesh_to_obj_round_trip() {
        // obj_to_mesh -> mesh_to_obj -> obj_to_mesh: structure preserved.
        let blender = include_bytes!("../testdata/blender_square.obj");
        let mesh = obj_to_mesh(blender).expect("parse should succeed");

        let positions: Vec<f32> = mesh.positions().to_vec();
        let tex_coords: Vec<f32> = mesh.tex_coords().to_vec();
        let normals: Vec<f32> = mesh.normals().to_vec();
        let indices: Vec<u32> = mesh.indices().to_vec();

        let obj_bytes = mesh_to_obj("Cube", &positions, &tex_coords, &normals, &indices)
            .expect("write should succeed");

        let round_tripped = obj_to_mesh(&obj_bytes).expect("reparse should succeed");
        assert_eq!(round_tripped.positions().length(), positions.len() as u32);
        assert_eq!(round_tripped.tex_coords().length(), tex_coords.len() as u32);
        assert_eq!(round_tripped.normals().length(), normals.len() as u32);
        assert_eq!(round_tripped.indices().length(), indices.len() as u32);
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
        );
        assert!(result.is_err());
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
