use crate::vpx::biff::{BiffError, BiffRead, BiffReader, BiffWrite, BiffWriter};
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
///
/// The serialization format is not vpinball's own: it is Microsoft's OLE
/// `StdFont` persistence format, inherited because old Windows vpinball
/// saved fonts through `OleSaveToStream` on a COM font object.
#[derive(PartialEq, Debug)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Font {
    /// Version byte of the OLE `StdFont` stream. Microsoft defined it as
    /// always 1 and never revised the format, so 1 is the only value in
    /// the wild. vpinball keeps whatever value it reads and writes it
    /// back, so we do the same to round-trip tables unchanged.
    version: u8,
    /// from <https://learn.microsoft.com/en-us/windows/win32/lwef/fontcharset-property>
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
    version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    charset: Option<u16>,
    style: HashSet<FontStyle>,
    weight: u16,
    size: u32,
    name: String,
}
impl FontJson {
    pub fn from_font(font: &Font) -> Self {
        let version = match font.version {
            EXPECTED_FONTDESC_VERSION => None,
            _ => Some(font.version),
        };
        let charset = match font.charset {
            CHARSET_ANSI => None,
            _ => Some(font.charset),
        };
        Self {
            version,
            charset,
            style: font.style.clone(),
            weight: font.weight,
            size: font.size,
            name: font.name.clone(),
        }
    }
    pub fn to_font(&self) -> Font {
        Font {
            version: self.version.unwrap_or(EXPECTED_FONTDESC_VERSION),
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
            version: EXPECTED_FONTDESC_VERSION,
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
            version: EXPECTED_FONTDESC_VERSION,
            charset: CHARSET_ANSI,
            style: HashSet::new(),
            weight: 0,
            size: 400,
            name: "Arial".to_string(),
        }
    }
}

impl BiffRead for Font {
    fn biff_read(reader: &mut BiffReader<'_>) -> Result<Self, BiffError> {
        let version = reader.get_u8_no_remaining_update()?;
        if version != EXPECTED_FONTDESC_VERSION {
            // vpinball reads this byte and continues regardless of its value,
            // keeping it for the next save
            log::warn!(
                "Unexpected font descriptor version {version}, expected {EXPECTED_FONTDESC_VERSION}"
            );
        }
        let charset = reader.get_u16_no_remaining_update()?;
        let style = reader.get_u8_no_remaining_update()?;
        let weight = reader.get_u16_no_remaining_update()?;
        let size = reader.get_u32_no_remaining_update()?;
        let name_len = reader.get_u8_no_remaining_update()?;
        let name = reader.get_str_no_remaining_update(name_len as usize)?;
        Ok(Font {
            version,
            charset,
            style: FontStyle::flags_to_styles(style),
            weight,
            size,
            name,
        })
    }
}

impl BiffWrite for Font {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_u8(self.version);
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
            version: EXPECTED_FONTDESC_VERSION,
            charset: CHARSET_SYMBOL,
            style: HashSet::from([FontStyle::Bold, FontStyle::Italic, FontStyle::Underline]),
            weight: 100,
            size: 12,
            name: "Wingdings 3".to_string(),
        };
        let mut writer = BiffWriter::new();
        Font::biff_write(&font, &mut writer);
        let mut reader = BiffReader::new(writer.get_data());
        let font2 = Font::biff_read(&mut reader).unwrap();
        assert_eq!(font, font2);
    }

    #[test]
    fn read_font_with_unexpected_version() {
        // vpinball reads the version byte and continues regardless of its
        // value, keeping it for the next save, so an unexpected version
        // should not fail the parse and should round-trip unchanged
        let font = Font::default();
        let mut writer = BiffWriter::new();
        Font::biff_write(&font, &mut writer);
        let mut data = writer.get_data().to_vec();
        data[0] = 76;
        let mut reader = BiffReader::new(&data);
        let font2 = Font::biff_read(&mut reader).unwrap();
        assert_eq!(font2.version, 76);
        let mut writer2 = BiffWriter::new();
        Font::biff_write(&font2, &mut writer2);
        assert_eq!(writer2.get_data(), data.as_slice());
    }

    #[test]
    fn json_omits_the_standard_version() {
        let json = serde_json::to_value(FontJson::from_font(&Font::default())).unwrap();
        assert!(json.get("version").is_none());
        // json from before the field existed reads as the standard version
        let back: FontJson = serde_json::from_value(json).unwrap();
        assert_eq!(back.to_font().version, EXPECTED_FONTDESC_VERSION);
        let font = Font {
            version: 76,
            ..Font::default()
        };
        let json = serde_json::to_value(FontJson::from_font(&font)).unwrap();
        assert_eq!(json.get("version").unwrap(), 76);
        let back: FontJson = serde_json::from_value(json).unwrap();
        assert_eq!(back.to_font(), font);
    }
}
