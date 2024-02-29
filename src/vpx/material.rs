use crate::vpx::biff;
use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use crate::vpx::color::{Color, ColorJson};
use bytes::{Buf, BufMut, BytesMut};
use fake::Dummy;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};

const MAX_NAME_BUFFER: usize = 32;

#[derive(Dummy, Debug, PartialEq)]
enum MaterialType {
    Basic,
    Metal,
}

impl MaterialType {
    fn from_i32(i: i32) -> Self {
        match i {
            0 => MaterialType::Basic,
            1 => MaterialType::Metal,
            _ => panic!("Unknown MaterialType {}", i),
        }
    }
    fn to_i32(&self) -> i32 {
        match self {
            MaterialType::Basic => 0,
            MaterialType::Metal => 1,
        }
    }
}

/**
 * Only used for backward compatibility loading and saving (VPX version < 10.8)
*/
#[derive(Dummy, Debug, PartialEq)]
pub(crate) struct SaveMaterial {
    pub name: String,
    /**
     * Base color of the material
     * Can be overridden by texture on object itself
     */
    pub base_color: Color,
    /**
     * Specular of glossy layer
     */
    pub glossy_color: Color,
    /**
     * Specular of clearcoat layer
     */
    pub clearcoat_color: Color,
    /**
     * Wrap/rim lighting factor (0(off)..1(full))
     */
    pub wrap_lighting: f32,
    /**
     * Is a metal material or not
     */
    pub is_metal: bool,
    /**
     * Roughness of glossy layer (0(diffuse)..1(specular))
     */
    pub roughness: f32,
    /**
     * Use image also for the glossy layer (0(no tinting at all)..1(use image))
     * Stupid quantization because of legacy loading/saving
     */
    pub glossy_image_lerp: u8,
    /**
     * Edge weight/brightness for glossy and clearcoat (0(dark edges)..1(full fresnel))
     */
    pub edge: f32,
    /**
     * Thickness for transparent materials (0(paper thin)..1(maximum))
     * Stupid quantization because of legacy loading/saving
     */
    pub thickness: u8,
    /**
     * Opacity (0..1)
     */
    pub opacity: f32,
    /**
     * Lowest bit = on/off, upper 7bits = edge weight for fresnel (0(no opacity change)..1(full fresnel))
     * Stupid encoding because of legacy loading/saving
     */
    pub opacity_active_edge_alpha: u8,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SaveMaterialJson {
    name: String,
    base_color: ColorJson,
    glossy_color: ColorJson,
    clearcoat_color: ColorJson,
    wrap_lighting: f32,
    is_metal: bool,
    roughness: f32,
    glossy_image_lerp: u8,
    edge: f32,
    thickness: u8,
    opacity: f32,
    opacity_active_edge_alpha: u8,
}

impl SaveMaterialJson {
    pub fn from_save_material(save_material: &SaveMaterial) -> Self {
        Self {
            name: save_material.name.clone(),
            base_color: ColorJson::from_color(&save_material.base_color),
            glossy_color: ColorJson::from_color(&save_material.glossy_color),
            clearcoat_color: ColorJson::from_color(&save_material.clearcoat_color),
            wrap_lighting: save_material.wrap_lighting,
            is_metal: save_material.is_metal,
            roughness: save_material.roughness,
            glossy_image_lerp: save_material.glossy_image_lerp,
            edge: save_material.edge,
            thickness: save_material.thickness,
            opacity: save_material.opacity,
            opacity_active_edge_alpha: save_material.opacity_active_edge_alpha,
        }
    }
    pub fn to_save_material(&self) -> SaveMaterial {
        SaveMaterial {
            name: self.name.clone(),
            base_color: self.base_color.to_color(),
            glossy_color: self.glossy_color.to_color(),
            clearcoat_color: self.clearcoat_color.to_color(),
            wrap_lighting: self.wrap_lighting,
            is_metal: self.is_metal,
            roughness: self.roughness,
            glossy_image_lerp: self.glossy_image_lerp,
            edge: self.edge,
            thickness: self.thickness,
            opacity: self.opacity,
            opacity_active_edge_alpha: self.opacity_active_edge_alpha,
        }
    }
}

impl SaveMaterial {
    pub(crate) fn read(bytes: &mut BytesMut) -> SaveMaterial {
        if !bytes.has_remaining() {
            panic!("No more bytes to read SaveMaterial from");
        }
        // total should be 76 bytes
        // string can have max size of 32 bytes (including null terminator)
        let name = read_padded_cstring(bytes, MAX_NAME_BUFFER);
        let base_color = bytes.get_u32_le();
        let glossy_color = bytes.get_u32_le();
        let clearcoat_color = bytes.get_u32_le();
        let wrap_lighting = bytes.get_f32_le();
        let is_metal = bytes.get_u8() != 0;
        get_padding_3_validate(bytes);
        let roughness = bytes.get_f32_le();
        let glossy_image_lerp = bytes.get_u8();
        // TODO apply quantization to glossy_image_lerp
        get_padding_3_validate(bytes);
        let edge = bytes.get_f32_le();
        let thickness = bytes.get_u8();
        get_padding_3_validate(bytes);
        let opacity = bytes.get_f32_le();
        let opacity_active_edge_alpha = bytes.get_u8();
        // TODO split opacity_active_edge_alpha into on/off and edge weight
        get_padding_3_validate(bytes);

        SaveMaterial {
            name,
            base_color: Color::from_argb(base_color),
            glossy_color: Color::from_argb(glossy_color),
            clearcoat_color: Color::from_argb(clearcoat_color),
            wrap_lighting,
            is_metal,
            roughness,
            glossy_image_lerp,
            edge,
            thickness,
            opacity,
            opacity_active_edge_alpha,
        }
    }

    pub(crate) fn write(&self, bytes: &mut BytesMut) {
        write_padded_cstring(self.name.as_str(), bytes, MAX_NAME_BUFFER);
        bytes.put_u32_le(self.base_color.argb());
        bytes.put_u32_le(self.glossy_color.argb());
        bytes.put_u32_le(self.clearcoat_color.argb());
        bytes.put_f32_le(self.wrap_lighting);
        bytes.put_u8(if self.is_metal { 1 } else { 0 });
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_f32_le(self.roughness);
        bytes.put_u8(self.glossy_image_lerp);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_f32_le(self.edge);
        bytes.put_u8(self.thickness);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_f32_le(self.opacity);
        bytes.put_u8(self.opacity_active_edge_alpha);
        bytes.put_u8(0);
        bytes.put_u8(0);
        bytes.put_u8(0);
    }
}

/**
 * Only used for backward compatibility loading and saving (VPX version < 10.8)
 */
#[derive(Dummy, Debug, PartialEq)]
pub struct SavePhysicsMaterial {
    name: String,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter_angle: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SavePhysicsMaterialJson {
    name: String,
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter_angle: f32,
}

impl SavePhysicsMaterialJson {
    pub fn from_save_physics_material(save_physics_material: &SavePhysicsMaterial) -> Self {
        Self {
            name: save_physics_material.name.clone(),
            elasticity: save_physics_material.elasticity,
            elasticity_falloff: save_physics_material.elasticity_falloff,
            friction: save_physics_material.friction,
            scatter_angle: save_physics_material.scatter_angle,
        }
    }
    pub fn to_save_physics_material(&self) -> SavePhysicsMaterial {
        SavePhysicsMaterial {
            name: self.name.clone(),
            elasticity: self.elasticity,
            elasticity_falloff: self.elasticity_falloff,
            friction: self.friction,
            scatter_angle: self.scatter_angle,
        }
    }
}

impl SavePhysicsMaterial {
    pub(crate) fn read(bytes: &mut BytesMut) -> SavePhysicsMaterial {
        if !bytes.has_remaining() {
            panic!("No more bytes to read SavePhysicsMaterial from");
        }
        // total should be 24 bytes
        // string can have max size of 32 bytes (including null terminator)
        let name = read_padded_cstring(bytes, MAX_NAME_BUFFER);
        let elasticity = bytes.get_f32_le();
        let elasticity_falloff = bytes.get_f32_le();
        let friction = bytes.get_f32_le();
        let scatter_angle = bytes.get_f32_le();

        SavePhysicsMaterial {
            name,
            elasticity,
            elasticity_falloff,
            friction,
            scatter_angle,
        }
    }

    pub(crate) fn write(&self, bytes: &mut BytesMut) {
        // write name as cstring with fixed size of MAX_NAME_BUFFER
        write_padded_cstring(self.name.as_str(), bytes, MAX_NAME_BUFFER);
        bytes.put_f32_le(self.elasticity);
        bytes.put_f32_le(self.elasticity_falloff);
        bytes.put_f32_le(self.friction);
        bytes.put_f32_le(self.scatter_angle);
    }
}

fn write_padded_cstring(str: &str, bytes: &mut BytesMut, len: usize) {
    // write name as cstring with fixed size of len
    let cname = CString::new(str).unwrap();
    let cname_bytes = cname.as_bytes_with_nul();
    bytes.put_slice(&cname_bytes);
    bytes.put_slice(&vec![0; len - cname_bytes.len()]);
}

fn read_padded_cstring(bytes: &mut BytesMut, len: usize) -> String {
    let cname = bytes.copy_to_bytes(len);
    // read cstring and drop remaining bytes which contain random padding data
    CStr::from_bytes_until_nul(&cname)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

fn get_padding_3_validate(bytes: &mut BytesMut) {
    let padding_ = bytes.copy_to_bytes(3);
    // since we have random padding data, we can't validate it
    //assert_eq!(padding.to_vec(), [0, 0, 0]);
}

#[derive(Dummy, Debug)]
struct Material {
    name: String,

    // shading properties
    type_: MaterialType,
    wrap_lighting: f32,
    roughness: f32,
    glossy_image_lerp: f32,
    thickness: f32,
    edge: f32,
    edge_alpha: f32,
    opacity: f32,
    base_color: Color,
    glossy_color: Color,
    clearcoat_color: Color,
    opacity_active: bool,

    // physic properties
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter_angle: f32,

    refraction_tint: Color, // 10.8+ only
}

impl Default for Material {
    fn default() -> Self {
        Material {
            type_: MaterialType::Basic,
            wrap_lighting: 0.0,
            roughness: 0.0,
            glossy_image_lerp: 1.0,
            thickness: 0.05,
            edge: 1.0,
            edge_alpha: 1.0,
            opacity: 1.0,
            base_color: Color::new_argb(0xB469FF),
            glossy_color: Color::new_argb(0),
            clearcoat_color: Color::new_argb(0),
            opacity_active: false,
            elasticity: 0.0,
            elasticity_falloff: 0.0,
            friction: 0.0,
            scatter_angle: 0.0,
            refraction_tint: Color::new_argb(0xFFFFFF),
            name: "dummyMaterial".to_string(),
        }
    }
}

impl Default for SaveMaterial {
    fn default() -> Self {
        SaveMaterial {
            name: "dummyMaterial".to_string(),
            base_color: Color::new_argb(0xB469FF),
            glossy_color: Color::new_argb(0),
            clearcoat_color: Color::new_argb(0),
            wrap_lighting: 0.0,
            is_metal: false,
            roughness: 0.0,
            glossy_image_lerp: 1,
            edge: 1.0,
            thickness: 0,
            opacity: 1.0,
            opacity_active_edge_alpha: 0,
        }
    }
}

impl Default for SavePhysicsMaterial {
    fn default() -> Self {
        SavePhysicsMaterial {
            name: "dummyMaterial".to_string(),
            elasticity: 0.0,
            elasticity_falloff: 0.0,
            friction: 0.0,
            scatter_angle: 0.0,
        }
    }
}

impl Serialize for Material {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        todo!()
    }
}

impl<'de> Deserialize<'de> for Material {
    fn deserialize<D>(_deserializer: D) -> Result<Material, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        todo!()
    }
}

impl BiffRead for Material {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut material = Material::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "TYPE" => material.type_ = MaterialType::from_i32(reader.get_i32()),
                "NAME" => material.name = reader.get_string(),
                "WLIG" => material.wrap_lighting = reader.get_f32(),
                "ROUG" => material.roughness = reader.get_f32(),
                "GIML" => material.glossy_image_lerp = reader.get_f32(),
                "THCK" => {
                    // pbr->GetFloat(m_fThickness);
                    let _thickness = reader.get_f32();
                }
                "EDGE" => {
                    // pbr->GetFloat(m_fEdge);
                    let _edge = reader.get_f32();
                }
                "EALP" => {
                    // pbr->GetFloat(m_fEdgeAlpha);
                    let _edge_alpha = reader.get_f32();
                }
                "OPAC" => {
                    // pbr->GetFloat(m_fOpacity);
                    let _opacity = reader.get_f32();
                }
                "BASE" => {
                    // pbr->GetInt(m_cBase);
                    let _base = reader.get_u32();
                }
                "GLOS" => {
                    // pbr->GetInt(m_cGlossy);
                    let _glossy = reader.get_u32();
                }
                "COAT" => {
                    // pbr->GetInt(m_cClearcoat);
                    let _clearcoat = reader.get_u32();
                }
                "RTNT" => {
                    // pbr->GetInt(m_cRefractionTint);
                    let _refraction_tint = reader.get_u32();
                }
                "EOPA" => {
                    // pbr->GetBool(m_bOpacityActive);
                    let _opacity_active = reader.get_bool();
                }
                "ELAS" => {
                    // pbr->GetFloat(m_fElasticity);
                    let _elasticity = reader.get_f32();
                }
                "ELFO" => {
                    // pbr->GetFloat(m_fElasticityFalloff
                    let _elasticity_falloff = reader.get_f32();
                }
                "FRIC" => {
                    // pbr->GetFloat(m_fFriction);
                    let _friction = reader.get_f32();
                }
                "SCAT" => {
                    // pbr->GetFloat(m_fScatterAngle);
                    let _scatter_angle = reader.get_f32();
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
        material
    }
}

impl BiffWrite for Material {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_tagged_i32("TYPE", self.type_.to_i32());
        writer.write_tagged_string("NAME", &self.name);
        writer.write_tagged_f32("WLIG", self.wrap_lighting);
        writer.write_tagged_f32("ROUG", self.roughness);
        writer.write_tagged_f32("GIML", self.glossy_image_lerp);
        writer.write_tagged_f32("THCK", self.thickness);
        writer.write_tagged_f32("EDGE", self.edge);
        writer.write_tagged_f32("EALP", self.edge_alpha);
        writer.write_tagged_f32("OPAC", self.opacity);
        writer.write_tagged_u32("BASE", self.base_color.argb());
        writer.write_tagged_u32("GLOS", self.glossy_color.argb());
        writer.write_tagged_u32("COAT", self.clearcoat_color.argb());
        writer.write_tagged_u32("RTNT", self.refraction_tint.argb());
        writer.write_tagged_bool("EOPA", self.opacity_active);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("FRIC", self.friction);
        writer.write_tagged_f32("SCAT", self.scatter_angle);
        writer.close(true);
    }
}

impl BiffRead for SaveMaterial {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut save_material = SaveMaterial::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
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
        save_material
    }
}

impl BiffWrite for SaveMaterial {
    fn biff_write(&self, writer: &mut BiffWriter) {
        todo!()
    }
}

impl BiffRead for SavePhysicsMaterial {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut save_material = SavePhysicsMaterial::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
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
        save_material
    }
}

impl BiffWrite for SavePhysicsMaterial {
    fn biff_write(&self, writer: &mut BiffWriter) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use fake::{Fake, Faker};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_save_material_write_read() {
        let save_material: SaveMaterial = Faker.fake();
        let mut bytes = BytesMut::new();
        save_material.write(&mut bytes);
        // is there a better way to reset the cursor?
        bytes = BytesMut::from(bytes.to_vec().as_slice());
        let read_save_material = SaveMaterial::read(&mut bytes);
        assert_eq!(save_material, read_save_material);
    }

    #[test]
    fn test_save_physics_material_write_read() {
        let save_physics_material: SavePhysicsMaterial = Faker.fake();
        let mut bytes = BytesMut::new();
        save_physics_material.write(&mut bytes);
        // is there a better way to reset the cursor?
        bytes = BytesMut::from(bytes.to_vec().as_slice());
        let read_save_physics_material = SavePhysicsMaterial::read(&mut bytes);
        assert_eq!(save_physics_material, read_save_physics_material);
    }
}
