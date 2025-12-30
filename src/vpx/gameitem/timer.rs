use super::vertex2d::Vertex2D;
use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::{HasSharedAttributes, TimerDataRoot, WriteSharedAttributes};
use fake::Dummy;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Dummy)]
pub struct Timer {
    pub center: Vertex2D,
    is_timer_enabled: bool,
    timer_interval: i32,
    pub name: String,
    pub backglass: bool,

    // these are shared between all items
    pub is_locked: bool,
    pub editor_layer: Option<u32>,
    pub editor_layer_name: Option<String>,
    // default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct TimerJson {
    center: Vertex2D,
    is_timer_enabled: bool,
    timer_interval: i32,
    name: String,
    backglass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl TimerJson {
    pub fn from_timer(timer: &Timer) -> Self {
        Self {
            center: timer.center,
            is_timer_enabled: timer.is_timer_enabled,
            timer_interval: timer.timer_interval,
            name: timer.name.clone(),
            backglass: timer.backglass,
            part_group_name: timer.part_group_name.clone(),
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
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
            part_group_name: self.part_group_name.clone(),
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
            editor_layer: None,
            editor_layer_name: None,
            editor_layer_visibility: None,
            part_group_name: None,
        }
    }
}

impl HasSharedAttributes for Timer {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_locked(&self) -> bool {
        self.is_locked
    }
    fn editor_layer(&self) -> Option<u32> {
        self.editor_layer
    }
    fn editor_layer_name(&self) -> Option<&str> {
        self.editor_layer_name.as_deref()
    }
    fn editor_layer_visibility(&self) -> Option<bool> {
        self.editor_layer_visibility
    }
    fn part_group_name(&self) -> Option<&str> {
        self.part_group_name.as_deref()
    }

    fn set_is_locked(&mut self, locked: bool) {
        self.is_locked = locked;
    }

    fn set_editor_layer(&mut self, layer: Option<u32>) {
        self.editor_layer = layer;
    }

    fn set_editor_layer_name(&mut self, name: Option<String>) {
        self.editor_layer_name = name;
    }

    fn set_editor_layer_visibility(&mut self, visibility: Option<bool>) {
        self.editor_layer_visibility = visibility;
    }

    fn set_part_group_name(&mut self, name: Option<String>) {
        self.part_group_name = name;
    }
}

impl TimerDataRoot for Timer {
    fn is_timer_enabled(&self) -> bool {
        self.is_timer_enabled
    }
    fn timer_interval(&self) -> i32 {
        self.timer_interval
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
                _ => {
                    if !timer.read_shared_attribute(tag_str, reader) {
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

        self.write_shared_attributes(writer);

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
            editor_layer: Some(5),
            editor_layer_name: Some("test layer".to_string()),
            editor_layer_visibility: Some(false),
            part_group_name: Some("test group".to_string()),
        };
        let mut writer = BiffWriter::new();
        Timer::biff_write(&timer, &mut writer);
        let timer_read = Timer::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(timer, timer_read);
    }
}
