use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use fake::Dummy;
use serde::{Deserialize, Serialize};

const TTF_STYLE_NORMAL: u8 = 0x00;
const TTF_STYLE_BOLD: u8 = 0x01;
const TTF_STYLE_ITALIC: u8 = 0x02;
const TTF_STYLE_UNDERLINE: u8 = 0x04;
const TTF_STYLE_STRIKETHROUGH: u8 = 0x08;

#[derive(PartialEq, Debug, Dummy)]
pub struct Font {
    /**
     * The style of the font.
     * This is a bitfield, so multiple styles can be combined.
     * The styles are:
     * - 0x00: normal
     * - 0x01: bold
     * - 0x02: italic
     * - 0x04: underline
     * - 0x08: strikethrough
     */
    style: u8,
    weight: u16,
    size: u32,
    name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontJson {
    style: u8,
    weight: u16,
    size: u32,
    name: String,
}
impl FontJson {
    pub fn from_font(font: &Font) -> Self {
        Self {
            style: font.style,
            weight: font.weight,
            size: font.size,
            name: font.name.clone(),
        }
    }
    pub fn to_font(&self) -> Font {
        Font {
            style: self.style,
            weight: self.weight,
            size: self.size,
            name: self.name.clone(),
        }
    }
}

impl Font {
    pub fn new(style: u8, weight: u16, size: u32, name: String) -> Self {
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
            style: 0,
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
            style,
            weight,
            size,
            name,
        }
    }
}

impl BiffWrite for Font {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_data(&[0x01, 0x00, 0x00]);
        writer.write_u8(self.style);
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
            style: 0,
            weight: 0,
            size: 0,
            name: "Arial Black".to_string(),
        };
        let mut writer = BiffWriter::new();
        Font::biff_write(&font, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let font2 = Font::biff_read(&mut reader);
        assert_eq!(font, font2);
    }
}
