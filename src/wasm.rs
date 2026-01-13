use std::cell::RefCell;
use std::io::Cursor;
use std::path::Path;
use wasm_bindgen::prelude::*;

use crate::filesystem::MemoryFileSystem;
use crate::vpx::expanded::{read_fs, vpx_image_to_dynamic_image, write_fs};
use crate::vpx::font::FontDataJson;
use crate::vpx::gamedata::GameDataJson;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::image::ImageDataJson;
use crate::vpx::jsonmodel::{collections_json, info_to_json};
use crate::vpx::material::MaterialJson;
use crate::vpx::renderprobe::RenderProbeJson;
use crate::vpx::sound::{SoundDataJson, write_sound};
use crate::vpx::{self, VPX};
use crate::wavefront_obj_io;

thread_local! {
    static PROGRESS_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

pub fn set_progress_callback(callback: Option<js_sys::Function>) {
    PROGRESS_CALLBACK.with(|cb| {
        *cb.borrow_mut() = callback;
    });
}

pub fn emit_progress(message: &str) {
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
pub struct VpxFile {
    vpx: VPX,
}

#[wasm_bindgen]
impl VpxFile {
    #[wasm_bindgen(constructor)]
    pub fn from_bytes(data: &[u8]) -> Result<VpxFile, JsError> {
        let vpx = vpx::from_bytes(data).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(VpxFile { vpx })
    }

    #[wasm_bindgen]
    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        let buffer = std::io::Cursor::new(Vec::new());
        let mut comp =
            cfb::CompoundFile::create(buffer).map_err(|e| JsError::new(&e.to_string()))?;
        vpx::write_vpx(&mut comp, &self.vpx).map_err(|e| JsError::new(&e.to_string()))?;
        comp.flush().map_err(|e| JsError::new(&e.to_string()))?;
        let cursor = comp.into_inner();
        Ok(cursor.into_inner())
    }

    #[wasm_bindgen]
    pub fn get_version(&self) -> u32 {
        self.vpx.version.u32()
    }

    #[wasm_bindgen]
    pub fn get_version_string(&self) -> String {
        self.vpx.version.to_u32_string()
    }

    #[wasm_bindgen]
    pub fn get_table_name(&self) -> Option<String> {
        self.vpx.info.table_name.clone()
    }

    #[wasm_bindgen]
    pub fn get_author_name(&self) -> Option<String> {
        self.vpx.info.author_name.clone()
    }

    #[wasm_bindgen]
    pub fn get_table_description(&self) -> Option<String> {
        self.vpx.info.table_description.clone()
    }

    #[wasm_bindgen]
    pub fn get_script(&self) -> String {
        self.vpx.gamedata.code.string.clone()
    }

    #[wasm_bindgen]
    pub fn set_script(&mut self, script: String) {
        self.vpx.gamedata.set_code(script);
    }

    #[wasm_bindgen]
    pub fn get_gamedata_json(&self) -> Result<String, JsError> {
        let json = GameDataJson::from_game_data(&self.vpx.gamedata);
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_info_json(&self) -> Result<String, JsError> {
        let json = info_to_json(&self.vpx.info, &self.vpx.custominfotags);
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_collections_json(&self) -> Result<String, JsError> {
        let json = collections_json(&self.vpx.collections);
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_gameitems_json(&self) -> Result<String, JsError> {
        serde_json::to_string_pretty(&self.vpx.gameitems).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn set_gameitems_json(&mut self, json: &str) -> Result<(), JsError> {
        let gameitems = serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;
        self.vpx.gameitems = gameitems;
        self.vpx.gamedata.gameitems_size = self.vpx.gameitems.len() as u32;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_images_json(&self) -> Result<String, JsError> {
        let json: Vec<ImageDataJson> = self
            .vpx
            .images
            .iter()
            .map(ImageDataJson::from_image_data)
            .collect();
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_image_data(&self, index: usize) -> Result<Vec<u8>, JsError> {
        let image = self
            .vpx
            .images
            .get(index)
            .ok_or_else(|| JsError::new(&format!("Image index {} out of bounds", index)))?;

        if image.is_link() {
            return Ok(Vec::new());
        }

        if let Some(jpeg) = &image.jpeg {
            return Ok(jpeg.data.clone());
        }

        if let Some(bits) = &image.bits {
            let dynamic_image =
                vpx_image_to_dynamic_image(&bits.lzw_compressed_data, image.width, image.height);
            let mut buffer = std::io::Cursor::new(Vec::new());
            dynamic_image
                .write_to(&mut buffer, image::ImageFormat::Bmp)
                .map_err(|e| JsError::new(&e.to_string()))?;
            return Ok(buffer.into_inner());
        }

        Ok(Vec::new())
    }

    #[wasm_bindgen]
    pub fn get_sounds_json(&self) -> Result<String, JsError> {
        let json: Vec<SoundDataJson> = self
            .vpx
            .sounds
            .iter()
            .map(SoundDataJson::from_sound_data)
            .collect();
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_sound_data(&self, index: usize) -> Result<Vec<u8>, JsError> {
        let sound = self
            .vpx
            .sounds
            .get(index)
            .ok_or_else(|| JsError::new(&format!("Sound index {} out of bounds", index)))?;
        Ok(write_sound(sound))
    }

    #[wasm_bindgen]
    pub fn get_fonts_json(&self) -> Result<String, JsError> {
        let json: Vec<FontDataJson> = self
            .vpx
            .fonts
            .iter()
            .map(FontDataJson::from_font_data)
            .collect();
        serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn get_font_data(&self, index: usize) -> Result<Vec<u8>, JsError> {
        let font = self
            .vpx
            .fonts
            .get(index)
            .ok_or_else(|| JsError::new(&format!("Font index {} out of bounds", index)))?;
        Ok(font.data.clone())
    }

    #[wasm_bindgen]
    pub fn get_materials_json(&self) -> Result<String, JsError> {
        if let Some(materials) = &self.vpx.gamedata.materials {
            let json: Vec<MaterialJson> =
                materials.iter().map(MaterialJson::from_material).collect();
            serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
        } else {
            Ok("[]".to_string())
        }
    }

    #[wasm_bindgen]
    pub fn get_renderprobes_json(&self) -> Result<String, JsError> {
        if let Some(probes) = &self.vpx.gamedata.render_probes {
            let json: Vec<RenderProbeJson> = probes
                .iter()
                .map(RenderProbeJson::from_renderprobe)
                .collect();
            serde_json::to_string_pretty(&json).map_err(|e| JsError::new(&e.to_string()))
        } else {
            Ok("[]".to_string())
        }
    }

    #[wasm_bindgen]
    pub fn get_screenshot(&self) -> Option<Vec<u8>> {
        self.vpx.info.screenshot.clone()
    }

    #[wasm_bindgen]
    pub fn gameitems_count(&self) -> usize {
        self.vpx.gameitems.len()
    }

    #[wasm_bindgen]
    pub fn images_count(&self) -> usize {
        self.vpx.images.len()
    }

    #[wasm_bindgen]
    pub fn sounds_count(&self) -> usize {
        self.vpx.sounds.len()
    }

    #[wasm_bindgen]
    pub fn fonts_count(&self) -> usize {
        self.vpx.fonts.len()
    }

    #[wasm_bindgen]
    pub fn collections_count(&self) -> usize {
        self.vpx.collections.len()
    }

    #[wasm_bindgen]
    pub fn get_primitive_obj(&self, index: usize) -> Result<String, JsError> {
        use wavefront_obj_io::ObjWriter;

        let item = self
            .vpx
            .gameitems
            .get(index)
            .ok_or_else(|| JsError::new(&format!("Gameitem index {} out of bounds", index)))?;

        if let GameItemEnum::Primitive(primitive) = item {
            let mesh_opt = primitive
                .read_mesh()
                .map_err(|e| JsError::new(&format!("Failed to read mesh: {}", e)))?;

            let mesh = match mesh_opt {
                Some(m) => m,
                None => return Ok(String::new()),
            };

            let mut buffer = Vec::new();
            {
                let cursor = Cursor::new(&mut buffer);
                let mut writer = std::io::BufWriter::new(cursor);
                let mut obj_writer = wavefront_obj_io::IoObjWriter::new(&mut writer);

                let _ = obj_writer.write_comment(format!("Primitive: {}", primitive.name));
                let _ = obj_writer.write_object_name(&primitive.name);

                for (_, vertex) in &mesh.vertices {
                    let _ = obj_writer.write_vertex(
                        vertex.x as f64,
                        vertex.y as f64,
                        -(vertex.z as f64),
                        None,
                    );
                }

                for (_, vertex) in &mesh.vertices {
                    let _ = obj_writer.write_normal(
                        vertex.nx as f64,
                        vertex.ny as f64,
                        -(vertex.nz as f64),
                    );
                }

                for (_, vertex) in &mesh.vertices {
                    let _ = obj_writer.write_texture_coordinate(
                        vertex.tu as f64,
                        Some(vertex.tv as f64),
                        None,
                    );
                }

                for face in mesh.indices.chunks(3) {
                    if face.len() == 3 {
                        let i1 = (face[0] + 1) as usize;
                        let i2 = (face[1] + 1) as usize;
                        let i3 = (face[2] + 1) as usize;
                        let face_indices = vec![
                            (i1, Some(i1), Some(i1)),
                            (i2, Some(i2), Some(i2)),
                            (i3, Some(i3), Some(i3)),
                        ];
                        let _ = obj_writer.write_face(&face_indices);
                    }
                }
            }

            String::from_utf8(buffer)
                .map_err(|e| JsError::new(&format!("Failed to convert OBJ to string: {}", e)))
        } else {
            Ok(String::new())
        }
    }
}

#[wasm_bindgen]
pub fn get_vpx_info(data: &[u8]) -> Result<String, JsError> {
    let vpx = vpx::from_bytes(data).map_err(|e| JsError::new(&e.to_string()))?;
    let info = serde_json::json!({
        "version": vpx.version.u32(),
        "table_name": vpx.info.table_name,
        "author_name": vpx.info.author_name,
        "table_version": vpx.info.table_version,
        "table_description": vpx.info.table_description,
        "gameitems_count": vpx.gameitems.len(),
        "images_count": vpx.images.len(),
        "sounds_count": vpx.sounds.len(),
        "collections_count": vpx.collections.len(),
    });
    Ok(info.to_string())
}

#[wasm_bindgen]
pub struct VpxExtracted {
    fs: MemoryFileSystem,
    root_dir: String,
}

#[wasm_bindgen]
impl VpxExtracted {
    #[wasm_bindgen(constructor)]
    pub fn from_bytes(data: &[u8]) -> Result<VpxExtracted, JsError> {
        Self::from_bytes_with_callback(data, None)
    }

    #[wasm_bindgen]
    pub fn from_bytes_with_callback(
        data: &[u8],
        callback: Option<js_sys::Function>,
    ) -> Result<VpxExtracted, JsError> {
        set_progress_callback(callback);

        emit_progress("Parsing VPX file...");
        let vpx = vpx::from_bytes(data).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&e.to_string())
        })?;

        let fs = MemoryFileSystem::new();
        let root_dir = "/vpx".to_string();

        emit_progress(&format!("Extracting {} images...", vpx.images.len()));
        emit_progress(&format!("Extracting {} sounds...", vpx.sounds.len()));
        emit_progress(&format!("Extracting {} game items...", vpx.gameitems.len()));

        write_fs(&vpx, &root_dir, &fs).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&format!("Failed to extract VPX: {}", e))
        })?;

        emit_progress("Extraction complete");
        set_progress_callback(None);

        Ok(VpxExtracted { fs, root_dir })
    }

    #[wasm_bindgen]
    pub fn list_files(&self) -> Vec<String> {
        self.fs.list_files()
    }

    #[wasm_bindgen]
    pub fn get_file(&self, path: &str) -> Result<Vec<u8>, JsError> {
        self.fs
            .get_file(path)
            .ok_or_else(|| JsError::new(&format!("File not found: {}", path)))
    }

    #[wasm_bindgen]
    pub fn get_file_string(&self, path: &str) -> Result<String, JsError> {
        let data = self.get_file(path)?;
        String::from_utf8(data)
            .map_err(|e| JsError::new(&format!("Failed to decode file as UTF-8: {}", e)))
    }

    #[wasm_bindgen]
    pub fn set_file(&self, path: &str, data: &[u8]) -> Result<(), JsError> {
        use crate::filesystem::FileSystem;
        self.fs
            .write_file(Path::new(path), data)
            .map_err(|e| JsError::new(&format!("Failed to write file: {}", e)))
    }

    #[wasm_bindgen]
    pub fn set_file_string(&self, path: &str, content: &str) -> Result<(), JsError> {
        self.set_file(path, content.as_bytes())
    }

    #[wasm_bindgen]
    pub fn delete_file(&self, path: &str) {
        self.fs.delete_file(path);
    }

    #[wasm_bindgen]
    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        self.to_bytes_with_callback(None)
    }

    #[wasm_bindgen]
    pub fn to_bytes_with_callback(
        &self,
        callback: Option<js_sys::Function>,
    ) -> Result<Vec<u8>, JsError> {
        set_progress_callback(callback);

        emit_progress("Reading table data...");
        let vpx = read_fs(&self.root_dir, &self.fs).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&format!("Failed to assemble VPX: {}", e))
        })?;

        emit_progress(&format!("Assembling {} images...", vpx.images.len()));
        emit_progress(&format!("Assembling {} sounds...", vpx.sounds.len()));
        emit_progress(&format!("Assembling {} game items...", vpx.gameitems.len()));

        emit_progress("Creating VPX compound file...");
        let buffer = std::io::Cursor::new(Vec::new());
        let mut comp = cfb::CompoundFile::create(buffer).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&e.to_string())
        })?;

        emit_progress("Writing VPX data...");
        vpx::write_vpx(&mut comp, &vpx).map_err(|e| {
            set_progress_callback(None);
            JsError::new(&e.to_string())
        })?;

        comp.flush().map_err(|e| {
            set_progress_callback(None);
            JsError::new(&e.to_string())
        })?;

        emit_progress("Save complete");
        set_progress_callback(None);

        let cursor = comp.into_inner();
        Ok(cursor.into_inner())
    }

    #[wasm_bindgen]
    pub fn root_dir(&self) -> String {
        self.root_dir.clone()
    }
}
