use super::vertex2d::Vertex2D;
use crate::impl_shared_attributes;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Spinner {
    pub name: String,
    /// BIFF tag: VCEN
    pub center: Vertex2D,
    /// Rotation in degrees, where 0 is the default position,
    /// positive is clockwise, and negative is counterclockwise
    ///
    /// BIFF tag: ROTA
    pub rotation: f32,
    /// Height offset of the spinner above its surface in VPU.
    ///
    /// The spinner's z position is calculated as: `surface_height + height`
    ///
    /// Default: 60.0
    ///
    /// BIFF tag: HIGH
    pub height: f32,
    /// Length of the spinner plate in VPU.
    ///
    /// This value is used to scale the spinner mesh uniformly in x, y, and z.
    /// The mesh vertices are multiplied by this value.
    ///
    /// Default: 80.0
    ///
    /// BIFF tag: LGTH
    pub length: f32,
    /// Damping (anti-friction) factor for the spinner's rotation.
    ///
    /// Controls how quickly the spinner loses rotational energy.
    /// Higher values mean less friction and longer spin times.
    /// Range: 0.0 to 1.0, where 1.0 is no friction.
    ///
    /// Also known as "AntiFriction" in the VPinball UI.
    ///
    /// Default: 0.9879
    ///
    /// BIFF tag: AFRC
    pub damping: f32,
    /// Maximum rotation angle in degrees (for limited angle mode).
    ///
    /// When `angle_max` equals `angle_min`, the spinner rotates freely (360°).
    /// When they differ, the spinner is constrained between these angles.
    ///
    /// Default: 0.0
    ///
    /// BIFF tag: SMAX
    pub angle_max: f32,
    /// Minimum rotation angle in degrees (for limited angle mode).
    ///
    /// When `angle_max` equals `angle_min`, the spinner rotates freely (360°).
    /// When they differ, the spinner is constrained between these angles.
    ///
    /// Default: 0.0
    ///
    /// BIFF tag: SMIN
    pub angle_min: f32,
    /// Elasticity (bounciness) of the spinner when hit by the ball.
    ///
    /// Higher values mean more energy is transferred to the ball on collision.
    ///
    /// Default: 0.3
    ///
    /// BIFF tag: SELA
    pub elasticity: f32,
    /// Whether the spinner plate is visible.
    ///
    /// When `false`, the spinner is invisible but still interacts with the ball.
    ///
    /// Default: true
    ///
    /// BIFF tag: SVIS
    pub is_visible: bool,
    /// Whether to show the mounting bracket.
    ///
    /// The bracket is the fixed metal frame that holds the spinner plate.
    ///
    /// Default: true
    ///
    /// BIFF tag: SSUP
    pub show_bracket: bool,
    /// Material name for the spinner plate.
    ///
    /// References a material defined in the table's material list.
    /// Controls the appearance (color, reflectivity, etc.) of the plate.
    ///
    /// Default: empty string (uses default material)
    ///
    /// BIFF tag: MATR
    pub material: String,
    /// Image/texture name for the spinner plate.
    ///
    /// References an image defined in the table's image list.
    /// Applied to the plate mesh surface.
    ///
    /// The texture is a texture atlas, you can base it on the "Spinner" texture
    /// in the example table.
    ///
    /// Default: empty string (no texture)
    ///
    /// BIFF tag: IMGF
    pub image: String,
    /// Name of the surface (ramp or wall top) this spinner sits on.
    /// Used to determine the spinner's base height (z position).
    /// If empty, the spinner sits on the playfield.
    /// BIFF tag: SURF
    pub surface: String,
    /// Whether this spinner appears in playfield reflections.
    ///
    /// When `true`, the ball is rendered in the reflection pass.
    /// When `false`, the ball won't appear as a reflection on the playfield.
    ///
    /// BIFF tag: `REEN` (was missing in 10.01)
    pub is_reflection_enabled: Option<bool>,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Spinner);

#[derive(Debug, Serialize, Deserialize)]
struct SpinnerJson {
    center: Vertex2D,
    rotation: f32,
    #[serde(flatten)]
    pub timer: TimerData,
    height: f32,
    length: f32,
    damping: f32,
    angle_max: f32,
    angle_min: f32,
    elasticity: f32,
    is_visible: bool,
    show_bracket: bool,
    material: String,
    image: String,
    surface: String,
    name: String,
    is_reflection_enabled: Option<bool>, // added in ?
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl SpinnerJson {
    pub fn from_spinner(spinner: &Spinner) -> Self {
        Self {
            center: spinner.center,
            rotation: spinner.rotation,
            timer: spinner.timer.clone(),
            height: spinner.height,
            length: spinner.length,
            damping: spinner.damping,
            angle_max: spinner.angle_max,
            angle_min: spinner.angle_min,
            elasticity: spinner.elasticity,
            is_visible: spinner.is_visible,
            show_bracket: spinner.show_bracket,
            material: spinner.material.clone(),
            image: spinner.image.clone(),
            surface: spinner.surface.clone(),
            name: spinner.name.clone(),
            is_reflection_enabled: spinner.is_reflection_enabled,
            part_group_name: spinner.part_group_name.clone(),
        }
    }

    pub fn to_spinner(&self) -> Spinner {
        Spinner {
            center: self.center,
            rotation: self.rotation,
            timer: self.timer.clone(),
            height: self.height,
            length: self.length,
            damping: self.damping,
            angle_max: self.angle_max,
            angle_min: self.angle_min,
            elasticity: self.elasticity,
            is_visible: self.is_visible,
            show_bracket: self.show_bracket,
            material: self.material.clone(),
            image: self.image.clone(),
            surface: self.surface.clone(),
            name: self.name.clone(),
            is_reflection_enabled: self.is_reflection_enabled,
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

impl Default for Spinner {
    fn default() -> Self {
        Self {
            center: Default::default(),
            rotation: 0.0,
            timer: TimerData::default(),
            height: 60.0,
            length: 80.0,
            damping: 0.9879,
            angle_max: 0.0,
            angle_min: 0.0,
            elasticity: 0.3,
            is_visible: true,
            show_bracket: true,
            material: Default::default(),
            image: Default::default(),
            surface: Default::default(),
            name: Default::default(),
            is_reflection_enabled: None,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl Serialize for Spinner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SpinnerJson::from_spinner(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Spinner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let spinner_json = SpinnerJson::deserialize(deserializer)?;
        Ok(spinner_json.to_spinner())
    }
}

impl BiffRead for Spinner {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut spinner = Self::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    spinner.center = Vertex2D::biff_read(reader);
                }
                "ROTA" => {
                    spinner.rotation = reader.get_f32();
                }
                "HIGH" => {
                    spinner.height = reader.get_f32();
                }
                "LGTH" => {
                    spinner.length = reader.get_f32();
                }
                "AFRC" => {
                    spinner.damping = reader.get_f32();
                }
                "SMAX" => {
                    spinner.angle_max = reader.get_f32();
                }
                "SMIN" => {
                    spinner.angle_min = reader.get_f32();
                }
                "SELA" => {
                    spinner.elasticity = reader.get_f32();
                }
                "SVIS" => {
                    spinner.is_visible = reader.get_bool();
                }
                "SSUP" => {
                    spinner.show_bracket = reader.get_bool();
                }
                "MATR" => {
                    spinner.material = reader.get_string();
                }
                "IMGF" => {
                    spinner.image = reader.get_string();
                }
                "SURF" => {
                    spinner.surface = reader.get_string();
                }
                "NAME" => {
                    spinner.name = reader.get_wide_string();
                }
                "REEN" => {
                    spinner.is_reflection_enabled = Some(reader.get_bool());
                }
                _ => {
                    if !spinner.timer.biff_read_tag(tag_str, reader)
                        && !spinner.read_shared_attribute(tag_str, reader)
                    {
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
        spinner
    }
}

impl BiffWrite for Spinner {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_f32("ROTA", self.rotation);
        self.timer.biff_write(writer);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_f32("LGTH", self.length);
        writer.write_tagged_f32("AFRC", self.damping);
        writer.write_tagged_f32("SMAX", self.angle_max);
        writer.write_tagged_f32("SMIN", self.angle_min);
        writer.write_tagged_f32("SELA", self.elasticity);
        writer.write_tagged_bool("SVIS", self.is_visible);
        writer.write_tagged_bool("SSUP", self.show_bracket);
        writer.write_tagged_string("MATR", &self.material);
        writer.write_tagged_string("IMGF", &self.image);
        writer.write_tagged_string("SURF", &self.surface);
        writer.write_tagged_wide_string("NAME", &self.name);
        if let Some(is_reflection_enabled) = self.is_reflection_enabled {
            writer.write_tagged_bool("REEN", is_reflection_enabled);
        }

        self.write_shared_attributes(writer);

        writer.close(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;

    use super::*;
    use crate::vpx::gameitem::tests::RandomOption;
    use pretty_assertions::assert_eq;
    use rand::Rng;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        // values not equal to the defaults
        let spinner = Spinner {
            center: Vertex2D::new(rng.random(), rng.random()),
            rotation: rng.random(),
            timer: TimerData {
                is_enabled: rng.random(),
                interval: rng.random(),
            },
            height: rng.random(),
            length: rng.random(),
            damping: rng.random(),
            angle_max: rng.random(),
            angle_min: rng.random(),
            elasticity: rng.random(),
            is_visible: rng.random(),
            show_bracket: rng.random(),
            material: "test material".to_string(),
            image: "test image".to_string(),
            surface: "test surface".to_string(),
            name: "test name".to_string(),
            is_reflection_enabled: rng.random_option(),
            is_locked: rng.random(),
            editor_layer: Some(rng.random()),
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: rng.random_option(),
            part_group_name: Some("test group name".to_string()),
        };
        let mut writer = BiffWriter::new();
        Spinner::biff_write(&spinner, &mut writer);
        let spinner_read = Spinner::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(spinner, spinner_read);
    }
}
