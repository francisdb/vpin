use std::cell::RefCell;
use std::path::Path;
use wasm_bindgen::prelude::*;

use crate::filesystem::{FileSystem, MemoryFileSystem};
use crate::vpx::expanded::{read_fs, write_fs};
use crate::vpx;

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

    write_fs(&vpx_data, &root_dir, &fs).map_err(|e| {
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
pub fn assemble(files: js_sys::Object, callback: Option<js_sys::Function>) -> Result<Vec<u8>, JsError> {
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
