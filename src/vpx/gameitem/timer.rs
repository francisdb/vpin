use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Dummy)]
pub struct Timer {
    pub center: Vertex2D,
    pub is_timer_enabled: bool,
    pub timer_interval: i32,
    pub name: String,
    pub backglass: bool,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: u32,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct TimerJson {
    center: Vertex2D,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    backglass: bool,
}

impl TimerJson {
    pub fn from_timer(timer: &Timer) -> Self {
        Self {
            center: timer.center,
            is_timer_enabled: timer.is_timer_enabled,
            timer_interval: timer.timer_interval,
            name: timer.name.clone(),
            backglass: timer.backglass,
        }
    }
    pub fn to_timer(&self) -> Timer {
        Timer {
            center: self.center,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            name: self.name.clone(),
            backglass: self.backglass,
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

impl Serialize for Timer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TimerJson::from_timer(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = TimerJson::deserialize(deserializer)?;
        Ok(json.to_timer())
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            is_timer_enabled: false,
            timer_interval: 1000,
            name: "Timer".to_string(),
            backglass: false,
            is_locked: false,
            editor_layer: 0,
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

impl BiffRead for Timer {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut timer = Timer::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    timer.center = Vertex2D::biff_read(reader);
                }
                "TMON" => {
                    timer.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    timer.timer_interval = reader.get_i32();
                }
                "NAME" => {
                    timer.name = reader.get_wide_string();
                }
                "BGLS" => {
                    timer.backglass = reader.get_bool();
                }
                // shared
                "LOCK" => {
                    timer.is_locked = reader.get_bool();
                }
                "LAYR" => {
                    timer.editor_layer = reader.get_u32();
                }
                "LANR" => {
                    timer.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    timer.editor_layer_visibility = Some(reader.get_bool());
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
        timer
    }
}

impl BiffWrite for Timer {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_i32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_bool("BGLS", self.backglass);
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

    #[test]
    fn test_write_read() {
        // values not equal to the defaults
        let timer = Timer {
            center: Vertex2D::new(1.0, 2.0),
            is_timer_enabled: true,
            timer_interval: 3,
            name: "test timer".to_string(),
            backglass: false,
            is_locked: true,
            editor_layer: 5,
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(false),
        };
        let mut writer = BiffWriter::new();
        Timer::biff_write(&timer, &mut writer);
        let timer_read = Timer::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(timer, timer_read);
    }
}
