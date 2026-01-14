use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// from https://github.com/wine-mirror/wine/blob/f38a32e64c00600a5252fe0b9ca1ca42208bd6fe/dlls/oleaut32/olefont.c#L1546-L1566
/************************************************************************
 * OLEFontImpl_Load (IPersistStream)
 *
 * See Windows documentation for more details on IPersistStream methods.
 *
 * This is the format of the standard font serialization as far as I
 * know
 *
 * Offset   Type   Value           Comment
 * 0x0000   Byte   Unknown         Probably a version number, contains 0x01
 * 0x0001   Short  Charset         Charset value from the FONTDESC structure
 * 0x0003   Byte   Attributes      Flags defined as follows:
 *                                     00000010 - Italic
 *                                     00000100 - Underline
 *                                     00001000 - Strikethrough
 * 0x0004   Short  Weight          Weight value from FONTDESC structure
 * 0x0006   DWORD  size            "Low" portion of the cySize member of the FONTDESC
 *                                 structure/
 * 0x000A   Byte   name length     Length of the font name string (no null character)
 * 0x000B   String name            Name of the font (ASCII, no nul character)
 */

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
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Hash, Eq)]
#[cfg_attr(test, derive(fake::Dummy))]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
    Underline,
    Strikethrough,
}
impl FontStyle {
    fn to_bitflag(&self) -> u8 {
        match self {
            FontStyle::Normal => 1 << 0,
            FontStyle::Bold => 1 << 1,
            FontStyle::Italic => 1 << 2,
            FontStyle::Underline => 1 << 3,
            FontStyle::Strikethrough => 1 << 4,
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
            bitflags |= flag.to_bitflag();
        }
        bitflags
    }
}

const EXPECTED_FONTDESC_VERSION: u8 = 0x01;

/// Standard Windows characters (ANSI).
pub const CHARSET_ANSI: u16 = 0;

/// Default character set.
pub const CHARSET_DEFAULT: u16 = 1;

/// The symbol character set.
pub const CHARSET_SYMBOL: u16 = 2;

/// Double-byte character set (DBCS) unique to the Japanese version of Windows.
pub const CHARSET_JAPANESE: u16 = 128;

/// Double-byte character set (DBCS) unique to the Korean version of Windows.
pub const CHARSET_KOREAN: u16 = 129;

/// Double-byte character set (DBCS) unique to the Simplified Chinese version of Windows.
pub const CHARSET_SIMPLIFIED_CHINESE: u16 = 134;

/// Double-byte character set (DBCS) unique to the Traditional Chinese version of Windows.
pub const CHARSET_TRADITIONAL_CHINESE: u16 = 136;

/// Extended characters normally displayed by Microsoft MS-DOS applications.
pub const CHARSET_EXTENDED: u16 = 255;

/// This is a font reference some primitives use.
/// In vpinball represented as serialized win32 FONTDESC struct
#[derive(PartialEq, Debug)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Font {
    /// from https://learn.microsoft.com/en-us/windows/win32/lwef/fontcharset-property
    /// An integer value that specifies the character set used by the font. The following are some
    /// common settings for value:
    /// 0 Standard Windows characters (ANSI).
    /// 1 Default character set.
    /// 2 The symbol character set.
    /// 128 Double-byte character set (DBCS) unique to the Japanese version of Windows.
    /// 129 Double-byte character set (DBCS) unique to the Korean version of Windows.
    /// 134 Double-byte character set (DBCS) unique to the Simplified Chinese version of Windows.
    /// 136 Double-byte character set (DBCS) unique to the Traditional Chinese version of Windows.
    /// 255 Extended characters normally displayed by Microsoft MS-DOS applications.
    /// For other character set values, consult the Platform SDK documentation.
    charset: u16,
    style: HashSet<FontStyle>,
    weight: u16,
    size: u32,
    name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    charset: Option<u16>,
    style: HashSet<FontStyle>,
    weight: u16,
    size: u32,
    name: String,
}
impl FontJson {
    pub fn from_font(font: &Font) -> Self {
        let charset = match font.charset {
            CHARSET_ANSI => None,
            _ => Some(font.charset),
        };
        Self {
            charset,
            style: font.style.clone(),
            weight: font.weight,
            size: font.size,
            name: font.name.clone(),
        }
    }
    pub fn to_font(&self) -> Font {
        Font {
            charset: self.charset.unwrap_or(CHARSET_ANSI),
            style: self.style.clone(),
            weight: self.weight,
            size: self.size,
            name: self.name.clone(),
        }
    }
}

impl Font {
    pub fn new(
        charset: u16,
        style: HashSet<FontStyle>,
        weight: u16,
        size: u32,
        name: String,
    ) -> Self {
        Self {
            charset,
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
            charset: CHARSET_ANSI,
            style: HashSet::new(),
            weight: 0,
            size: 400,
            name: "Arial".to_string(),
        }
    }
}

impl BiffRead for Font {
    fn biff_read(reader: &mut BiffReader<'_>) -> Font {
        let version = reader.get_u8_no_remaining_update();
        assert_eq!(version, EXPECTED_FONTDESC_VERSION, "Font version is not 1");
        let charset = reader.get_u16_no_remaining_update();
        let style = reader.get_u8_no_remaining_update();
        let weight = reader.get_u16_no_remaining_update();
        let size = reader.get_u32_no_remaining_update();
        let name_len = reader.get_u8_no_remaining_update();
        let name = reader.get_str_no_remaining_update(name_len as usize);
        Font {
            charset,
            style: FontStyle::flags_to_styles(style),
            weight,
            size,
            name,
        }
    }
}

impl BiffWrite for Font {
    fn biff_write(&self, writer: &mut BiffWriter) {
        // version?
        writer.write_u8(EXPECTED_FONTDESC_VERSION);
        writer.write_u16(self.charset);
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
            charset: CHARSET_SYMBOL,
            style: HashSet::from([FontStyle::Bold, FontStyle::Italic, FontStyle::Underline]),
            weight: 100,
            size: 12,
            name: "Wingdings 3".to_string(),
        };
        let mut writer = BiffWriter::new();
        Font::biff_write(&font, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let font2 = Font::biff_read(&mut reader);
        assert_eq!(font, font2);
    }
}
