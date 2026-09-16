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

/// The attribute bits the OLE `StdFont` format defines.
const ITALIC_BIT: u8 = 0x02;
const UNDERLINE_BIT: u8 = 0x04;
const STRIKETHROUGH_BIT: u8 = 0x08;
const KNOWN_ATTRIBUTE_BITS: u8 = ITALIC_BIT | UNDERLINE_BIT | STRIKETHROUGH_BIT;

/// One style flag of a [`Font`], kept as a set since several combine.
///
/// The `StdFont` stream stores the styles in a single attribute byte, read
/// by vpinball's `FontDesc` (`src/utils/fileio.h`) as the OLE format
/// defines it: `0x02` italic, `0x04` underline and `0x08` strikethrough.
/// Bold is not a flag: vpinball treats a [`Font`] weight above 550 as
/// bold. Bits the format does not define are kept on the [`Font`] as they
/// are, so any byte round-trips.
///
/// Versions of this crate before 0.34 mapped the variants one bit too
/// high, so their extracted tables name the italic bit `Bold`; see
/// [`FontStyle::Bold`] for how such JSON is read.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Hash, Eq)]
pub enum FontStyle {
    /// No style. Contributes no bit and is never produced when reading a
    /// file (an empty set means normal); accepted for compatibility with
    /// JSON written by older versions of this crate.
    Normal,
    /// Legacy name of the italic bit (`0x02`), from versions of this crate
    /// before 0.34 that mapped the styles one bit too high. Bold is not a
    /// style flag in the `StdFont` format; use the [`Font`] weight (700 is
    /// bold, vpinball treats anything above 550 as bold). Reading a file
    /// never produces this variant; writing it sets the italic bit, and
    /// JSON that contains it is read as [`FontStyle::Italic`] with a
    /// warning.
    #[deprecated(
        since = "0.34.0",
        note = "this named the italic bit; use Italic, and the font weight for bold"
    )]
    Bold,
    /// Italic, bit `0x02` of the attribute byte.
    Italic,
    /// Underline, bit `0x04` of the attribute byte.
    Underline,
    /// Strikethrough, bit `0x08` of the attribute byte.
    Strikethrough,
}
/// Fakes only the styles a file can produce; the deprecated `Bold` would
/// not survive a JSON round trip.
#[cfg(test)]
impl fake::Dummy<fake::Faker> for FontStyle {
    fn dummy_with_rng<R: rand::RngExt + ?Sized>(_: &fake::Faker, rng: &mut R) -> Self {
        match rng.random_range(0..4) {
            0 => FontStyle::Normal,
            1 => FontStyle::Italic,
            2 => FontStyle::Underline,
            _ => FontStyle::Strikethrough,
        }
    }
}

impl FontStyle {
    #[allow(deprecated)]
    fn to_bitflag(&self) -> u8 {
        match self {
            FontStyle::Normal => 0,
            FontStyle::Bold => ITALIC_BIT,
            FontStyle::Italic => ITALIC_BIT,
            FontStyle::Underline => UNDERLINE_BIT,
            FontStyle::Strikethrough => STRIKETHROUGH_BIT,
        }
    }
    /// The set of styles whose bits are set in an attribute byte as read
    /// from the `StdFont` stream. Only the italic, underline and
    /// strikethrough bits are looked at; other bits are ignored here and
    /// kept on the [`Font`] when it is read from a file.
    pub fn flags_to_styles(style: u8) -> HashSet<Self> {
        let mut styles = HashSet::with_capacity(3);
        if style & ITALIC_BIT != 0 {
            styles.insert(Self::Italic);
        }
        if style & UNDERLINE_BIT != 0 {
            styles.insert(Self::Underline);
        }
        if style & STRIKETHROUGH_BIT != 0 {
            styles.insert(Self::Strikethrough);
        }
        styles
    }

    /// The attribute byte to write to the `StdFont` stream for a set of
    /// styles: the bitwise or of the bit of each style, `0` for the empty
    /// set or for `Normal` alone.
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

/// The font of a text box or decal.
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
    /// The bits of the attribute byte the `StdFont` format does not define
    /// (`0x01` and `0x10` to `0x80`), kept so the byte round-trips. Always
    /// `0` in files written by vpinball.
    other_attributes: u8,
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
    /// Bits of the attribute byte outside the known style bits, only
    /// present when a file carries some.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    other_attributes: Option<u8>,
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
        let other_attributes = match font.other_attributes {
            0 => None,
            other => Some(other),
        };
        Self {
            version,
            charset,
            style: font.style.clone(),
            other_attributes,
            weight: font.weight,
            size: font.size,
            name: font.name.clone(),
        }
    }
    #[allow(deprecated)]
    pub fn to_font(&self) -> Font {
        let mut style = self.style.clone();
        // JSON written by this crate before 0.34 named the italic bit Bold
        if style.remove(&FontStyle::Bold) {
            log::warn!(
                "Font \"{}\": style Bold from an older extracted table is the italic bit, reading it as Italic; bold is the weight",
                self.name
            );
            style.insert(FontStyle::Italic);
        }
        Font {
            version: self.version.unwrap_or(EXPECTED_FONTDESC_VERSION),
            charset: self.charset.unwrap_or(CHARSET_ANSI),
            style,
            other_attributes: self.other_attributes.unwrap_or(0),
            weight: self.weight,
            size: self.size,
            name: self.name.clone(),
        }
    }
}

impl Font {
    /// The face name, which is how a font is looked up: a system font or
    /// one embedded in the table
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A font with the standard stream version `1`.
    ///
    /// `charset` is one of the `CHARSET_*` constants of this module,
    /// `weight` the Windows font weight where `400` is normal and `700`
    /// bold (vpinball treats anything above 550 as bold), `size` the low
    /// 32 bits of the OLE `cySize` currency value (the point size times
    /// 10000) and `name` the face name.
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
            other_attributes: 0,
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
            other_attributes: 0,
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
        let attributes = reader.get_u8_no_remaining_update()?;
        let weight = reader.get_u16_no_remaining_update()?;
        let size = reader.get_u32_no_remaining_update()?;
        let name_len = reader.get_u8_no_remaining_update()?;
        let name = reader.get_str_no_remaining_update(name_len as usize)?;
        Ok(Font {
            version,
            charset,
            style: FontStyle::flags_to_styles(attributes),
            other_attributes: attributes & !KNOWN_ATTRIBUTE_BITS,
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
        writer.write_u8(FontStyle::styles_to_flags(&self.style) | self.other_attributes);
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

    /// The attribute byte at offset 3 of a written font.
    fn written_attributes(font: &Font) -> u8 {
        let mut writer = BiffWriter::new();
        Font::biff_write(font, &mut writer);
        writer.get_data()[3]
    }

    fn read_with_attributes(attributes: u8) -> Font {
        let mut writer = BiffWriter::new();
        Font::biff_write(&Font::default(), &mut writer);
        let mut data = writer.get_data().to_vec();
        data[3] = attributes;
        let mut reader = BiffReader::new(&data);
        Font::biff_read(&mut reader).unwrap()
    }

    #[test]
    fn write_read_font() {
        let font: Font = Font {
            version: EXPECTED_FONTDESC_VERSION,
            charset: CHARSET_SYMBOL,
            style: HashSet::from([
                FontStyle::Italic,
                FontStyle::Underline,
                FontStyle::Strikethrough,
            ]),
            other_attributes: 0,
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
    fn styles_use_the_ole_attribute_bits() {
        // the bits vpinball's FontDesc reads: 0x02 italic, 0x04 underline,
        // 0x08 strikethrough; bold is the weight, not a bit
        assert_eq!(
            read_with_attributes(0x02).style,
            HashSet::from([FontStyle::Italic])
        );
        assert_eq!(
            read_with_attributes(0x04).style,
            HashSet::from([FontStyle::Underline])
        );
        assert_eq!(
            read_with_attributes(0x08).style,
            HashSet::from([FontStyle::Strikethrough])
        );
        assert_eq!(read_with_attributes(0x00).style, HashSet::new());
        let italic = Font::new(
            CHARSET_ANSI,
            HashSet::from([FontStyle::Italic]),
            700,
            120000,
            "Arial".to_string(),
        );
        assert_eq!(written_attributes(&italic), 0x02);
        let normal = Font::new(
            CHARSET_ANSI,
            HashSet::from([FontStyle::Normal]),
            400,
            120000,
            "Arial".to_string(),
        );
        assert_eq!(written_attributes(&normal), 0x00);
    }

    #[test]
    fn unknown_attribute_bits_round_trip() {
        let font = read_with_attributes(0xB1);
        assert_eq!(font.style, HashSet::new());
        assert_eq!(font.other_attributes, 0xB1);
        assert_eq!(written_attributes(&font), 0xB1);
        let json = serde_json::to_value(FontJson::from_font(&font)).unwrap();
        assert_eq!(json.get("other_attributes").unwrap(), 0xB1);
        let back: FontJson = serde_json::from_value(json).unwrap();
        assert_eq!(back.to_font(), font);
        // the common case keeps the json as it was
        let json = serde_json::to_value(FontJson::from_font(&Font::default())).unwrap();
        assert!(json.get("other_attributes").is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn legacy_json_bold_is_the_italic_bit() {
        // json written by this crate before 0.34 named the 0x02 bit Bold
        let json = serde_json::json!({
            "style": ["Bold"],
            "weight": 700,
            "size": 120000,
            "name": "Arial"
        });
        let back: FontJson = serde_json::from_value(json).unwrap();
        let font = back.to_font();
        assert_eq!(font.style, HashSet::from([FontStyle::Italic]));
        assert_eq!(written_attributes(&font), 0x02);
        // and writing the deprecated variant directly sets the same bit
        let bold = Font::new(
            CHARSET_ANSI,
            HashSet::from([FontStyle::Bold]),
            700,
            120000,
            "Arial".to_string(),
        );
        assert_eq!(written_attributes(&bold), 0x02);
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
