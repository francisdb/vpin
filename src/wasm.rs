use std::cell::RefCell;
use std::path::Path;
use wasm_bindgen::prelude::*;

use crate::filesystem::{FileSystem, MemoryFileSystem};
use crate::vpx;
use crate::vpx::expanded::{PrimitiveMeshFormat, read_fs, write_fs};

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

    write_fs(&vpx_data, &root_dir, PrimitiveMeshFormat::Obj, &fs).map_err(|e| {
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
