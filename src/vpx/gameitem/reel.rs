use super::vertex2d::Vertex2D;
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use crate::vpx::{
    biff::{self, BiffError, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Reel {
    /// Top-left corner of the reel group's bounding box, in editor
    /// background coordinates. This is the object's anchor position (what
    /// vpinball returns as its center); [`Self::ver2`] is derived from it.
    ///
    /// BIFF tag: `VER1`
    pub ver1: Vertex2D,
    /// Bottom-right corner of the reel group's bounding box. vpinball
    /// recomputes this from [`Self::ver1`] plus the total box size (derived
    /// from reel count, width, height and spacing), so it is effectively a
    /// cached value rather than an independent setting.
    ///
    /// BIFF tag: `VER2`
    pub ver2: Vertex2D,
    /// Colour of the background box drawn behind the reels. Defaults to
    /// RGB(64, 64, 64).
    ///
    /// BIFF tag: `CLRB`
    pub back_color: Color,

    /// Whether the background box is transparent (i.e. only the reel digits
    /// are drawn, not the backing rectangle). Defaults to `false`.
    ///
    /// BIFF tag: `TRNS`
    pub is_transparent: bool,
    /// Name of the image (texture) containing the reel digit strip. The digits
    /// are laid out either as a single horizontal strip or as a grid (see
    /// [`Self::use_image_grid`]). HDR images (.exr/.hdr) are rejected by the
    /// editor.
    ///
    /// BIFF tag: `IMAG`
    pub image: String,
    /// Name of the sound to play for each single-digit click as a reel turns.
    /// Empty or `<None>` means no sound.
    ///
    /// BIFF tag: `SOUN`
    pub sound: String,
    /// Name of this game item.
    ///
    /// BIFF tag: `NAME`
    pub name: String,
    /// Width of each individual reel digit, in editor background units.
    /// Defaults to 30.0. vpinball clamps this to be non-negative.
    ///
    /// BIFF tag: `WDTH`
    pub width: f32,
    /// Height of each individual reel digit, in editor background units.
    /// Defaults to 40.0. vpinball clamps this to be non-negative.
    ///
    /// BIFF tag: `HIGH`
    pub height: f32,
    /// Number of individual reels (digits) in the set. Defaults to 5.0.
    /// Although stored here as an `f32`, vpinball treats it as an integer
    /// clamped to the range 1..=MAX_REELS.
    ///
    /// BIFF tag: `RCNT`
    pub reel_count: f32,
    /// Spacing between each reel and around the borders of the background box,
    /// in editor background units. Defaults to 4.0. vpinball clamps this to be
    /// non-negative. (The `boarders` typo in the original comment means
    /// "borders".)
    ///
    /// BIFF tag: `RSPC`
    pub reel_spacing: f32,
    /// Number of motor steps (animation frames) used to roll each reel by one
    /// digit. Defaults to 2.0. Stored as `f32` but treated by vpinball as an
    /// integer clamped to at least 1.
    ///
    /// BIFF tag: `MSTP`
    pub motor_steps: f32,
    /// Highest digit value a single reel can show (the reel cycles through
    /// `0..=digit_range`), so it is one less than the number of distinct
    /// digit images. Usually 9 (decimal digits); defaults to 9.0. Stored as
    /// `f32` but treated by vpinball as an integer clamped to 0..=511.
    ///
    /// BIFF tag: `RANG`
    pub digit_range: f32,
    /// Time in milliseconds between animation updates. Defaults to 50.
    /// vpinball clamps this to at least 5.
    ///
    /// BIFF tag: `UPTM`
    pub update_interval: u32,
    /// Whether the digit image is arranged as a 2D grid rather than a single
    /// horizontal strip. When `true`, [`Self::images_per_grid_row`] gives the
    /// number of columns. Defaults to `false`.
    ///
    /// BIFF tag: `UGRD`
    pub use_image_grid: bool,
    /// Whether the reel group is rendered in-game. Defaults to `true`.
    ///
    /// BIFF tag: `VISI`
    pub is_visible: bool,
    /// Number of digit images per row when [`Self::use_image_grid`] is `true`;
    /// ignored otherwise. Defaults to 1. vpinball clamps this to at least 1.
    ///
    /// BIFF tag: `GIPR`
    pub images_per_grid_row: u32,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    /// Whether the item is locked in the editor to prevent accidental
    /// moving or editing. Editor-only; has no runtime effect.
    ///
    /// BIFF tag: `LOCK`
    pub is_locked: bool,
    /// Legacy editor layer index. Removed in 10.8.1, superseded by part
    /// groups (see `part_group_name`). `None` when absent.
    ///
    /// BIFF tag: `LAYR`
    pub editor_layer: Option<u32>,
    /// Display name of the legacy editor layer; defaults to
    /// `"Layer_{editor_layer + 1}"`. Editor-only. `None` when absent.
    ///
    /// BIFF tag: `LANR`
    pub editor_layer_name: Option<String>,
    /// Whether the legacy editor layer is shown in the editor.
    /// Editor-only; has no runtime effect. `None` when absent.
    ///
    /// BIFF tag: `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Reel);

#[derive(Debug, Serialize, Deserialize)]
struct ReelJson {
    ver1: Vertex2D,
    ver2: Vertex2D,
    back_color: Color,
    #[serde(flatten)]
    /// Timer state (enabled flag and interval in ms) that drives this
    /// item's script `_Timer` events. See [`TimerData`].
    pub timer: TimerData,
    is_transparent: bool,
    image: String,
    sound: String,
    name: String,
    width: f32,
    height: f32,
    reel_count: f32,
    reel_spacing: f32,
    motor_steps: f32,
    digit_range: f32,
    update_interval: u32,
    use_image_grid: bool,
    is_visible: bool,
    images_per_grid_row: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl ReelJson {
    pub fn from_reel(reel: &Reel) -> Self {
        Self {
            ver1: reel.ver1,
            ver2: reel.ver2,
            back_color: reel.back_color,
            timer: reel.timer.clone(),
            is_transparent: reel.is_transparent,
            image: reel.image.clone(),
            sound: reel.sound.clone(),
            name: reel.name.clone(),
            width: reel.width,
            height: reel.height,
            reel_count: reel.reel_count,
            reel_spacing: reel.reel_spacing,
            motor_steps: reel.motor_steps,
            digit_range: reel.digit_range,
            update_interval: reel.update_interval,
            use_image_grid: reel.use_image_grid,
            is_visible: reel.is_visible,
            images_per_grid_row: reel.images_per_grid_row,
            part_group_name: reel.part_group_name.clone(),
        }
    }
    pub fn to_reel(&self) -> Reel {
        Reel {
            ver1: self.ver1,
            ver2: self.ver2,
            back_color: self.back_color,
            timer: self.timer.clone(),
            is_transparent: self.is_transparent,
            image: self.image.clone(),
            sound: self.sound.clone(),
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            reel_count: self.reel_count,
            reel_spacing: self.reel_spacing,
            motor_steps: self.motor_steps,
            digit_range: self.digit_range,
            update_interval: self.update_interval,
            use_image_grid: self.use_image_grid,
            is_visible: self.is_visible,
            images_per_grid_row: self.images_per_grid_row,
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

impl Default for Reel {
    fn default() -> Self {
        Self {
            ver1: Vertex2D::default(),
            ver2: Vertex2D::default(),
            back_color: Color::rgb(64, 64, 64),
            timer: TimerData::default(),
            is_transparent: false,
            image: Default::default(),
            sound: Default::default(),
            name: Default::default(),
            width: 30.0,
            height: 40.0,
            reel_count: 5.0,
            reel_spacing: 4.0,
            motor_steps: 2.0,
            digit_range: 9.0,
            update_interval: 50,
            use_image_grid: false,
            is_visible: true,
            images_per_grid_row: 1,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl Serialize for Reel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ReelJson::from_reel(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Reel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reel_json = ReelJson::deserialize(deserializer)?;
        Ok(reel_json.to_reel())
    }
}

impl BiffRead for Reel {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut reel = Reel::default();

        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VER1" => {
                    reel.ver1 = Vertex2D::biff_read(reader)?;
                }
                "VER2" => {
                    reel.ver2 = Vertex2D::biff_read(reader)?;
                }
                "CLRB" => {
                    reel.back_color = Color::biff_read(reader)?;
                }
                "TRNS" => {
                    reel.is_transparent = reader.get_bool()?;
                }
                "IMAG" => {
                    reel.image = reader.get_string()?;
                }
                "SOUN" => {
                    reel.sound = reader.get_string()?;
                }
                "NAME" => {
                    reel.name = reader.get_wide_string()?;
                }
                "WDTH" => {
                    reel.width = reader.get_f32()?;
                }
                "HIGH" => {
                    reel.height = reader.get_f32()?;
                }
                "RCNT" => {
                    reel.reel_count = reader.get_f32()?;
                }
                "RSPC" => {
                    reel.reel_spacing = reader.get_f32()?;
                }
                "MSTP" => {
                    reel.motor_steps = reader.get_f32()?;
                }
                "RANG" => {
                    reel.digit_range = reader.get_f32()?;
                }
                "UPTM" => {
                    reel.update_interval = reader.get_u32()?;
                }
                "UGRD" => {
                    reel.use_image_grid = reader.get_bool()?;
                }
                "VISI" => {
                    reel.is_visible = reader.get_bool()?;
                }
                "GIPR" => {
                    reel.images_per_grid_row = reader.get_u32()?;
                }
                _ => {
                    if !reel.timer.biff_read_tag(tag_str, reader)?
                        && !reel.read_shared_attribute(tag_str, reader)?
                    {
                        warn!(
                            "Unknown tag {} for {}",
                            tag_str,
                            std::any::type_name::<Self>()
                        );
                        reader.skip_tag()?;
                    }
                }
            }
        }
        Ok(reel)
    }
}

impl BiffWrite for Reel {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VER1", &self.ver1);
        writer.write_tagged("VER2", &self.ver2);
        writer.write_tagged_with("CLRB", &self.back_color, Color::biff_write);
        self.timer.biff_write(writer);
        writer.write_tagged_bool("TRNS", self.is_transparent);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_string("SOUN", &self.sound);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_f32("WDTH", self.width);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_f32("RCNT", self.reel_count);
        writer.write_tagged_f32("RSPC", self.reel_spacing);
        writer.write_tagged_f32("MSTP", self.motor_steps);
        writer.write_tagged_f32("RANG", self.digit_range);
        writer.write_tagged_u32("UPTM", self.update_interval);
        writer.write_tagged_bool("UGRD", self.use_image_grid);
        writer.write_tagged_bool("VISI", self.is_visible);
        writer.write_tagged_u32("GIPR", self.images_per_grid_row);

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
    use rand::RngExt;

    #[test]
    fn test_write_read() {
        let mut rng = rand::rng();
        // values not equal to the defaults
        let reel = Reel {
            ver1: Vertex2D::new(rng.random(), rng.random()),
            ver2: Vertex2D::new(rng.random(), rng.random()),
            back_color: Faker.fake(),
            timer: TimerData {
                is_enabled: rng.random(),
                interval: rng.random(),
            },
            is_transparent: rng.random(),
            image: "test image".to_string(),
            sound: "test sound".to_string(),
            name: "test name".to_string(),
            width: rng.random(),
            height: rng.random(),
            reel_count: rng.random(),
            reel_spacing: rng.random(),
            motor_steps: rng.random(),
            digit_range: rng.random(),
            update_interval: rng.random(),
            use_image_grid: rng.random(),
            is_visible: rng.random(),
            images_per_grid_row: rng.random(),
            is_locked: rng.random(),
            editor_layer: Some(rng.random()),
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: rng.random_option(),
            part_group_name: Some("test part group name".to_string()),
        };
        let mut writer = BiffWriter::new();
        Reel::biff_write(&reel, &mut writer);
        let reel_read = Reel::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(reel, reel_read);
    }
}
