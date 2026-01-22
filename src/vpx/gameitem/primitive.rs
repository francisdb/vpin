use super::vertex3d::Vertex3D;
use crate::vpx::gameitem::select::WriteSharedAttributes;

use crate::vpx::expanded::WriteError;
use crate::vpx::model::Vertex3dNoTex2;

use crate::impl_shared_attributes;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use bytes::{Buf, BufMut, BytesMut};
use flate2::read::ZlibDecoder;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::{self, Read};

const BYTES_PER_VERTEX: usize = 32;

/// when there are more than 65535 vertices we use 4 bytes per index value
/// TODO make private
pub const MAX_VERTICES_FOR_2_BYTE_INDEX: usize = 65535;

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Primitive {
    pub position: Vertex3D, // 0 VPOS
    pub size: Vertex3D,     // 1 VSIZ
    /// Indices for RotAndTra:
    ///      RotX = 0
    ///      RotY = 1
    ///      RotZ = 2
    ///      TraX = 3
    ///      TraY = 4
    ///      TraZ = 5
    ///   ObjRotX = 6
    ///   ObjRotY = 7
    ///   ObjRotZ = 8
    pub rot_and_tra: [f32; 9], // 2-11 RTV0-RTV8
    pub image: String,      // 12 IMAG
    pub normal_map: Option<String>, // 13 NRMA (added in 10.?)
    pub sides: u32,         // 14
    pub name: String,       // 15
    pub material: String,   // 16
    pub side_color: Color,  // 17
    pub is_visible: bool,   // 18
    pub draw_textures_inside: bool, // 19
    pub hit_event: bool,    // 20
    pub threshold: f32,     // 21
    pub elasticity: f32,    // 22
    pub elasticity_falloff: f32, // 23
    pub friction: f32,      // 24
    pub scatter: f32,       // 25
    pub edge_factor_ui: f32, // 26
    pub collision_reduction_factor: Option<f32>, // 27 CORF (was missing in 10.01)
    pub is_collidable: bool, // 28
    pub is_toy: bool,       // 29
    pub use_3d_mesh: bool,  // 30
    pub static_rendering: bool, // 31
    pub disable_lighting_top_old: Option<f32>, // DILI (removed in 10.8)
    pub disable_lighting_top: Option<f32>, // DILT (added in 10.8)
    pub disable_lighting_below: Option<f32>, // 33 DILB (added in 10.?)
    pub is_reflection_enabled: Option<bool>, // 34 REEN (was missing in 10.01)
    pub backfaces_enabled: Option<bool>, // 35 EBFC (added in 10.?)
    pub physics_material: Option<String>, // 36 MAPH (added in 10.?)
    pub overwrite_physics: Option<bool>, // 37 OVPH (added in 10.?)
    pub display_texture: Option<bool>, // 38 DIPT (added in ?)
    pub object_space_normal_map: Option<bool>, // 38.5 OSNM (added in ?)
    pub min_aa_bound: Option<Vec<u8>>, // BMIN added in 10.8 ( TODO Vector3D)
    pub max_aa_bound: Option<Vec<u8>>, // BMAX added in 10.8( TODO Vector3D)
    pub mesh_file_name: Option<String>, // 39 M3DN
    pub num_vertices: Option<u32>, // 40 M3VN
    pub compressed_vertices_len: Option<u32>, // 41 M3CY
    pub compressed_vertices_data: Option<Vec<u8>>, // 42 M3CX
    pub num_indices: Option<u32>, // 43 M3FN
    pub compressed_indices_len: Option<u32>, // 44 M3CJ
    pub compressed_indices_data: Option<Vec<u8>>, // 45 M3CI
    pub compressed_animation_vertices_len: Option<Vec<u32>>, // 46 M3AY multiple
    pub compressed_animation_vertices_data: Option<Vec<Vec<u8>>>, // 47 M3AX multiple
    pub depth_bias: f32,    // 45 PIDB
    pub add_blend: Option<bool>, // 46 ADDB - added in ?
    pub use_depth_mask: Option<bool>, // ZMSK added in 10.8
    pub alpha: Option<f32>, // 47 FALP - added in ?
    pub color: Option<Color>, // 48 COLR - added in ?
    pub light_map: Option<String>, // LMAP - added in 10.8
    pub reflection_probe: Option<String>, // REFL - added in 10.8
    pub reflection_strength: Option<f32>, // RSTR - added in 10.8
    pub refraction_probe: Option<String>, // REFR - added in 10.8
    pub refraction_thickness: Option<f32>, // RTHI - added in 10.8

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Primitive);

impl Default for Primitive {
    fn default() -> Self {
        Self {
            position: Default::default(),
            size: Vertex3D::new(100.0, 100.0, 100.0),
            rot_and_tra: [0.0; 9],
            image: Default::default(),
            normal_map: None,
            sides: 4,
            name: Default::default(),
            material: Default::default(),
            side_color: Color::BLACK,
            is_visible: true,
            draw_textures_inside: false,
            hit_event: true,
            threshold: 2.0,
            elasticity: 0.3,
            elasticity_falloff: 0.5,
            friction: 0.3,
            scatter: 0.0,
            edge_factor_ui: 0.25,
            collision_reduction_factor: None,
            is_collidable: true,
            is_toy: false,
            use_3d_mesh: false,
            static_rendering: false,
            disable_lighting_top_old: None,
            disable_lighting_top: None,
            disable_lighting_below: None,
            is_reflection_enabled: None,
            backfaces_enabled: None,
            physics_material: None,
            overwrite_physics: None,
            display_texture: None,
            object_space_normal_map: None,
            min_aa_bound: None,
            max_aa_bound: None,
            mesh_file_name: None,
            num_vertices: None,
            compressed_vertices_len: None,
            compressed_vertices_data: None,
            num_indices: None,
            compressed_indices_len: None,
            compressed_indices_data: None,
            compressed_animation_vertices_len: None,
            compressed_animation_vertices_data: None,
            depth_bias: 0.0,
            add_blend: None,
            use_depth_mask: None,
            alpha: None,
            color: None,
            light_map: None,
            reflection_probe: None,
            reflection_strength: None,
            refraction_probe: None,
            refraction_thickness: None,
            is_locked: false,
            editor_layer: None,
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PrimitiveJson {
    position: Vertex3D,
    size: Vertex3D,
    rot_and_tra: [f32; 9],
    image: String,
    normal_map: Option<String>,
    sides: u32,
    name: String,
    material: String,
    side_color: Color,
    is_visible: bool,
    draw_textures_inside: bool,
    hit_event: bool,
    threshold: f32,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter: f32,
    edge_factor_ui: f32,
    collision_reduction_factor: Option<f32>,
    is_collidable: bool,
    is_toy: bool,
    use_3d_mesh: bool,
    static_rendering: bool,
    disable_lighting_top_old: Option<f32>,
    disable_lighting_top: Option<f32>,
    disable_lighting_below: Option<f32>,
    is_reflection_enabled: Option<bool>,
    backfaces_enabled: Option<bool>,
    physics_material: Option<String>,
    overwrite_physics: Option<bool>,
    display_texture: Option<bool>,
    object_space_normal_map: Option<bool>,
    min_aa_bound: Option<Vec<u8>>,
    max_aa_bound: Option<Vec<u8>>,
    mesh_file_name: Option<String>,
    depth_bias: f32,
    add_blend: Option<bool>,
    use_depth_mask: Option<bool>,
    alpha: Option<f32>,
    color: Option<Color>,
    light_map: Option<String>,
    reflection_probe: Option<String>,
    reflection_strength: Option<f32>,
    refraction_probe: Option<String>,
    refraction_thickness: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

pub struct ReadMesh {
    pub vertices: Vec<([u8; 32], Vertex3dNoTex2)>,
    pub indices: Vec<i64>,
}

impl Primitive {
    pub fn read_mesh(&self) -> Result<Option<ReadMesh>, WriteError> {
        if let Some(vertices_data) = &self.compressed_vertices_data {
            if let Some(indices_data) = &self.compressed_indices_data {
                let raw_vertices = decompress_mesh_data(vertices_data)?;
                let indices = decompress_mesh_data(indices_data)?;
                let calculated_num_vertices = raw_vertices.len() / BYTES_PER_VERTEX;
                assert_eq!(
                    calculated_num_vertices,
                    self.num_vertices.unwrap_or(0) as usize,
                    "Vertices count mismatch"
                );

                let calculated_num_indices =
                    if calculated_num_vertices > MAX_VERTICES_FOR_2_BYTE_INDEX {
                        indices.len() / 4
                    } else {
                        indices.len() / 2
                    };
                assert_eq!(
                    calculated_num_indices,
                    self.num_indices.unwrap_or(0) as usize,
                    "Indices count mismatch"
                );
                let num_vertices = raw_vertices.len() / 32;
                let bytes_per_index: u8 = if num_vertices > MAX_VERTICES_FOR_2_BYTE_INDEX {
                    4
                } else {
                    2
                };
                let mut vertices: Vec<([u8; 32], Vertex3dNoTex2)> =
                    Vec::with_capacity(num_vertices);

                let mut buff = BytesMut::from(raw_vertices.as_slice());
                for _ in 0..num_vertices {
                    let mut vertex = read_vertex(&mut buff);
                    // invert the z axis for both position and normal
                    vertex.1.z = -vertex.1.z;
                    vertex.1.nz = -vertex.1.nz;
                    vertices.push(vertex);
                }

                let mut buff = BytesMut::from(indices.as_slice());
                let num_indices = indices.len() / bytes_per_index as usize;
                let mut indices: Vec<i64> = Vec::with_capacity(num_indices);
                for _ in 0..num_indices / 3 {
                    // Looks like the indices are in reverse order
                    let v1 = read_vertex_index_from_vpx(bytes_per_index, &mut buff);
                    let v2 = read_vertex_index_from_vpx(bytes_per_index, &mut buff);
                    let v3 = read_vertex_index_from_vpx(bytes_per_index, &mut buff);
                    indices.push(v3);
                    indices.push(v2);
                    indices.push(v1);
                }

                Ok(Some(ReadMesh { vertices, indices }))
            } else {
                Err(WriteError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Primitive {} has vertices but no indices", self.name),
                )))
            }
        } else {
            Ok(None)
        }
    }
}

impl PrimitiveJson {
    pub fn from_primitive(primitive: &Primitive) -> Self {
        Self {
            position: primitive.position,
            size: primitive.size,
            rot_and_tra: primitive.rot_and_tra,
            image: primitive.image.clone(),
            normal_map: primitive.normal_map.clone(),
            sides: primitive.sides,
            name: primitive.name.clone(),
            material: primitive.material.clone(),
            side_color: primitive.side_color,
            is_visible: primitive.is_visible,
            draw_textures_inside: primitive.draw_textures_inside,
            hit_event: primitive.hit_event,
            threshold: primitive.threshold,
            elasticity: primitive.elasticity,
            elasticity_falloff: primitive.elasticity_falloff,
            friction: primitive.friction,
            scatter: primitive.scatter,
            edge_factor_ui: primitive.edge_factor_ui,
            collision_reduction_factor: primitive.collision_reduction_factor,
            is_collidable: primitive.is_collidable,
            is_toy: primitive.is_toy,
            use_3d_mesh: primitive.use_3d_mesh,
            static_rendering: primitive.static_rendering,
            disable_lighting_top_old: primitive.disable_lighting_top_old,
            disable_lighting_top: primitive.disable_lighting_top,
            disable_lighting_below: primitive.disable_lighting_below,
            is_reflection_enabled: primitive.is_reflection_enabled,
            backfaces_enabled: primitive.backfaces_enabled,
            physics_material: primitive.physics_material.clone(),
            overwrite_physics: primitive.overwrite_physics,
            display_texture: primitive.display_texture,
            object_space_normal_map: primitive.object_space_normal_map,
            min_aa_bound: primitive.min_aa_bound.clone(),
            max_aa_bound: primitive.max_aa_bound.clone(),
            mesh_file_name: primitive.mesh_file_name.clone(),
            // num_vertices: primitive.num_vertices,
            // compressed_vertices: primitive.compressed_vertices_len,
            //compressed_vertices_data: primitive.m3cx.clone(),
            // num_indices: primitive.num_indices,
            // compressed_indices: primitive.compressed_indices_len,
            // compressed_indices_Data: primitive.m3ci.clone(),
            // compressed_animation_vertices: primitive.compressed_animation_vertices_len.clone(),
            // compressed_animation_vertices_data: primitive
            //     .compressed_animation_vertices_data
            //     .clone(),
            depth_bias: primitive.depth_bias,
            add_blend: primitive.add_blend,
            use_depth_mask: primitive.use_depth_mask,
            alpha: primitive.alpha,
            color: primitive.color,
            light_map: primitive.light_map.clone(),
            reflection_probe: primitive.reflection_probe.clone(),
            reflection_strength: primitive.reflection_strength,
            refraction_probe: primitive.refraction_probe.clone(),
            refraction_thickness: primitive.refraction_thickness,
            part_group_name: primitive.part_group_name.clone(),
        }
    }
    pub fn to_primitive(&self) -> Primitive {
        Primitive {
            position: self.position,
            size: self.size,
            rot_and_tra: self.rot_and_tra,
            image: self.image.clone(),
            normal_map: self.normal_map.clone(),
            sides: self.sides,
            name: self.name.clone(),
            material: self.material.clone(),
            side_color: self.side_color,
            is_visible: self.is_visible,
            draw_textures_inside: self.draw_textures_inside,
            hit_event: self.hit_event,
            threshold: self.threshold,
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            scatter: self.scatter,
            edge_factor_ui: self.edge_factor_ui,
            collision_reduction_factor: self.collision_reduction_factor,
            is_collidable: self.is_collidable,
            is_toy: self.is_toy,
            use_3d_mesh: self.use_3d_mesh,
            static_rendering: self.static_rendering,
            disable_lighting_top_old: self.disable_lighting_top_old,
            disable_lighting_top: self.disable_lighting_top,
            disable_lighting_below: self.disable_lighting_below,
            is_reflection_enabled: self.is_reflection_enabled,
            backfaces_enabled: self.backfaces_enabled,
            physics_material: self.physics_material.clone(),
            overwrite_physics: self.overwrite_physics,
            display_texture: self.display_texture,
            object_space_normal_map: self.object_space_normal_map,
            min_aa_bound: self.min_aa_bound.clone(),
            max_aa_bound: self.max_aa_bound.clone(),
            mesh_file_name: self.mesh_file_name.clone(),
            num_vertices: None,                       //self.num_vertices,
            compressed_vertices_len: None,            //self.compressed_vertices,
            compressed_vertices_data: None,           //self.m3cx.clone(),
            num_indices: None,                        //self.num_indices,
            compressed_indices_len: None,             //self.compressed_indices,
            compressed_indices_data: None,            //self.m3ci.clone(),
            compressed_animation_vertices_len: None,  //self.compressed_animation_vertices.clone(),
            compressed_animation_vertices_data: None, //self.compressed_animation_vertices_data.clone(),
            depth_bias: self.depth_bias,
            add_blend: self.add_blend,
            use_depth_mask: self.use_depth_mask,
            alpha: self.alpha,
            color: self.color,
            light_map: self.light_map.clone(),
            reflection_probe: self.reflection_probe.clone(),
            reflection_strength: self.reflection_strength,
            refraction_probe: self.refraction_probe.clone(),
            refraction_thickness: self.refraction_thickness,
            // this is populated from a different file
            is_locked: false,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
        }
    }
}

impl Serialize for Primitive {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PrimitiveJson::from_primitive(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Primitive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = PrimitiveJson::deserialize(deserializer)?;
        Ok(json.to_primitive())
    }
}

impl BiffRead for Primitive {
    fn biff_read(reader: &mut BiffReader<'_>) -> Primitive {
        let mut compressed_animation_vertices: Option<Vec<u32>> = None;
        let mut m3ax: Option<Vec<Vec<u8>>> = None;
        let mut primitive = Primitive::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            //println!("tag: {}", tag_str);
            match tag_str {
                // TOTAN4K had this
                // https://github.com/freezy/VisualPinball.Engine/blob/ec1e9765cd4832c134e889d6e6d03320bc404bd5/VisualPinball.Engine/VPT/Primitive/PrimitiveData.cs#L64
                // Unknown tag M3AY for vpxtool::vpx::gameitem::primitive::Primitive
                // Unknown tag M3AY for vpxtool::vpx::gameitem::primitive::Primitive
                // Unknown tag M3AY for vpxtool::vpx::gameitem::primitive::Primitive
                "VPOS" => {
                    primitive.position = Vertex3D::biff_read(reader);
                }
                "VSIZ" => {
                    primitive.size = Vertex3D::biff_read(reader);
                }
                "RTV0" => {
                    primitive.rot_and_tra[0] = reader.get_f32();
                }
                "RTV1" => {
                    primitive.rot_and_tra[1] = reader.get_f32();
                }
                "RTV2" => {
                    primitive.rot_and_tra[2] = reader.get_f32();
                }
                "RTV3" => {
                    primitive.rot_and_tra[3] = reader.get_f32();
                }
                "RTV4" => {
                    primitive.rot_and_tra[4] = reader.get_f32();
                }
                "RTV5" => {
                    primitive.rot_and_tra[5] = reader.get_f32();
                }
                "RTV6" => {
                    primitive.rot_and_tra[6] = reader.get_f32();
                }
                "RTV7" => {
                    primitive.rot_and_tra[7] = reader.get_f32();
                }
                "RTV8" => {
                    primitive.rot_and_tra[8] = reader.get_f32();
                }
                "IMAG" => {
                    primitive.image = reader.get_string();
                }
                "NRMA" => {
                    primitive.normal_map = Some(reader.get_string());
                }
                "SIDS" => {
                    primitive.sides = reader.get_u32();
                }
                "NAME" => {
                    primitive.name = reader.get_wide_string();
                }
                "MATR" => {
                    primitive.material = reader.get_string();
                }
                "SCOL" => {
                    primitive.side_color = Color::biff_read(reader);
                }
                "TVIS" => {
                    primitive.is_visible = reader.get_bool();
                }
                "DTXI" => {
                    primitive.draw_textures_inside = reader.get_bool();
                }
                "HTEV" => {
                    primitive.hit_event = reader.get_bool();
                }
                "THRS" => {
                    primitive.threshold = reader.get_f32();
                }
                "ELAS" => {
                    primitive.elasticity = reader.get_f32();
                }
                "ELFO" => {
                    primitive.elasticity_falloff = reader.get_f32();
                }
                "RFCT" => {
                    primitive.friction = reader.get_f32();
                }
                "RSCT" => {
                    primitive.scatter = reader.get_f32();
                }
                "EFUI" => {
                    primitive.edge_factor_ui = reader.get_f32();
                }
                "CORF" => {
                    primitive.collision_reduction_factor = Some(reader.get_f32());
                }
                "CLDR" => {
                    primitive.is_collidable = reader.get_bool();
                }
                "ISTO" => {
                    primitive.is_toy = reader.get_bool();
                }
                "U3DM" => {
                    primitive.use_3d_mesh = reader.get_bool();
                }
                "STRE" => {
                    primitive.static_rendering = reader.get_bool();
                }
                //[BiffFloat("DILI", QuantizedUnsignedBits = 8, Pos = 32)]
                //public float DisableLightingTop; // m_d.m_fDisableLightingTop = (tmp == 1) ? 1.f : dequantizeUnsigned<8>(tmp); // backwards compatible hacky loading!
                "DILI" => {
                    primitive.disable_lighting_top_old = Some(reader.get_f32());
                }
                "DILT" => {
                    primitive.disable_lighting_top = Some(reader.get_f32());
                }
                "DILB" => {
                    primitive.disable_lighting_below = Some(reader.get_f32());
                }
                "REEN" => {
                    primitive.is_reflection_enabled = Some(reader.get_bool());
                }
                "EBFC" => {
                    primitive.backfaces_enabled = Some(reader.get_bool());
                }
                "MAPH" => {
                    primitive.physics_material = Some(reader.get_string());
                }
                "OVPH" => {
                    primitive.overwrite_physics = Some(reader.get_bool());
                }
                "DIPT" => {
                    primitive.display_texture = Some(reader.get_bool());
                }
                "OSNM" => {
                    primitive.object_space_normal_map = Some(reader.get_bool());
                }
                "BMIN" => {
                    primitive.min_aa_bound = Some(reader.get_record_data(false));
                }
                "BMAX" => {
                    primitive.max_aa_bound = Some(reader.get_record_data(false));
                }
                "M3DN" => {
                    primitive.mesh_file_name = Some(reader.get_string());
                }
                "M3VN" => {
                    primitive.num_vertices = Some(reader.get_u32());
                }
                "M3CY" => {
                    primitive.compressed_vertices_len = Some(reader.get_u32());
                }

                // [BiffVertices("M3DX", SkipWrite = true)]
                // [BiffVertices("M3CX", IsCompressed = true, Pos = 42)]
                // [BiffIndices("M3DI", SkipWrite = true)]
                // [BiffIndices("M3CI", IsCompressed = true, Pos = 45)]
                // [BiffAnimation("M3AX", IsCompressed = true, Pos = 47 )]
                // public Mesh Mesh = new Mesh();
                "M3CX" => {
                    primitive.compressed_vertices_data = Some(reader.get_record_data(false));
                }
                "M3FN" => {
                    primitive.num_indices = Some(reader.get_u32());
                }
                "M3CJ" => {
                    primitive.compressed_indices_len = Some(reader.get_u32());
                }
                "M3CI" => {
                    primitive.compressed_indices_data = Some(reader.get_record_data(false));
                }
                "M3AY" => {
                    match compressed_animation_vertices {
                        Some(ref mut m3ay) => {
                            m3ay.push(reader.get_u32());
                        }
                        None => compressed_animation_vertices = Some(vec![reader.get_u32()]),
                    };
                }
                "M3AX" => {
                    match m3ax {
                        Some(ref mut m3ax) => {
                            m3ax.push(reader.get_record_data(false));
                        }
                        None => {
                            m3ax = Some(vec![reader.get_record_data(false)]);
                        }
                    };
                }
                "PIDB" => {
                    primitive.depth_bias = reader.get_f32();
                }
                "ADDB" => {
                    primitive.add_blend = Some(reader.get_bool());
                }
                "ZMSK" => {
                    primitive.use_depth_mask = Some(reader.get_bool());
                }
                "FALP" => {
                    primitive.alpha = Some(reader.get_f32());
                }
                "COLR" => {
                    primitive.color = Some(Color::biff_read(reader));
                }
                "LMAP" => {
                    primitive.light_map = Some(reader.get_string());
                }
                "REFL" => {
                    primitive.reflection_probe = Some(reader.get_string());
                }
                "RSTR" => {
                    primitive.reflection_strength = Some(reader.get_f32());
                }
                "REFR" => {
                    primitive.refraction_probe = Some(reader.get_string());
                }
                "RTHI" => {
                    primitive.refraction_thickness = Some(reader.get_f32());
                }
                _ => {
                    if !primitive.read_shared_attribute(tag_str, reader) {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag();
                    }
                }
            }
        }

        primitive.compressed_animation_vertices_len = compressed_animation_vertices;
        primitive.compressed_animation_vertices_data = m3ax;
        primitive
    }
}

impl BiffWrite for Primitive {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VPOS", &self.position);
        writer.write_tagged("VSIZ", &self.size);
        writer.write_tagged_f32("RTV0", self.rot_and_tra[0]);
        writer.write_tagged_f32("RTV1", self.rot_and_tra[1]);
        writer.write_tagged_f32("RTV2", self.rot_and_tra[2]);
        writer.write_tagged_f32("RTV3", self.rot_and_tra[3]);
        writer.write_tagged_f32("RTV4", self.rot_and_tra[4]);
        writer.write_tagged_f32("RTV5", self.rot_and_tra[5]);
        writer.write_tagged_f32("RTV6", self.rot_and_tra[6]);
        writer.write_tagged_f32("RTV7", self.rot_and_tra[7]);
        writer.write_tagged_f32("RTV8", self.rot_and_tra[8]);
        writer.write_tagged_string("IMAG", &self.image);
        if let Some(normal_map) = &self.normal_map {
            writer.write_tagged_string("NRMA", normal_map);
        }
        writer.write_tagged_u32("SIDS", self.sides);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_with("SCOL", &self.side_color, Color::biff_write);
        writer.write_tagged_bool("TVIS", self.is_visible);
        writer.write_tagged_bool("DTXI", self.draw_textures_inside);
        writer.write_tagged_bool("HTEV", self.hit_event);
        writer.write_tagged_f32("THRS", self.threshold);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("RFCT", self.friction);
        writer.write_tagged_f32("RSCT", self.scatter);
        writer.write_tagged_f32("EFUI", self.edge_factor_ui);
        if let Some(collision_reduction_factor) = self.collision_reduction_factor {
            writer.write_tagged_f32("CORF", collision_reduction_factor);
        }
        writer.write_tagged_bool("CLDR", self.is_collidable);
        writer.write_tagged_bool("ISTO", self.is_toy);
        writer.write_tagged_bool("U3DM", self.use_3d_mesh);
        writer.write_tagged_bool("STRE", self.static_rendering);
        if let Some(disable_lighting_top_old) = self.disable_lighting_top_old {
            writer.write_tagged_f32("DILI", disable_lighting_top_old);
        }
        if let Some(disable_lighting_top) = self.disable_lighting_top {
            writer.write_tagged_f32("DILT", disable_lighting_top);
        }
        if let Some(disable_lighting_below) = self.disable_lighting_below {
            writer.write_tagged_f32("DILB", disable_lighting_below);
        }
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }
        if let Some(backfaces_enabled) = self.backfaces_enabled {
            writer.write_tagged_bool("EBFC", backfaces_enabled);
        }
        if let Some(physics_material) = &self.physics_material {
            writer.write_tagged_string("MAPH", physics_material);
        }
        if let Some(overwrite_physics) = self.overwrite_physics {
            writer.write_tagged_bool("OVPH", overwrite_physics);
        }
        if let Some(display_texture) = self.display_texture {
            writer.write_tagged_bool("DIPT", display_texture);
        }
        if let Some(object_space_normal_map) = self.object_space_normal_map {
            writer.write_tagged_bool("OSNM", object_space_normal_map);
        }

        if let Some(min_aa_bound) = &self.min_aa_bound {
            writer.write_tagged_data("BMIN", min_aa_bound);
        }
        if let Some(max_aa_bound) = &self.max_aa_bound {
            writer.write_tagged_data("BMAX", max_aa_bound);
        }
        if let Some(mesh_file_name) = &self.mesh_file_name {
            writer.write_tagged_string("M3DN", mesh_file_name);
        }
        if let Some(num_vertices) = &self.num_vertices {
            writer.write_tagged_u32("M3VN", *num_vertices);
        }
        if let Some(compressed_vertices) = &self.compressed_vertices_len {
            writer.write_tagged_u32("M3CY", *compressed_vertices);
        }
        if let Some(m3cx) = &self.compressed_vertices_data {
            writer.write_tagged_data("M3CX", m3cx);
        }
        if let Some(num_indices) = &self.num_indices {
            writer.write_tagged_u32("M3FN", *num_indices);
        }
        if let Some(compressed_indices) = &self.compressed_indices_len {
            writer.write_tagged_u32("M3CJ", *compressed_indices);
        }
        if let Some(m3ci) = &self.compressed_indices_data {
            writer.write_tagged_data("M3CI", m3ci);
        }

        // these should come in pairs
        // TODO rework in a better way
        // if both are present, write them in pairs
        if let (Some(m3ays), Some(m3axs)) = (
            &self.compressed_animation_vertices_len,
            &self.compressed_animation_vertices_data,
        ) {
            for (m3ay, m3ax) in m3ays.iter().zip(m3axs.iter()) {
                writer.write_tagged_u32("M3AY", *m3ay);
                writer.write_tagged_data("M3AX", m3ax);
            }
        }

        writer.write_tagged_f32("PIDB", self.depth_bias);

        if let Some(add_blend) = self.add_blend {
            writer.write_tagged_bool("ADDB", add_blend);
        }
        if let Some(use_depth_mask) = self.use_depth_mask {
            writer.write_tagged_bool("ZMSK", use_depth_mask);
        }
        if let Some(alpha) = self.alpha {
            writer.write_tagged_f32("FALP", alpha);
        }
        if let Some(color) = &self.color {
            writer.write_tagged_with("COLR", color, Color::biff_write);
        }
        if let Some(light_map) = &self.light_map {
            writer.write_tagged_string("LMAP", light_map);
        }
        if let Some(reflection_probe) = &self.reflection_probe {
            writer.write_tagged_string("REFL", reflection_probe);
        }
        if let Some(reflection_strength) = &self.reflection_strength {
            writer.write_tagged_f32("RSTR", *reflection_strength);
        }
        if let Some(refraction_probe) = &self.refraction_probe {
            writer.write_tagged_string("REFR", refraction_probe);
        }
        if let Some(refraction_thickness) = &self.refraction_thickness {
            writer.write_tagged_f32("RTHI", *refraction_thickness);
        }

        self.write_shared_attributes(writer);

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;
    use fake::{Fake, Faker};

    use super::*;
    use crate::vpx::gameitem::tests::RandomOption;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        let primitive: Primitive = Primitive {
            position: Vertex3D::new(1.0, 2.0, 3.0),
            size: Vertex3D::new(4.0, 5.0, 6.0),
            rot_and_tra: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
            image: "image".to_string(),
            normal_map: Some("normal_map".to_string()),
            sides: 1,
            name: "name".to_string(),
            material: "material".to_string(),
            side_color: Faker.fake(),
            is_visible: rng.random(),
            // random bool
            draw_textures_inside: rng.random(),
            hit_event: rng.random(),
            threshold: 1.0,
            elasticity: 2.0,
            elasticity_falloff: 3.0,
            friction: 4.0,
            scatter: 5.0,
            edge_factor_ui: 6.0,
            collision_reduction_factor: Some(7.0),
            is_collidable: rng.random(),
            is_toy: rng.random(),
            use_3d_mesh: rng.random(),
            static_rendering: rng.random(),
            disable_lighting_top_old: Some(rng.random()),
            disable_lighting_top: Some(rng.random()),
            disable_lighting_below: rng.random_option(),
            is_reflection_enabled: rng.random_option(),
            backfaces_enabled: rng.random_option(),
            physics_material: Some("physics_material".to_string()),
            overwrite_physics: rng.random_option(),
            display_texture: rng.random_option(),
            object_space_normal_map: rng.random_option(),
            min_aa_bound: Some(vec![0, 1, 2, 3, 4, 5, 6, 7, 8]),
            max_aa_bound: Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            mesh_file_name: Some("mesh_file_name".to_string()),
            num_vertices: Some(8),
            compressed_vertices_len: Some(9),
            compressed_vertices_data: Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            num_indices: Some(10),
            compressed_indices_len: Some(11),
            compressed_indices_data: Some(vec![2, 3, 4, 5, 6, 7, 8, 9, 10]),
            compressed_animation_vertices_len: Some(vec![9, 8]),
            compressed_animation_vertices_data: Some(vec![
                vec![4, 5, 6, 7, 8, 9, 10, 11, 12],
                vec![5, 6, 7, 8, 9, 10, 11, 12],
            ]),
            depth_bias: 12.0,
            add_blend: rng.random_option(),
            use_depth_mask: rng.random_option(),
            alpha: Some(13.0),
            color: Faker.fake(),
            light_map: Some("light_map".to_string()),
            reflection_probe: Some("reflection_probe".to_string()),
            reflection_strength: Some(14.0),
            refraction_probe: Some("refraction_probe".to_string()),
            refraction_thickness: Some(15.0),
            is_locked: rng.random(),
            editor_layer: Some(17),
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: rng.random_option(),
            part_group_name: Some("part_group_name".to_string()),
        };
        let mut writer = BiffWriter::new();
        Primitive::biff_write(&primitive, &mut writer);
        let primitive_read = Primitive::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(primitive, primitive_read);
    }
}

fn read_vertex_index_from_vpx(bytes_per_index: u8, buff: &mut BytesMut) -> i64 {
    if bytes_per_index == 2 {
        buff.get_u16_le() as i64
    } else {
        buff.get_u32_le() as i64
    }
}

/// Decompress mesh data (vertices or indices) using zlib compression.
//
// This is how they were compressed using zlib
//
// const mz_ulong slen = (mz_ulong)(sizeof(Vertex3dNoTex2)*m_mesh.NumVertices());
// mz_ulong clen = compressBound(slen);
// mz_uint8 * c = (mz_uint8 *)malloc(clen);
// if (compress2(c, &clen, (const unsigned char *)m_mesh.m_vertices.data(), slen, MZ_BEST_COMPRESSION) != Z_OK)
// ShowError("Could not compress primitive vertex data");
fn decompress_mesh_data(compressed_data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(compressed_data);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data)?;
    Ok(decompressed_data)
}

/// Compress mesh data (vertices or indices) using zlib compression.
pub(crate) fn compress_mesh_data(data: &[u8]) -> io::Result<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    // before 10.6.1, compression was always LZW
    // "abuses the VP-Image-LZW compressor"
    // see https://github.com/vpinball/vpinball/commit/09f5510d676cd6b204350dfc4a93b9bf93284c56

    // Pre-allocate buffer with estimated compressed size (typically ~50-70% of original)
    let estimated_size = (data.len() * 7) / 10;
    let output = Vec::with_capacity(estimated_size);

    // The best compression level is too slow for large meshes, so we use a default level
    let compression_level = Compression::default();

    let mut encoder = ZlibEncoder::new(output, compression_level);
    encoder.write_all(data)?;
    encoder.finish()
}

fn read_vertex(buffer: &mut BytesMut) -> ([u8; 32], Vertex3dNoTex2) {
    let mut bytes = [0; 32];
    buffer.copy_to_slice(&mut bytes);
    let mut vertex_buff = BytesMut::from(bytes.as_ref());

    let x = vertex_buff.get_f32_le();
    let y = vertex_buff.get_f32_le();
    let z = vertex_buff.get_f32_le();
    // normals
    let nx = vertex_buff.get_f32_le();
    let ny = vertex_buff.get_f32_le();
    let nz = vertex_buff.get_f32_le();
    // texture coordinates
    let tu = vertex_buff.get_f32_le();
    let tv = vertex_buff.get_f32_le();
    let v3d = Vertex3dNoTex2 {
        x,
        y,
        z,
        nx,
        ny,
        nz,
        tu,
        tv,
    };
    (bytes, v3d)
}

/// Animation frame vertex data
/// this is combined with the primary mesh face and texture data.
///
/// This struct is used for serializing and deserializing in the vpinball C++ code
#[derive(Debug, Clone, Copy)]
pub struct VertData {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
}
impl VertData {
    pub const SERIALIZED_SIZE: usize = 24;
}

pub(crate) fn read_vpx_animation_frame(
    compressed_frame: &[u8],
    compressed_length: &u32,
) -> Result<Vec<VertData>, WriteError> {
    if compressed_frame.len() != *compressed_length as usize {
        return Err(WriteError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Animation frame compressed length does not match: {} != {}",
                compressed_frame.len(),
                compressed_length
            ),
        )));
    }
    let decompressed_frame = decompress_mesh_data(compressed_frame)?;
    let frame_data_len = decompressed_frame.len() / VertData::SERIALIZED_SIZE;
    let mut buff = BytesMut::from(decompressed_frame.as_slice());
    let mut vertices: Vec<VertData> = Vec::with_capacity(frame_data_len);
    for _ in 0..frame_data_len {
        let vertex = read_animation_vertex_data(&mut buff);
        vertices.push(vertex);
    }
    Ok(vertices)
}

fn read_animation_vertex_data(buffer: &mut BytesMut) -> VertData {
    let x = buffer.get_f32_le();
    let y = buffer.get_f32_le();
    let z = buffer.get_f32_le();
    let nx = buffer.get_f32_le();
    let ny = buffer.get_f32_le();
    let nz = buffer.get_f32_le();
    VertData {
        x,
        y,
        z,
        nx,
        ny,
        nz,
    }
}

pub(crate) fn write_animation_vertex_data(buff: &mut BytesMut, vertex: &VertData) {
    buff.put_f32_le(vertex.x);
    buff.put_f32_le(vertex.y);
    buff.put_f32_le(vertex.z);
    buff.put_f32_le(vertex.nx);
    buff.put_f32_le(vertex.ny);
    buff.put_f32_le(vertex.nz);
}
