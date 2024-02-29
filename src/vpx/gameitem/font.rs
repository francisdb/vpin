use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use fake::Dummy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/**
 * The style of the font.
 * This is serialized as a bitfield, so multiple styles can be combined.
 * The styles are:
 * - 0x00: normal
 * - 0x01: bold
 * - 0x02: italic
 * - 0x04: underline
 * - 0x08: strikethrough
 */
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Dummy, Hash, Eq)]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
    Underline,
    Strikethrough,
}
impl FontStyle {
    fn to_u8(flag: &FontStyle) -> u8 {
        match flag {
            &FontStyle::Normal => 1 << 0,
            &FontStyle::Bold => 1 << 1,
            &FontStyle::Italic => 1 << 2,
            &FontStyle::Underline => 1 << 3,
            &FontStyle::Strikethrough => 1 << 4,
        }
    }
    pub fn flags_to_styles(style: u8) -> HashSet<Self> {
        let mut styles = HashSet::with_capacity(5);
        if style & (1 << 0) != 0 {
            styles.insert(Self::Normal);
        }
        if style & (1 << 1) != 0 {
            styles.insert(Self::Bold);
        }
        if style & (1 << 2) != 0 {
            styles.insert(Self::Italic);
        }
        if style & (1 << 3) != 0 {
            styles.insert(Self::Underline);
        }
        if style & (1 << 4) != 0 {
            styles.insert(Self::Strikethrough);
        }
        styles
    }

    pub fn styles_to_flags(flags: &HashSet<Self>) -> u8 {
        let mut bitflags = 0u8;
        for flag in flags {
            bitflags |= Self::to_u8(flag);
        }
        return bitflags;
    }
}

#[derive(PartialEq, Debug, Dummy)]
pub struct Font {
    style: HashSet<FontStyle>,
    weight: u16,
    size: u32,
    name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontJson {
    style: HashSet<FontStyle>,
    weight: u16,
    size: u32,
    name: String,
}
impl FontJson {
    pub fn from_font(font: &Font) -> Self {
        Self {
            style: font.style.clone(),
            weight: font.weight,
            size: font.size,
            name: font.name.clone(),
        }
    }
    pub fn to_font(&self) -> Font {
        Font {
            style: self.style.clone(),
            weight: self.weight,
            size: self.size,
            name: self.name.clone(),
        }
    }
}

impl Font {
    pub fn new(style: HashSet<FontStyle>, weight: u16, size: u32, name: String) -> Self {
        Self {
            style,
            weight,
            size,
            name,
        }
    }
}

impl Default for Font {
    fn default() -> Self {
        // TODO get proper defaults
        Self {
            style: HashSet::new(),
            weight: 0,
            size: 400,
            name: "Arial".to_string(),
        }
    }
}

impl BiffRead for Font {
    fn biff_read(reader: &mut BiffReader<'_>) -> Font {
        let _header = reader.get_data(3); // always? 0x01, 0x0, 0x0

        let style = reader.get_u8_no_remaining_update();
        let weight = reader.get_u16_no_remaining_update();
        let size = reader.get_u32_no_remaining_update();
        let name_len = reader.get_u8_no_remaining_update();
        let name = reader.get_str_no_remaining_update(name_len as usize);
        Font {
            style: FontStyle::flags_to_styles(style),
            weight,
            size,
            name,
        }
    }
}

impl BiffWrite for Font {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_data(&[0x01, 0x00, 0x00]);
        writer.write_u8(FontStyle::styles_to_flags(&self.style));
        writer.write_u16(self.weight);
        writer.write_u32(self.size);
        writer.write_short_string(&self.name);
    }
}

#[cfg(test)]
mod test {

    use crate::vpx::biff::BiffWrite;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn write_read_font() {
        let font: Font = Font {
            style: HashSet::from([FontStyle::Bold, FontStyle::Italic, FontStyle::Underline]),
            weight: 100,
            size: 12,
            name: "Arial Black".to_string(),
        };
        let mut writer = BiffWriter::new();
        Font::biff_write(&font, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let font2 = Font::biff_read(&mut reader);
        assert_eq!(font, font2);
    }
}
