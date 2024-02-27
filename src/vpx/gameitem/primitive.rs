use crate::vpx::color::ColorJson;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex3d::Vertex3D;

#[derive(Debug, PartialEq, Dummy)]
pub struct Primitive {
    pub position: Vertex3D,                      // 0 VPOS
    pub size: Vertex3D,                          // 1 VSIZ
    pub rot_and_tra: [f32; 9],                   // 2-11 RTV0-RTV8
    pub image: String,                           // 12 IMAG
    pub normal_map: Option<String>,              // 13 NRMA (added in 10.?)
    pub sides: u32,                              // 14
    pub name: String,                            // 15
    pub material: String,                        // 16
    pub side_color: Color,                       // 17
    pub is_visible: bool,                        // 18
    pub draw_textures_inside: bool,              // 19
    pub hit_event: bool,                         // 20
    pub threshold: f32,                          // 21
    pub elasticity: f32,                         // 22
    pub elasticity_falloff: f32,                 // 23
    pub friction: f32,                           // 24
    pub scatter: f32,                            // 25
    pub edge_factor_ui: f32,                     // 26
    pub collision_reduction_factor: Option<f32>, // 27 CORF (was missing in 10.01)
    pub is_collidable: bool,                     // 28
    pub is_toy: bool,                            // 29
    pub use_3d_mesh: bool,                       // 30
    pub static_rendering: bool,                  // 31
    pub disable_lighting_top_old: Option<f32>,   // DILI (removed in 10.8)
    pub disable_lighting_top: Option<f32>,       // DILT (added in 10.8)
    pub disable_lighting_below: Option<f32>,     // 33 DILB (added in 10.?)
    pub is_reflection_enabled: Option<bool>,     // 34 REEN (was missing in 10.01)
    pub backfaces_enabled: Option<bool>,         // 35 EBFC (added in 10.?)
    pub physics_material: Option<String>,        // 36 MAPH (added in 10.?)
    pub overwrite_physics: Option<bool>,         // 37 OVPH (added in 10.?)
    pub display_texture: Option<bool>,           // 38 DIPT (added in ?)
    pub object_space_normal_map: Option<bool>,   // 38.5 OSNM (added in ?)
    pub min_aa_bound: Option<Vec<u8>>,           // BMIN added in 10.8 ( TODO Vector3D)
    pub max_aa_bound: Option<Vec<u8>>,           // BMAX added in 10.8( TODO Vector3D)
    pub mesh_file_name: Option<String>,          // 39 M3DN
    pub num_vertices: Option<u32>,               // 40 M3VN
    pub compressed_vertices: Option<u32>,        // 41 M3CY
    pub m3cx: Option<Vec<u8>>,                   // 42 M3CX
    pub num_indices: Option<u32>,                // 43 M3FN
    pub compressed_indices: Option<u32>,         // 44 M3CJ
    pub m3ci: Option<Vec<u8>>,                   // 45 M3CI
    pub m3ay: Option<Vec<Vec<u8>>>,              // 46 M3AY multiple
    pub m3ax: Option<Vec<Vec<u8>>>,              // 47 M3AX multiple
    pub depth_bias: f32,                         // 45 PIDB
    pub add_blend: Option<bool>,                 // 46 ADDB - added in ?
    pub use_depth_mask: Option<bool>,            // ZMSK added in 10.8
    pub alpha: Option<f32>,                      // 47 FALP - added in ?
    pub color: Option<Color>,                    // 48 COLR - added in ?
    pub light_map: Option<String>,               // LMAP - added in 10.8
    pub reflection_probe: Option<String>,        // REFL - added in 10.8
    pub reflection_strength: Option<f32>,        // RSTR - added in 10.8
    pub refraction_probe: Option<String>,        // REFR - added in 10.8
    pub refraction_thickness: Option<f32>,       // RTHI - added in 10.8

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
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
    side_color: ColorJson,
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
    num_vertices: Option<u32>,
    compressed_vertices: Option<u32>,
    m3cx: Option<Vec<u8>>,
    num_indices: Option<u32>,
    compressed_indices: Option<u32>,
    m3ci: Option<Vec<u8>>,
    m3ay: Option<Vec<Vec<u8>>>,
    m3ax: Option<Vec<Vec<u8>>>,
    depth_bias: f32,
    add_blend: Option<bool>,
    use_depth_mask: Option<bool>,
    alpha: Option<f32>,
    color: Option<ColorJson>,
    light_map: Option<String>,
    reflection_probe: Option<String>,
    reflection_strength: Option<f32>,
    refraction_probe: Option<String>,
    refraction_thickness: Option<f32>,
    is_locked: bool,
    editor_layer: u32,
    editor_layer_name: Option<String>,
    editor_layer_visibility: Option<bool>,
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
            side_color: ColorJson::from_color(&primitive.side_color),
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
            num_vertices: primitive.num_vertices,
            compressed_vertices: primitive.compressed_vertices,
            m3cx: primitive.m3cx.clone(),
            num_indices: primitive.num_indices,
            compressed_indices: primitive.compressed_indices,
            m3ci: primitive.m3ci.clone(),
            m3ay: primitive.m3ay.clone(),
            m3ax: primitive.m3ax.clone(),
            depth_bias: primitive.depth_bias,
            add_blend: primitive.add_blend,
            use_depth_mask: primitive.use_depth_mask,
            alpha: primitive.alpha,
            color: primitive.color.map(|c| ColorJson::from_color(&c)),
            light_map: primitive.light_map.clone(),
            reflection_probe: primitive.reflection_probe.clone(),
            reflection_strength: primitive.reflection_strength,
            refraction_probe: primitive.refraction_probe.clone(),
            refraction_thickness: primitive.refraction_thickness,
            is_locked: primitive.is_locked,
            editor_layer: primitive.editor_layer,
            editor_layer_name: primitive.editor_layer_name.clone(),
            editor_layer_visibility: primitive.editor_layer_visibility,
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
            side_color: self.side_color.to_color(),
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
            num_vertices: self.num_vertices,
            compressed_vertices: self.compressed_vertices,
            m3cx: self.m3cx.clone(),
            num_indices: self.num_indices,
            compressed_indices: self.compressed_indices,
            m3ci: self.m3ci.clone(),
            m3ay: self.m3ay.clone(),
            m3ax: self.m3ax.clone(),
            depth_bias: self.depth_bias,
            add_blend: self.add_blend,
            use_depth_mask: self.use_depth_mask,
            alpha: self.alpha,
            color: self.color.as_ref().map(|c| ColorJson::to_color(c)),
            light_map: self.light_map.clone(),
            reflection_probe: self.reflection_probe.clone(),
            reflection_strength: self.reflection_strength,
            refraction_probe: self.refraction_probe.clone(),
            refraction_thickness: self.refraction_thickness,
            is_locked: self.is_locked,
            editor_layer: self.editor_layer,
            editor_layer_name: self.editor_layer_name.clone(),
            editor_layer_visibility: self.editor_layer_visibility,
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
        let mut position = Default::default();
        let mut size = Vertex3D::new(100.0, 100.0, 100.0);
        let mut rot_and_tra: [f32; 9] = [0.0; 9];
        let mut image = Default::default();
        let mut normal_map: Option<String> = None;
        let mut sides: u32 = 4;
        let mut name = Default::default();
        let mut material = Default::default();
        let mut side_color = Color::new_bgr(0x0);
        let mut is_visible: bool = true;
        let mut draw_textures_inside: bool = false;
        let mut hit_event: bool = true;
        let mut threshold: f32 = 2.0;
        let mut elasticity: f32 = 0.3;
        let mut elasticity_falloff: f32 = 0.5;
        let mut friction: f32 = 0.3;
        let mut scatter: f32 = 0.0;
        let mut edge_factor_ui: f32 = 0.25;
        let mut collision_reduction_factor: Option<f32> = None; //0.0;
        let mut is_collidable: bool = true;
        let mut is_toy: bool = false;
        let mut use_3d_mesh: bool = false;
        let mut static_rendering: bool = false;
        let mut disable_lighting_top_old: Option<f32> = None; //0.0;
        let mut disable_lighting_top: Option<f32> = None; //0.0;
        let mut disable_lighting_below: Option<f32> = None; //0.0;
        let mut is_reflection_enabled: Option<bool> = None; //true;
        let mut backfaces_enabled: Option<bool> = None; //false;
        let mut physics_material: Option<String> = None;
        let mut overwrite_physics: Option<bool> = None; //true;
        let mut display_texture: Option<bool> = None; //true;
        let mut object_space_normal_map: Option<bool> = None; //false;
        let mut min_aa_bound: Option<Vec<u8>> = None;
        let mut max_aa_bound: Option<Vec<u8>> = None;

        let mut mesh_file_name: Option<String> = None;
        let mut num_vertices: Option<u32> = None;
        let mut compressed_vertices: Option<u32> = None;
        let mut m3cx: Option<Vec<u8>> = None;
        let mut num_indices: Option<u32> = None;
        let mut compressed_indices: Option<u32> = None;
        let mut m3ci: Option<Vec<u8>> = None;
        let mut m3ay: Option<Vec<Vec<u8>>> = None;
        let mut m3ax: Option<Vec<Vec<u8>>> = None;

        let mut depth_bias: f32 = 0.0;
        let mut add_blend: Option<bool> = None; // false;
        let mut use_depth_mask: Option<bool> = None; // false;
        let mut alpha: Option<f32> = None; //1.0;
        let mut color: Option<Color> = None; //Color::new_bgr(0x0);
        let mut light_map: Option<String> = None;
        let mut reflection_probe: Option<String> = None;
        let mut reflection_strength: Option<f32> = None;
        let mut refraction_probe: Option<String> = None;
        let mut refraction_thickness: Option<f32> = None;

        // these are shared between all items
        let mut is_locked: bool = false;
        let mut editor_layer: u32 = Default::default();
        let mut editor_layer_name: Option<String> = None;
        let mut editor_layer_visibility: Option<bool> = None;

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
                    position = Vertex3D::biff_read(reader);
                }
                "VSIZ" => {
                    size = Vertex3D::biff_read(reader);
                }
                "RTV0" => {
                    rot_and_tra[0] = reader.get_f32();
                }
                "RTV1" => {
                    rot_and_tra[1] = reader.get_f32();
                }
                "RTV2" => {
                    rot_and_tra[2] = reader.get_f32();
                }
                "RTV3" => {
                    rot_and_tra[3] = reader.get_f32();
                }
                "RTV4" => {
                    rot_and_tra[4] = reader.get_f32();
                }
                "RTV5" => {
                    rot_and_tra[5] = reader.get_f32();
                }
                "RTV6" => {
                    rot_and_tra[6] = reader.get_f32();
                }
                "RTV7" => {
                    rot_and_tra[7] = reader.get_f32();
                }
                "RTV8" => {
                    rot_and_tra[8] = reader.get_f32();
                }
                "IMAG" => {
                    image = reader.get_string();
                }
                "NRMA" => {
                    normal_map = Some(reader.get_string());
                }
                "SIDS" => {
                    sides = reader.get_u32();
                }
                "NAME" => {
                    name = reader.get_wide_string();
                }
                "MATR" => {
                    material = reader.get_string();
                }
                "SCOL" => {
                    side_color = Color::biff_read_bgr(reader);
                }
                "TVIS" => {
                    is_visible = reader.get_bool();
                }
                "DTXI" => {
                    draw_textures_inside = reader.get_bool();
                }
                "HTEV" => {
                    hit_event = reader.get_bool();
                }
                "THRS" => {
                    threshold = reader.get_f32();
                }
                "ELAS" => {
                    elasticity = reader.get_f32();
                }
                "ELFO" => {
                    elasticity_falloff = reader.get_f32();
                }
                "RFCT" => {
                    friction = reader.get_f32();
                }
                "RSCT" => {
                    scatter = reader.get_f32();
                }
                "EFUI" => {
                    edge_factor_ui = reader.get_f32();
                }
                "CORF" => {
                    collision_reduction_factor = Some(reader.get_f32());
                }
                "CLDR" => {
                    is_collidable = reader.get_bool();
                }
                "ISTO" => {
                    is_toy = reader.get_bool();
                }
                "U3DM" => {
                    use_3d_mesh = reader.get_bool();
                }
                "STRE" => {
                    static_rendering = reader.get_bool();
                }
                //[BiffFloat("DILI", QuantizedUnsignedBits = 8, Pos = 32)]
                //public float DisableLightingTop; // m_d.m_fDisableLightingTop = (tmp == 1) ? 1.f : dequantizeUnsigned<8>(tmp); // backwards compatible hacky loading!
                "DILI" => {
                    disable_lighting_top_old = Some(reader.get_f32());
                }
                "DILT" => {
                    disable_lighting_top = Some(reader.get_f32());
                }
                "DILB" => {
                    disable_lighting_below = Some(reader.get_f32());
                }
                "REEN" => {
                    is_reflection_enabled = Some(reader.get_bool());
                }
                "EBFC" => {
                    backfaces_enabled = Some(reader.get_bool());
                }
                "MAPH" => {
                    physics_material = Some(reader.get_string());
                }
                "OVPH" => {
                    overwrite_physics = Some(reader.get_bool());
                }
                "DIPT" => {
                    display_texture = Some(reader.get_bool());
                }
                "OSNM" => {
                    object_space_normal_map = Some(reader.get_bool());
                }
                "BMIN" => {
                    min_aa_bound = Some(reader.get_record_data(false));
                }
                "BMAX" => {
                    max_aa_bound = Some(reader.get_record_data(false));
                }
                "M3DN" => {
                    mesh_file_name = Some(reader.get_string());
                }
                "M3VN" => {
                    num_vertices = Some(reader.get_u32());
                }
                "M3CY" => {
                    compressed_vertices = Some(reader.get_u32());
                }

                // [BiffVertices("M3DX", SkipWrite = true)]
                // [BiffVertices("M3CX", IsCompressed = true, Pos = 42)]
                // [BiffIndices("M3DI", SkipWrite = true)]
                // [BiffIndices("M3CI", IsCompressed = true, Pos = 45)]
                // [BiffAnimation("M3AX", IsCompressed = true, Pos = 47 )]
                // public Mesh Mesh = new Mesh();
                "M3CX" => {
                    m3cx = Some(reader.get_record_data(false));
                }
                "M3FN" => {
                    num_indices = Some(reader.get_u32());
                }
                "M3CJ" => {
                    compressed_indices = Some(reader.get_u32());
                }
                "M3CI" => {
                    m3ci = Some(reader.get_record_data(false));
                }
                "M3AY" => {
                    match m3ay {
                        Some(ref mut m3ay) => {
                            m3ay.push(reader.get_record_data(false));
                        }
                        None => m3ay = Some(vec![reader.get_record_data(false)]),
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
                    depth_bias = reader.get_f32();
                }
                "ADDB" => {
                    add_blend = Some(reader.get_bool());
                }
                "ZMSK" => {
                    use_depth_mask = Some(reader.get_bool());
                }
                "FALP" => {
                    alpha = Some(reader.get_f32());
                }
                "COLR" => {
                    color = Some(Color::biff_read_bgr(reader));
                }
                "LMAP" => {
                    light_map = Some(reader.get_string());
                }
                "REFL" => {
                    reflection_probe = Some(reader.get_string());
                }
                "RSTR" => {
                    reflection_strength = Some(reader.get_f32());
                }
                "REFR" => {
                    refraction_probe = Some(reader.get_string());
                }
                "RTHI" => {
                    refraction_thickness = Some(reader.get_f32());
                }

                // shared
                "LOCK" => {
                    is_locked = reader.get_bool();
                }
                "LAYR" => {
                    editor_layer = reader.get_u32();
                }
                "LANR" => {
                    editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    editor_layer_visibility = Some(reader.get_bool());
                }
                _ => {
                    println!(
                        "Unknown tag {} for {}",
                        tag_str,
                        std::any::type_name::<Self>()
                    );
                    reader.skip_tag();
                }
            }
        }
        Primitive {
            position,
            size,
            rot_and_tra,
            image,
            normal_map,
            sides,
            name,
            material,
            side_color,
            is_visible,
            draw_textures_inside,
            hit_event,
            threshold,
            elasticity,
            elasticity_falloff,
            friction,
            scatter,
            edge_factor_ui,
            collision_reduction_factor,
            is_collidable,
            is_toy,
            use_3d_mesh,
            static_rendering,
            disable_lighting_top_old,
            disable_lighting_top,
            disable_lighting_below,
            is_reflection_enabled,
            backfaces_enabled,
            physics_material,
            overwrite_physics,
            display_texture,
            object_space_normal_map,
            min_aa_bound,
            max_aa_bound,
            mesh_file_name,
            num_vertices,
            compressed_vertices,
            m3cx,
            num_indices,
            compressed_indices,
            m3ci,
            m3ay,
            m3ax,
            depth_bias,
            add_blend,
            use_depth_mask,
            alpha,
            color,
            light_map,
            reflection_probe,
            reflection_strength,
            refraction_probe,
            refraction_thickness,
            is_locked,
            editor_layer,
            editor_layer_name,
            editor_layer_visibility,
        }
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
        writer.write_tagged_with("SCOL", &self.side_color, Color::biff_write_bgr);
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
        if let Some(compressed_vertices) = &self.compressed_vertices {
            writer.write_tagged_u32("M3CY", *compressed_vertices);
        }
        if let Some(m3cx) = &self.m3cx {
            writer.write_tagged_data("M3CX", m3cx);
        }
        if let Some(num_indices) = &self.num_indices {
            writer.write_tagged_u32("M3FN", *num_indices);
        }
        if let Some(compressed_indices) = &self.compressed_indices {
            writer.write_tagged_u32("M3CJ", *compressed_indices);
        }
        if let Some(m3ci) = &self.m3ci {
            writer.write_tagged_data("M3CI", m3ci);
        }

        // these should come in pairs
        // TODO rework in a better way
        // if both are present, write them in pairs
        if let (Some(m3ays), Some(m3axs)) = (&self.m3ay, &self.m3ax) {
            for (m3ay, m3ax) in m3ays.iter().zip(m3axs.iter()) {
                writer.write_tagged_data("M3AY", m3ay);
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
            writer.write_tagged_with("COLR", color, Color::biff_write_bgr);
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

        // shared
        writer.write_tagged_bool("LOCK", self.is_locked);
        writer.write_tagged_u32("LAYR", self.editor_layer);
        if let Some(editor_layer_name) = &self.editor_layer_name {
            writer.write_tagged_string("LANR", editor_layer_name);
        }
        if let Some(editor_layer_visibility) = self.editor_layer_visibility {
            writer.write_tagged_bool("LVIS", editor_layer_visibility);
        }

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;

    use super::*;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::thread_rng();
        let primitive: Primitive = Primitive {
            position: Vertex3D::new(1.0, 2.0, 3.0),
            size: Vertex3D::new(4.0, 5.0, 6.0),
            rot_and_tra: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
            image: "image".to_string(),
            normal_map: Some("normal_map".to_string()),
            sides: 1,
            name: "name".to_string(),
            material: "material".to_string(),
            side_color: Color::new_bgr(0x12345678),
            is_visible: rng.gen(),
            // random bool
            draw_textures_inside: rng.gen(),
            hit_event: rng.gen(),
            threshold: 1.0,
            elasticity: 2.0,
            elasticity_falloff: 3.0,
            friction: 4.0,
            scatter: 5.0,
            edge_factor_ui: 6.0,
            collision_reduction_factor: Some(7.0),
            is_collidable: rng.gen(),
            is_toy: rng.gen(),
            use_3d_mesh: rng.gen(),
            static_rendering: rng.gen(),
            disable_lighting_top_old: Some(rng.gen()),
            disable_lighting_top: Some(rng.gen()),
            disable_lighting_below: rng.gen(),
            is_reflection_enabled: rng.gen(),
            backfaces_enabled: rng.gen(),
            physics_material: Some("physics_material".to_string()),
            overwrite_physics: rng.gen(),
            display_texture: rng.gen(),
            object_space_normal_map: rng.gen(),
            min_aa_bound: Some(vec![0, 1, 2, 3, 4, 5, 6, 7, 8]),
            max_aa_bound: Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            mesh_file_name: Some("mesh_file_name".to_string()),
            num_vertices: Some(8),
            compressed_vertices: Some(9),
            m3cx: Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            num_indices: Some(10),
            compressed_indices: Some(11),
            m3ci: Some(vec![2, 3, 4, 5, 6, 7, 8, 9, 10]),
            m3ay: Some(vec![
                vec![3, 4, 5, 6, 7, 8, 9, 10, 11],
                vec![4, 5, 6, 7, 8, 9, 10, 11, 12],
            ]),
            m3ax: Some(vec![
                vec![4, 5, 6, 7, 8, 9, 10, 11, 12],
                vec![5, 6, 7, 8, 9, 10, 11, 12, 13],
            ]),
            depth_bias: 12.0,
            add_blend: rng.gen(),
            use_depth_mask: rng.gen(),
            alpha: Some(13.0),
            color: Some(Color::new_bgr(0x23456789)),
            light_map: Some("light_map".to_string()),
            reflection_probe: Some("reflection_probe".to_string()),
            reflection_strength: Some(14.0),
            refraction_probe: Some("refraction_probe".to_string()),
            refraction_thickness: Some(15.0),
            is_locked: rng.gen(),
            editor_layer: 17,
            editor_layer_name: Some("editor_layer_name".to_string()),
            editor_layer_visibility: rng.gen(),
        };
        let mut writer = BiffWriter::new();
        Primitive::biff_write(&primitive, &mut writer);
        let primitive_read = Primitive::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(primitive, primitive_read);
    }
}
