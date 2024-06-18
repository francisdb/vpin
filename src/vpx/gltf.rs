use crate::vpx::expanded::ReadMesh;
use gltf::json;
use gltf::json::validation::Checked::Valid;
use gltf::json::validation::USize64;
use std::borrow::Cow;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::mem;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Output {
    /// Output standard glTF.
    Standard,

    /// Output binary glTF.
    Binary,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

/// Calculate bounding coordinates of a list of vertices, used for the clipping distance of the model
fn bounding_coords(points: &[Vertex]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for point in points {
        let p = point.position;
        for i in 0..3 {
            min[i] = f32::min(min[i], p[i]);
            max[i] = f32::max(max[i], p[i]);
        }
    }
    (min, max)
}

fn align_to_multiple_of_four(n: &mut usize) {
    *n = (*n + 3) & !3;
}

fn to_padded_byte_vector<T>(vec: Vec<T>) -> Vec<u8> {
    let byte_length = vec.len() * mem::size_of::<T>();
    let byte_capacity = vec.capacity() * mem::size_of::<T>();
    let alloc = vec.into_boxed_slice();
    let ptr = Box::<[T]>::into_raw(alloc) as *mut u8;
    let mut new_vec = unsafe { Vec::from_raw_parts(ptr, byte_length, byte_capacity) };
    while new_vec.len() % 4 != 0 {
        new_vec.push(0); // pad to multiple of four bytes
    }
    new_vec
}

pub(crate) fn write_gltf(
    name: String,
    mesh: &ReadMesh,
    gltf_file_path: &PathBuf,
    output: Output,
    image_rel_path: &str,
) -> Result<(), Box<dyn Error>> {
    let bin_path = gltf_file_path.with_extension("bin");

    // use the indices to look up the vertices
    let vertices = mesh
        .indices
        .iter()
        .map(|i| {
            let v = &mesh.vertices[*i as usize];
            Vertex {
                position: [v.vertex.x, v.vertex.y, v.vertex.z],
                normal: [v.vertex.nx, v.vertex.ny, v.vertex.nz],
                uv: [v.vertex.tu, v.vertex.tv],
            }
        })
        .collect::<Vec<Vertex>>();

    let (min, max) = bounding_coords(&vertices);

    let mut root = json::Root::default();

    let buffer_length = vertices.len() * mem::size_of::<Vertex>();
    let buffer = root.push(json::Buffer {
        byte_length: USize64::from(buffer_length),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: if output == Output::Standard {
            let path: String = bin_path
                .file_name()
                .expect("Invalid file name")
                .to_str()
                .expect("Invalid file name")
                .to_string();
            Some(path.into())
        } else {
            None
        },
    });
    let buffer_view = root.push(json::buffer::View {
        buffer,
        byte_length: USize64::from(buffer_length),
        byte_offset: None,
        byte_stride: Some(json::buffer::Stride(mem::size_of::<Vertex>())),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    });
    let positions = root.push(json::Accessor {
        buffer_view: Some(buffer_view),
        byte_offset: Some(USize64(0)),
        count: USize64::from(vertices.len()),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec3),
        min: Some(json::Value::from(Vec::from(min))),
        max: Some(json::Value::from(Vec::from(max))),
        name: None,
        normalized: false,
        sparse: None,
    });
    let normals = root.push(json::Accessor {
        buffer_view: Some(buffer_view),
        // we have to skip the first 3 floats to get to the normals
        byte_offset: Some(USize64::from(3 * mem::size_of::<f32>())),
        count: USize64::from(vertices.len()),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec3),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    let tex_coords = root.push(json::Accessor {
        buffer_view: Some(buffer_view),
        // we have to skip the first 5 floats to get to the texture coordinates
        byte_offset: Some(USize64::from(6 * mem::size_of::<f32>())),
        count: USize64::from(vertices.len()),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec2),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    let image = root.push(json::Image {
        buffer_view: None,
        uri: Some(image_rel_path.to_string()),
        mime_type: None,
        name: Some("gottlieb_flipper_red".to_string()),
        extensions: None,
        extras: Default::default(),
    });

    let sampler = root.push(json::texture::Sampler {
        mag_filter: None,
        min_filter: None,
        wrap_s: Valid(json::texture::WrappingMode::Repeat),
        wrap_t: Valid(json::texture::WrappingMode::Repeat),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
    });

    let texture = root.push(json::Texture {
        sampler: Some(sampler),
        source: image,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
    });

    let material = root.push(json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: texture,
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            }),
            // base_color_factor: PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
            // metallic_factor: StrengthFactor(1.0),
            // roughness_factor: StrengthFactor(1.0),
            // metallic_roughness_texture: None,
            // extensions: Default::default(),
            // extras: Default::default(),
            ..Default::default()
        },
        // normal_texture: None,
        // occlusion_texture: None,
        // emissive_texture: None,
        // emissive_factor: EmissiveFactor([0.0, 0.0, 0.0]),
        // alpha_mode: Valid(json::material::AlphaMode::Opaque),
        // alpha_cutoff: Some(AlphaCutoff(0.5)),
        // double_sided: false,
        // extensions: Default::default(),
        // extras: Default::default(),
        name: Some("material1".to_string()),
        ..Default::default()
    });

    let primitive = json::mesh::Primitive {
        material: Some(material),
        attributes: {
            let mut map = std::collections::BTreeMap::new();
            map.insert(Valid(json::mesh::Semantic::Positions), positions);
            //map.insert(Valid(json::mesh::Semantic::Colors(0)), colors);
            map.insert(Valid(json::mesh::Semantic::Normals), normals);
            map.insert(Valid(json::mesh::Semantic::TexCoords(0)), tex_coords);
            map
        },
        extensions: Default::default(),
        extras: Default::default(),
        indices: None,
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    };

    let mesh = root.push(json::Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        primitives: vec![primitive],
        weights: None,
    });

    let node = root.push(json::Node {
        mesh: Some(mesh),
        name: Some(name),
        ..Default::default()
    });

    root.push(json::Scene {
        extensions: Default::default(),
        extras: Default::default(),
        name: Some("table1".to_string()),
        nodes: vec![node],
    });

    match output {
        Output::Standard => {
            let writer = File::create(gltf_file_path)?;
            json::serialize::to_writer_pretty(writer, &root)?;
            let bin = to_padded_byte_vector(vertices);
            let mut writer = File::create(bin_path)?;
            writer.write_all(&bin)?;
        }
        Output::Binary => {
            let json_string = json::serialize::to_string(&root)?;
            let mut json_offset = json_string.len();
            align_to_multiple_of_four(&mut json_offset);
            let glb = gltf::binary::Glb {
                header: gltf::binary::Header {
                    magic: *b"glTF",
                    version: 2,
                    // N.B., the size of binary glTF file is limited to range of `u32`.
                    length: (json_offset + buffer_length)
                        .try_into()
                        .expect("file size exceeds binary glTF limit"),
                },
                bin: Some(Cow::Owned(to_padded_byte_vector(vertices))),
                json: Cow::Owned(json_string.into_bytes()),
            };
            let glb_path = gltf_file_path.with_extension("glb");
            let writer = std::fs::File::create(glb_path)?;
            glb.to_writer(writer)?;
        }
    }
    Ok(())
}
