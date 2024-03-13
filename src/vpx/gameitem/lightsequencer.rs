use crate::vpx::biff::{self, BiffRead, BiffReader, BiffWrite};
use fake::Dummy;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::vertex2d::Vertex2D;

#[derive(Debug, PartialEq, Dummy)]
pub struct LightSequencer {
    center: Vertex2D,
    collection: String,
    pos_x: f32,
    pos_y: f32,
    update_interval: u32,
    is_timer_enabled: bool,
    timer_interval: u32,
    pub name: String,
    backglass: bool,

    // these are shared between all items
    pub is_locked: Option<bool>,
    // LOCK (added in 10.7?)
    pub editor_layer: Option<u32>,
    // LAYR (added in 10.7?)
    pub editor_layer_name: Option<String>,
    // LANR (added in 10.7?) default "Layer_{editor_layer + 1}"
    pub editor_layer_visibility: Option<bool>, // LVIS (added in 10.7?)
}

#[derive(Serialize, Deserialize)]
struct LightSequencerJson {
    center: Vertex2D,
    collection: String,
    pos_x: f32,
    pos_y: f32,
    update_interval: u32,
    is_timer_enabled: bool,
    timer_interval: u32,
    name: String,
    backglass: bool,
}

impl LightSequencerJson {
    pub fn from_light_sequencer(light_sequencer: &LightSequencer) -> Self {
        Self {
            center: light_sequencer.center,
            collection: light_sequencer.collection.clone(),
            pos_x: light_sequencer.pos_x,
            pos_y: light_sequencer.pos_y,
            update_interval: light_sequencer.update_interval,
            is_timer_enabled: light_sequencer.is_timer_enabled,
            timer_interval: light_sequencer.timer_interval,
            name: light_sequencer.name.clone(),
            backglass: light_sequencer.backglass,
        }
    }
    pub fn to_light_sequencer(&self) -> LightSequencer {
        LightSequencer {
            center: self.center,
            collection: self.collection.clone(),
            pos_x: self.pos_x,
            pos_y: self.pos_y,
            update_interval: self.update_interval,
            is_timer_enabled: self.is_timer_enabled,
            timer_interval: self.timer_interval,
            name: self.name.clone(),
            backglass: self.backglass,
            // this is populated from a different file
            is_locked: None,
            // this is populated from a different file
            editor_layer: None,
            // this is populated from a different file
            editor_layer_name: None,
            // this is populated from a different file
            editor_layer_visibility: None,
        }
    }
}

impl Serialize for LightSequencer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LightSequencerJson::from_light_sequencer(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LightSequencer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json = LightSequencerJson::deserialize(deserializer)?;
        Ok(json.to_light_sequencer())
    }
}

impl Default for LightSequencer {
    fn default() -> Self {
        Self {
            center: Vertex2D::default(),
            collection: Default::default(),
            pos_x: Default::default(),
            pos_y: Default::default(),
            update_interval: 25,
            is_timer_enabled: false,
            timer_interval: 0,
            name: Default::default(),
            backglass: false,
            is_locked: None,
            editor_layer: None,
            editor_layer_name: None,
            editor_layer_visibility: None,
        }
    }
}

impl BiffRead for LightSequencer {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let mut light_sequencer = LightSequencer::default();
        loop {
            reader.next(biff::WARN);
            if reader.is_eof() {
                break;
            }
            let tag = reader.tag();
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    light_sequencer.center = Vertex2D::biff_read(reader);
                }
                "COLC" => {
                    light_sequencer.collection = reader.get_wide_string();
                }
                "CTRX" => {
                    light_sequencer.pos_x = reader.get_f32();
                }
                "CTRY" => {
                    light_sequencer.pos_y = reader.get_f32();
                }
                "UPTM" => {
                    light_sequencer.update_interval = reader.get_u32();
                }
                "TMON" => {
                    light_sequencer.is_timer_enabled = reader.get_bool();
                }
                "TMIN" => {
                    light_sequencer.timer_interval = reader.get_u32();
                }
                "NAME" => {
                    light_sequencer.name = reader.get_wide_string();
                }
                "BGLS" => {
                    light_sequencer.backglass = reader.get_bool();
                }

                // shared
                "LOCK" => {
                    light_sequencer.is_locked = Some(reader.get_bool());
                }
                "LAYR" => {
                    light_sequencer.editor_layer = Some(reader.get_u32());
                }
                "LANR" => {
                    light_sequencer.editor_layer_name = Some(reader.get_string());
                }
                "LVIS" => {
                    light_sequencer.editor_layer_visibility = Some(reader.get_bool());
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
        light_sequencer
    }
}

impl BiffWrite for LightSequencer {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        writer.write_tagged_wide_string("COLC", &self.collection);
        writer.write_tagged_f32("CTRX", self.pos_x);
        writer.write_tagged_f32("CTRY", self.pos_y);
        writer.write_tagged_u32("UPTM", self.update_interval);
        writer.write_tagged_bool("TMON", self.is_timer_enabled);
        writer.write_tagged_u32("TMIN", self.timer_interval);
        writer.write_tagged_wide_string("NAME", &self.name);
        writer.write_tagged_bool("BGLS", self.backglass);

        // shared
        if let Some(is_locked) = self.is_locked {
            writer.write_tagged_bool("LOCK", is_locked);
        }
        if let Some(editor_layer) = self.editor_layer {
            writer.write_tagged_u32("LAYR", editor_layer);
        }
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
        // values not equal to the defaults
        let spinner = LightSequencer {
            center: Vertex2D::new(rng.gen(), rng.gen()),
            collection: "test collection".to_string(),
            pos_x: rng.gen(),
            pos_y: rng.gen(),
            update_interval: rng.gen(),
            is_timer_enabled: rng.gen(),
            timer_interval: rng.gen(),
            name: "test name".to_string(),
            backglass: rng.gen(),
            is_locked: rng.gen(),
            editor_layer: rng.gen(),
            editor_layer_name: Some("test layer name".to_string()),
            editor_layer_visibility: rng.gen(),
        };
        let mut writer = BiffWriter::new();
        LightSequencer::biff_write(&spinner, &mut writer);
        let spinner_read = LightSequencer::biff_read(&mut BiffReader::new(writer.get_data()));
        assert_eq!(spinner, spinner_read);
    }
}
