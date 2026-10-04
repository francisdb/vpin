//! glTF constants shared by the exporters

// TODO switch to using gltf crate for reading / writing?

pub(crate) const GLTF_MAGIC: &[u8; 4] = b"glTF";
pub(crate) const GLTF_VERSION: u32 = 2;
pub(crate) const GLB_HEADER_BYTES: u32 = 12;
pub(crate) const GLB_CHUNK_HEADER_BYTES: u32 = 8;
pub(crate) const GLB_JSON_CHUNK_TYPE: &[u8; 4] = b"JSON";
pub(crate) const GLB_BIN_CHUNK_TYPE: &[u8; 4] = b"BIN\0";
pub(crate) const GLTF_PRIMITIVE_MODE_TRIANGLES: u32 = 4;
pub(crate) const GLTF_COMPONENT_TYPE_FLOAT: u32 = 5126;
pub(crate) const GLTF_COMPONENT_TYPE_UNSIGNED_SHORT: u32 = 5123;
pub(crate) const GLTF_COMPONENT_TYPE_UNSIGNED_INT: u32 = 5125;
pub(crate) const GLTF_TARGET_ARRAY_BUFFER: u32 = 34962;
pub(crate) const GLTF_TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

// Sampler filter modes (from OpenGL ES 2.0)
pub(crate) const GLTF_FILTER_LINEAR: u32 = 9729;
pub(crate) const GLTF_FILTER_LINEAR_MIPMAP_LINEAR: u32 = 9987;

// Sampler wrap modes (from OpenGL ES 2.0)
pub(crate) const GLTF_WRAP_REPEAT: u32 = 10497;
