use crate::vpx::color::ColorJson;
use crate::vpx::{
    biff::{self, BiffRead, BiffReader, BiffWrite},
    color::Color,
};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Dummy)]
pub struct Reel {
    ver1: Vertex2D,    // position on map (top right corner)
    ver2: Vertex2D,    // position on map (top right corner)
    back_color: Color, // colour of the background
    is_timer_enabled: bool,
    timer_interval: u32,
    is_transparent: bool, // is the background transparent
    image: String,
    sound: String, // sound to play for each turn of a digit
    pub name: String,
    width: f32,        // size of each reel
    height: f32,       // size of each reel
    reel_count: u32,   // number of individual reel in the set
    reel_spacing: f32, // spacing between each reel and the boarders
    motor_steps: u32,  // steps (or frames) to move each reel each frame
    digit_range: u32,  // max number of digits per reel (usually 9)
    update_interval: u32,
    use_image_grid: bool,
    is_visible: bool,
    images_per_grid_row: u32,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    // TODO we found at least one table where these two were missing
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReelJson {
    ver1: Vertex2D,
    ver2: Vertex2D,
    back_color: ColorJson,
    is_timer_enabled: bool,
    timer_interval: u32,
    is_transparent: bool,
    image: String,
    sound: String,
    name: String,
    width: f32,
    height: f32,
    reel_count: u32,
    reel_spacing: f32,
    motor_steps: u32,
    digit_range: u32,
    update_interval: u32,
    use_image_grid: bool,
    is_visible: bool,
    images_per_grid_row: u32,
}

impl ReelJson {
    pub fn from_reel(reel: &Reel) -> Self {
        Self {
            ver1: reel.ver1,
            ver2: reel.ver2,
            back_color: ColorJson::from_color(&reel.back_color),
            is_timer_enabled: reel.is_timer_enabled,
            timer_interval: reel.timer_interval,
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
        }
    }
    pub fn to_reel(&self) -> Reel {
        Reel {
            ver1: self.ver1,
            ver2: self.ver2,
            back_color: self.back_color.to_color(),
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
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
            editor_layer: 0,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
        }
    }
}

impl Default for Reel {
    fn default() -> Self {
        Self {
            ver1: Vertex2D::default(),
            ver2: Vertex2D::default(),
            back_color: Color::new_bgr(0x404040f),
            is_timer_enabled: false,
            timer_interval: Default::default(),
            is_transparent: false,
            image: Default::default(),
            sound: Default::default(),
            name: Default::default(),
            width: 30.0,
            height: 40.0,
            reel_count: 5,
            reel_spacing: 4.0,
            motor_steps: 2,
            digit_range: 9,
            update_interval: 50,
            use_image_grid: false,
            is_visible: true,
            images_per_grid_row: 1,
            is_locked: false,
            editor_layer: Default::default(),
            editor_layer_name: None,
            editor_layer_visibility: None,
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
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut reel = Reel::default();

        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VER1" => {
                    reel.ver1 = Vertex2D::biff_read(reader);
                }
                "VER2" => {
                    reel.ver2 = Vertex2D::biff_read(reader);
                }
                "CLRB" => {
                    reel.back_color = Color::biff_read_bgr(reader);
                }
                "TMON" => {
                    reel.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    reel.timer_interval = reader.get_u32();
                }
                "TRNS" => {
                    reel.is_transparent = reader.get_bool();
                }
                "IMAG" => {
                    reel.image = reader.get_string();
                }
                "SOUN" => {
                    reel.sound = reader.get_string();
                }
                "NAME" => {
                    reel.name = reader.get_wide_string();
                }
                "WDTH" => {
                    reel.width = reader.get_f32();
                }
                "HIGH" => {
                    reel.height = reader.get_f32();
                }
                "RCNT" => {
                    reel.reel_count = reader.get_u32();
                }
                "RSPC" => {
                    reel.reel_spacing = reader.get_f32();
                }
                "MSTP" => {
                    reel.motor_steps = reader.get_u32();
                }
                "RANG" => {
                    reel.digit_range = reader.get_u32();
                }
                "UPTM" => {
                    reel.update_interval = reader.get_u32();
                }
                "UGRD" => {
                    reel.use_image_grid = reader.get_bool();
                }
                "VISI" => {
                    reel.is_visible = reader.get_bool();
                }
                "GIPR" => {
                    reel.images_per_grid_row = reader.get_u32();
                }

                // shared
                "LOCK" => {
                    reel.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    reel.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    reel.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    reel.editor_layer_visibility = Some(reader.get_bool());
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
        reel
    }
}

impl BiffWrite for Reel {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VER1", &self.ver1);
        writer.write_tagged("VER2", &self.ver2);
        writer.write_tagged_with("CLRB", &self.back_color, Color::biff_write_bgr);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        writer.write_tagged_bool("TRNS", self.is_transparent);
        writer.write_tagged_string("IMAG", &self.image);
        writer.write_tagged_string("SOUN", &self.sound);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_f32("WDTH", self.width);
        writer.write_tagged_f32("HIGH", self.height);
        writer.write_tagged_u32("RCNT", self.reel_count);
        writer.write_tagged_f32("RSPC", self.reel_spacing);
        writer.write_tagged_u32("MSTP", self.motor_steps);
        writer.write_tagged_u32("RANG", self.digit_range);
        writer.write_tagged_u32("UPTM", self.update_interval);
        writer.write_tagged_bool("UGRD", self.use_image_grid);
        writer.write_tagged_bool("VISI", self.is_visible);
        writer.write_tagged_u32("GIPR", self.images_per_grid_row);
        // shared
        writer.write_tagged_bool("LOCK", self.is_locked);
        writer.write_tagged_u32("LAYR", self.editor_layer);

        if let Some(name) = self.editor_layer_name.as_ref() {
            writer.write_tagged_string("LANR", name);
        }
        if let Some(visibility) = self.editor_layer_visibility.as_ref() {
            writer.write_tagged_bool("LVIS", *visibility);
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
        // values not equal to the defaults
        let reel = Reel {
            ver1: Vertex2D::new(rng.gen(), rng.gen()),
            ver2: Vertex2D::new(rng.gen(), rng.gen()),
            back_color: Color::new_bgr(rng.gen()),
            is_timer_enabled: rng.gen(),
            timer_interval: rng.gen(),
            is_transparent: rng.gen(),
            image: "test image".to_string(),
            sound: "test sound".to_string(),
            name: "test name".to_string(),
            width: rng.gen(),
            height: rng.gen(),
            reel_count: rng.gen(),
            reel_spacing: rng.gen(),
            motor_steps: rng.gen(),
            digit_range: rng.gen(),
            update_interval: rng.gen(),
            use_image_grid: rng.gen(),
            is_visible: rng.gen(),
            images_per_grid_row: rng.gen(),
            is_locked: rng.gen(),
            editor_layer: rng.gen(),
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: rng.gen(),
        };
        let mut writer = BiffWriter::new();
        Reel::biff_write(&reel, &mut writer);
        let reel_read = Reel::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(reel, reel_read);
    }
}
