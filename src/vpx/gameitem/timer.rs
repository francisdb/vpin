use super::vertex2d::Vertex2D;
use crate::vpx::biff::{self, BiffError, BiffRead, BiffReader, BiffWrite};
use crate::vpx::gameitem::select::impl_shared_attributes;
use crate::vpx::gameitem::select::{TimerData, WriteSharedAttributes};
use log::warn;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Timer {
    /// Position of the timer's icon in the editor (x, y), in VP units.
    /// A timer has no visual or physical presence in the game; this only
    /// places its marker in the editor.
    ///
    /// BIFF tag: `VCEN`
    pub center: Vertex2D,
    /// Name of the timer, used to reference it from scripts.
    ///
    /// BIFF tag: `NAME`
    pub name: String,
    /// Whether the timer's editor icon is shown on the backglass/backdrop view
    /// rather than the playfield (VPinball `m_desktopBackdrop`). Editor
    /// placement only; does not affect timer behavior.
    ///
    /// BIFF tag: `BGLS`
    pub backglass: bool,

    /// Timer data for scripting (shared across all game items).
    /// See [`TimerData`] for details.
    pub timer: TimerData,

    // these are shared between all items
    /// Whether the item is locked in the editor to prevent accidental
    /// selection or movement. Editor-only; has no effect at runtime.
    ///
    /// BIFF tag: `LOCK`
    pub is_locked: bool,
    /// Legacy editor layer index this item belongs to. Superseded by part
    /// groups (`part_group_name`) in 10.8.1 and no longer written by newer
    /// versions. `None` when absent.
    ///
    /// BIFF tag: `LAYR`
    pub editor_layer: Option<u32>,
    /// Display name of the legacy editor layer. Defaults to
    /// `"Layer_{editor_layer + 1}"` when unset. `None` when absent.
    ///
    /// BIFF tag: `LANR`
    pub editor_layer_name: Option<String>,
    /// Whether the legacy editor layer is visible in the editor. `None` when
    /// absent. Editor-only; has no effect at runtime.
    ///
    /// BIFF tag: `LVIS`
    pub editor_layer_visibility: Option<bool>,
    /// Added in 10.8.1
    pub part_group_name: Option<String>,
}
impl_shared_attributes!(Timer);

#[derive(Serialize, Deserialize)]
struct TimerJson {
    center: Vertex2D,
    #[serde(flatten)]
    /// Timer state (enabled flag and interval in ms) that drives this
    /// item's script `_Timer` events. See [`TimerData`].
    pub timer: TimerData,
    name: String,
    backglass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_group_name: Option<String>,
}

impl TimerJson {
    pub fn from_timer(timer: &Timer) -> Self {
        Self {
            center: timer.center,
            timer: timer.timer.clone(),
            name: timer.name.clone(),
            backglass: timer.backglass,
            part_group_name: timer.part_group_name.clone(),
        }
    }
    pub fn to_timer(&self) -> Timer {
        Timer {
            center: self.center,
            timer: self.timer.clone(),
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
            timer: TimerData::default(),
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

impl BiffRead for Timer {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let mut timer = Timer::default();
        while let Some(tag) = reader.next(biff::WARN)? {
            let tag_str = tag.as_str();
            match tag_str {
                "VCEN" => {
                    timer.center = Vertex2D::biff_read(reader)?;
                }
                "NAME" => {
                    timer.name = reader.get_wide_string()?;
                }
                "BGLS" => {
                    timer.backglass = reader.get_bool()?;
                }
                _ => {
                    if !timer.timer.biff_read_tag(tag_str, reader)?
                        && !timer.read_shared_attribute(tag_str, reader)?
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
        Ok(timer)
    }
}

impl BiffWrite for Timer {
    fn biff_write(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged("VCEN", &self.center);
        self.timer.biff_write(writer);
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
            timer: TimerData {
                is_enabled: true,
                interval: 3,
            },
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
        let timer_read = Timer::biff_read(&mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(timer, timer_read);
    }
}
