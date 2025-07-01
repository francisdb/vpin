use crate::vpx::biff;
use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use crate::vpx::color::Color;
use crate::vpx::json::F32WithNanInf;
use crate::vpx::math::quantize_u8;
use bytes::{Buf, BufMut, BytesMut};
use encoding_rs::mem::{decode_latin1, encode_latin1_lossy};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ffi::CStr;
use std::io;

const MAX_NAME_BUFFER: usize = 32;

#[derive(Dummy, Debug, Clone, PartialEq)]
pub enum MaterialType {
    Unknown = -1, // found in Hot Line (Williams 1966) SG1bsoN.vpx
    Basic = 0,
    Metal = 1,
}

impl From<i32> for MaterialType {
    fn from(value: i32) -> Self {
        match value {
            -1 => MaterialType::Unknown,
            0 => MaterialType::Basic,
            1 => MaterialType::Metal,
            _ => panic!("Invalid MaterialType {value}"),
        }
    }
}

impl From<&MaterialType> for i32 {
    fn from(value: &MaterialType) -> Self {
        match value {
            MaterialType::Unknown => -1,
            MaterialType::Basic => 0,
            MaterialType::Metal => 1,
        }
    }
}

/// Serialize to lowercase string
impl Serialize for MaterialType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MaterialType::Unknown => serializer.serialize_str("unknown"),
            MaterialType::Basic => serializer.serialize_str("basic"),
            MaterialType::Metal => serializer.serialize_str("metal"),
        }
    }
}

/// Deserialize from lowercase string
/// or case-insensitive string for backwards compatibility
impl<'de> Deserialize<'de> for MaterialType {
    fn deserialize<D>(deserializer: D) -> Result<MaterialType, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MaterialTypeVisitor;

        impl serde::de::Visitor<'_> for MaterialTypeVisitor {
            type Value = MaterialType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string representing a MaterialType")
            }

            fn visit_str<E>(self, value: &str) -> Result<MaterialType, E>
            where
                E: serde::de::Error,
            {
                match value.to_lowercase().as_str() {
                    "unknown" => Ok(MaterialType::Unknown),
                    "basic" => Ok(MaterialType::Basic),
                    "metal" => Ok(MaterialType::Metal),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["basic", "metal"],
                    )),
                }
            }
        }

        deserializer.deserialize_str(MaterialTypeVisitor)
    }
}

/**
 * Only used for backward compatibility loading and saving (VPX version < 10.8)
*/
#[derive(Dummy, Debug, PartialEq)]
pub struct SaveMaterial {
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

impl From<&Material> for SaveMaterial {
    fn from(material: &Material) -> Self {
        // FIXME this is the code used on the vpinball side

        // template <typename T>
        //     __forceinline T clamp(const T x, const T mn, const T mx)
        // {
        //     return max(min(x,mx),mn);
        // }
        //
        // __forceinline int clamp(const int x, const int mn, const int mx)
        // {
        //     if (x < mn) return mn; else if (x > mx) return mx; else return x;
        // }
        // template <unsigned char bits> // bits to map to
        //     __forceinline float dequantizeUnsigned(const unsigned int i)
        // {
        //     enum { N = (1 << bits) - 1 };
        //     return min(precise_divide((float)i, (float)N), 1.f); //!! test: optimize div or does this break precision?
        // }
        //
        // template <unsigned char bits> // bits to map to
        //     __forceinline unsigned int quantizeUnsigned(const float x)
        // {
        //     enum { N = (1 << bits) - 1, Np1 = (1 << bits) };
        //     assert(x >= 0.f);
        //     return min((unsigned int)(x * (float)Np1), (unsigned int)N);
        // }

        // mats[i].fGlossyImageLerp = 255 - quantizeUnsigned<8>(clamp(m->m_fGlossyImageLerp, 0.f, 1.f)); // '255 -' to be compatible with previous table versions
        // '255 -' to be compatible with previous table versions
        let glossy_image_lerp: u8 =
            255 - quantize_u8(8, material.glossy_image_lerp.clamp(0.0, 1.0));

        // mats[i].fThickness = quantizeUnsigned<8>(clamp(m->m_fThickness, 0.05f, 1.f)); // clamp with 0.05f to be compatible with previous table versions
        // clamp with 0.05f to be compatible with previous table versions
        let thickness: u8 = quantize_u8(8, material.thickness.clamp(0.05, 1.0));

        // mats[i].bOpacityActive_fEdgeAlpha = m->m_bOpacityActive ? 1 : 0;
        // mats[i].bOpacityActive_fEdgeAlpha |= quantizeUnsigned<7>(clamp(m->m_fEdgeAlpha, 0.f, 1.f)) << 1;
        let mut opacity_active_edge_alpha: u8 = if material.opacity_active { 1 } else { 0 };
        opacity_active_edge_alpha |= quantize_u8(7, material.edge_alpha.clamp(0.0, 1.0)) << 1;

        SaveMaterial {
            name: material.name.clone(),
            base_color: material.base_color,
            glossy_color: material.glossy_color,
            clearcoat_color: material.clearcoat_color,
            wrap_lighting: material.wrap_lighting,
            is_metal: material.type_ == MaterialType::Metal,
            roughness: material.roughness,
            glossy_image_lerp,
            edge: material.edge,
            thickness,
            opacity: material.opacity,
            opacity_active_edge_alpha,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SaveMaterialJson {
    name: String,
    base_color: Color,
    glossy_color: Color,
    clearcoat_color: Color,
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
            base_color: save_material.base_color,
            glossy_color: save_material.glossy_color,
            clearcoat_color: save_material.clearcoat_color,
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
            base_color: self.base_color,
            glossy_color: self.glossy_color,
            clearcoat_color: self.clearcoat_color,
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
        let name = read_padded_cstring(bytes, MAX_NAME_BUFFER).unwrap();
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
            base_color: Color::from_win_color(base_color),
            glossy_color: Color::from_win_color(glossy_color),
            clearcoat_color: Color::from_win_color(clearcoat_color),
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
        write_padded_cstring_truncate(self.name.as_str(), bytes, MAX_NAME_BUFFER);
        bytes.put_u32_le(self.base_color.to_win_color());
        bytes.put_u32_le(self.glossy_color.to_win_color());
        bytes.put_u32_le(self.clearcoat_color.to_win_color());
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

impl From<&Material> for SavePhysicsMaterial {
    fn from(material: &Material) -> Self {
        SavePhysicsMaterial {
            name: material.name.clone(),
            elasticity: material.elasticity,
            elasticity_falloff: material.elasticity_falloff,
            friction: material.friction,
            scatter_angle: material.scatter_angle,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SavePhysicsMaterialJson {
    name: String,
    elasticity: F32WithNanInf,
    elasticity_falloff: F32WithNanInf,
    friction: F32WithNanInf,
    scatter_angle: F32WithNanInf,
}

impl SavePhysicsMaterialJson {
    pub fn from_save_physics_material(save_physics_material: &SavePhysicsMaterial) -> Self {
        Self {
            name: save_physics_material.name.clone(),
            elasticity: save_physics_material.elasticity.into(),
            elasticity_falloff: save_physics_material.elasticity_falloff.into(),
            friction: save_physics_material.friction.into(),
            scatter_angle: save_physics_material.scatter_angle.into(),
        }
    }
    pub fn to_save_physics_material(&self) -> SavePhysicsMaterial {
        SavePhysicsMaterial {
            name: self.name.clone(),
            elasticity: self.elasticity.into(),
            elasticity_falloff: self.elasticity_falloff.into(),
            friction: self.friction.into(),
            scatter_angle: self.scatter_angle.into(),
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
        let name = read_padded_cstring(bytes, MAX_NAME_BUFFER).unwrap();
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
        write_padded_cstring_truncate(self.name.as_str(), bytes, MAX_NAME_BUFFER);
        bytes.put_f32_le(self.elasticity);
        bytes.put_f32_le(self.elasticity_falloff);
        bytes.put_f32_le(self.friction);
        bytes.put_f32_le(self.scatter_angle);
    }
}

/**
 * Writes a padded cstring to bytes
 * Fills remaining bytes with 0
 * The string is encoded as latin1
 */
fn write_padded_cstring_truncate(str: &str, bytes: &mut BytesMut, len: usize) {
    let mut latin1_bytes = encode_latin1_lossy(str).into_owned();
    if latin1_bytes.len() > len - 1 {
        latin1_bytes.truncate(len - 1);
    }
    bytes.put_slice(&latin1_bytes);
    // put terminator
    bytes.put_u8(0);
    // fill
    bytes.put_slice(&vec![0; len - latin1_bytes.len() - 1]);
}

/**
 * Reads a padded cstring from bytes and returns the string
 * Drops remaining bytes (which may contain random padding data in vpx files)
 */
fn read_padded_cstring(bytes: &mut BytesMut, len: usize) -> Result<String, io::Error> {
    let cname = bytes.copy_to_bytes(len);
    let cstr = CStr::from_bytes_until_nul(&cname)
        .map_err(|_e| io::Error::other("Failed to read null-padded string from bytes"))?;
    let s = decode_latin1(cstr.to_bytes());
    Ok(s.to_string())
}

fn get_padding_3_validate(bytes: &mut BytesMut) {
    bytes.advance(3);
    //let padding = bytes.copy_to_bytes(3);
    // since we have random padding data, we can't validate it
    //assert_eq!(padding.to_vec(), [0, 0, 0]);
}

#[derive(Dummy, Debug, PartialEq)]
pub struct Material {
    pub name: String,

    // shading properties
    pub type_: MaterialType,
    pub wrap_lighting: f32,
    pub roughness: f32,
    pub glossy_image_lerp: f32,
    pub thickness: f32,
    pub edge: f32,
    pub edge_alpha: f32,
    pub opacity: f32,
    pub base_color: Color,
    pub glossy_color: Color,
    pub clearcoat_color: Color,
    // Transparency active in the UI
    pub opacity_active: bool,

    // physic properties
    elasticity: f32,
    elasticity_falloff: f32,
    friction: f32,
    scatter_angle: f32,

    refraction_tint: Color, // 10.8+ only
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MaterialJson {
    name: String,
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
    elasticity: F32WithNanInf,
    elasticity_falloff: F32WithNanInf,
    friction: F32WithNanInf,
    scatter_angle: F32WithNanInf,
    refraction_tint: Color,
}

impl MaterialJson {
    pub fn from_material(material: &Material) -> Self {
        Self {
            name: material.name.clone(),
            type_: material.type_.clone(),
            wrap_lighting: material.wrap_lighting,
            roughness: material.roughness,
            glossy_image_lerp: material.glossy_image_lerp,
            thickness: material.thickness,
            edge: material.edge,
            edge_alpha: material.edge_alpha,
            opacity: material.opacity,
            base_color: material.base_color,
            glossy_color: material.glossy_color,
            clearcoat_color: material.clearcoat_color,
            opacity_active: material.opacity_active,
            elasticity: material.elasticity.into(),
            elasticity_falloff: material.elasticity_falloff.into(),
            friction: material.friction.into(),
            scatter_angle: material.scatter_angle.into(),
            refraction_tint: material.refraction_tint,
        }
    }
    pub fn to_material(&self) -> Material {
        Material {
            name: self.name.clone(),
            type_: self.type_.clone(),
            wrap_lighting: self.wrap_lighting,
            roughness: self.roughness,
            glossy_image_lerp: self.glossy_image_lerp,
            thickness: self.thickness,
            edge: self.edge,
            edge_alpha: self.edge_alpha,
            opacity: self.opacity,
            base_color: self.base_color,
            glossy_color: self.glossy_color,
            clearcoat_color: self.clearcoat_color,
            opacity_active: self.opacity_active,
            elasticity: self.elasticity.into(),
            elasticity_falloff: self.elasticity_falloff.into(),
            friction: self.friction.into(),
            scatter_angle: self.scatter_angle.into(),
            refraction_tint: self.refraction_tint,
        }
    }
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
            base_color: Color::from_rgb(0xB469FF), // Purple / Heliotrope
            glossy_color: Color::BLACK,
            clearcoat_color: Color::BLACK,
            opacity_active: false,
            elasticity: 0.0,
            elasticity_falloff: 0.0,
            friction: 0.0,
            scatter_angle: 0.0,
            refraction_tint: Color::WHITE,
            name: "dummyMaterial".to_string(),
        }
    }
}

impl Default for SaveMaterial {
    fn default() -> Self {
        SaveMaterial {
            name: "dummyMaterial".to_string(),
            base_color: Color::from_rgb(0xB469FF),
            glossy_color: Color::BLACK,
            clearcoat_color: Color::BLACK,
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
        MaterialJson::from_material(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Material {
    fn deserialize<D>(_deserializer: D) -> Result<Material, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let material_json = MaterialJson::deserialize(_deserializer)?;
        Ok(material_json.to_material())
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
                "TYPE" => material.type_ = reader.get_i32().into(),
                "NAME" => material.name = reader.get_string(),
                "WLIG" => material.wrap_lighting = reader.get_f32(),
                "ROUG" => material.roughness = reader.get_f32(),
                "GIML" => material.glossy_image_lerp = reader.get_f32(),
                "THCK" => material.thickness = reader.get_f32(),
                "EDGE" => material.edge = reader.get_f32(),
                "EALP" => material.edge_alpha = reader.get_f32(),
                "OPAC" => material.opacity = reader.get_f32(),
                "BASE" => material.base_color = Color::biff_read(reader),
                "GLOS" => material.glossy_color = Color::biff_read(reader),
                "COAT" => material.clearcoat_color = Color::biff_read(reader),
                "RTNT" => material.refraction_tint = Color::biff_read(reader),
                "EOPA" => material.opacity_active = reader.get_bool(),
                "ELAS" => material.elasticity = reader.get_f32(),
                "ELFO" => material.elasticity_falloff = reader.get_f32(),
                "FRIC" => material.friction = reader.get_f32(),
                "SCAT" => material.scatter_angle = reader.get_f32(),
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
        writer.write_tagged_i32("TYPE", (&self.type_).into());
        writer.write_tagged_string("NAME", &self.name);
        writer.write_tagged_f32("WLIG", self.wrap_lighting);
        writer.write_tagged_f32("ROUG", self.roughness);
        writer.write_tagged_f32("GIML", self.glossy_image_lerp);
        writer.write_tagged_f32("THCK", self.thickness);
        writer.write_tagged_f32("EDGE", self.edge);
        writer.write_tagged_f32("EALP", self.edge_alpha);
        writer.write_tagged_f32("OPAC", self.opacity);
        writer.write_tagged_with("BASE", &self.base_color, Color::biff_write);
        writer.write_tagged_with("GLOS", &self.glossy_color, Color::biff_write);
        writer.write_tagged_with("COAT", &self.clearcoat_color, Color::biff_write);
        writer.write_tagged_with("RTNT", &self.refraction_tint, Color::biff_write);
        writer.write_tagged_bool("EOPA", self.opacity_active);
        writer.write_tagged_f32("ELAS", self.elasticity);
        writer.write_tagged_f32("ELFO", self.elasticity_falloff);
        writer.write_tagged_f32("FRIC", self.friction);
        writer.write_tagged_f32("SCAT", self.scatter_angle);
        writer.close(true);
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

    #[test]
    fn test_material_biff_write_read() {
        let material: Material = Faker.fake();
        let mut writer = BiffWriter::new();
        material.biff_write(&mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let read_material = Material::biff_read(&mut reader);
        assert_eq!(material, read_material);
    }

    #[test]
    fn test_padded_cstring() {
        let s = "test";
        let mut bytes = BytesMut::new();
        write_padded_cstring_truncate(s, &mut bytes, 32);
        let read_s = read_padded_cstring(&mut bytes, 32).unwrap();
        assert_eq!(s, read_s);
    }

    #[test]
    fn test_padded_cstring_truncated() {
        let s = "A too long string that should be truncated";
        let mut bytes = BytesMut::new();
        write_padded_cstring_truncate(s, &mut bytes, 8);
        let read_s = read_padded_cstring(&mut bytes, 8).unwrap();
        assert_eq!("A too l", read_s);
    }

    #[test]
    fn test_material_to_save_material() {
        let material = Material {
            name: "test".to_string(),
            type_: MaterialType::Basic,
            wrap_lighting: 0.5,
            roughness: 0.5,
            glossy_image_lerp: 0.1,
            thickness: 0.5,
            edge: 0.5,
            edge_alpha: 0.9,
            opacity: 0.5,
            base_color: Faker.fake(),
            glossy_color: Faker.fake(),
            clearcoat_color: Faker.fake(),
            opacity_active: true,
            elasticity: 0.5,
            elasticity_falloff: 0.5,
            friction: 0.5,
            scatter_angle: 0.5,
            refraction_tint: Faker.fake(),
        };
        let save_material: SaveMaterial = (&material).into();
        assert_eq!(save_material.name, "test");
        assert_eq!(save_material.glossy_image_lerp, 230);
        assert_eq!(save_material.thickness, 128);
        assert_eq!(save_material.opacity_active_edge_alpha, 231);
    }

    #[test]
    fn test_material_type_json() {
        let sizing_type = MaterialType::Metal;
        let json = serde_json::to_string(&sizing_type).unwrap();
        assert_eq!(json, "\"metal\"");
        let sizing_type_read: MaterialType = serde_json::from_str(&json).unwrap();
        assert_eq!(sizing_type, sizing_type_read);
    }

    #[test]
    #[should_panic = "Error(\"unknown variant `foo`, expected `basic` or `metal`\", line: 0, column: 0)"]
    fn test_material_type_json_fail() {
        let json = serde_json::Value::from("foo");
        let _: MaterialType = serde_json::from_value(json).unwrap();
    }
}
