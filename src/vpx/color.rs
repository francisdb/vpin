use crate::vpx::biff::BiffReader;
use serde::{Deserialize, Serialize};

use super::biff::BiffWriter;

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(test, derive(fake::Dummy))]
pub struct Color {
    /// Unused byte, should be 0 but when reading from vpx files it might contain random data.
    /// So used for BIFF reading and writing
    /// And since we want to round-trip the data, we need to store it in the json format as well.
    /// Seems to contain 255 or 128 in the wild.
    unused: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        unused: 0,
    };
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        unused: 0,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        unused: 0,
    };
}

/// Serialize as a string in the format "#RRGGBB".
impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.unused == 0 {
            let s = format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b);
            serializer.serialize_str(&s)
        } else {
            let s = format!(
                "{:02x}#{:02x}{:02x}{:02x}",
                self.unused, self.r, self.g, self.b
            );
            serializer.serialize_str(&s)
        }
    }
}

// Deserialize from a string in the format "#RRGGBB".
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Color, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.len() {
            7 => {
                if &s[0..1] != "#" {
                    return Err(serde::de::Error::custom(
                        "Invalid color format, expected #RRGGBB",
                    ));
                }
                let r = u8::from_str_radix(&s[1..3], 16).map_err(serde::de::Error::custom)?;
                let g = u8::from_str_radix(&s[3..5], 16).map_err(serde::de::Error::custom)?;
                let b = u8::from_str_radix(&s[5..7], 16).map_err(serde::de::Error::custom)?;
                Ok(Color {
                    unused: 0u8,
                    r,
                    g,
                    b,
                })
            }
            9 => {
                if &s[2..3] != "#" {
                    return Err(serde::de::Error::custom(
                        "Invalid color format, expected #RRGGBB",
                    ));
                }
                let unused = u8::from_str_radix(&s[0..2], 16).map_err(serde::de::Error::custom)?;
                let r = u8::from_str_radix(&s[3..5], 16).map_err(serde::de::Error::custom)?;
                let g = u8::from_str_radix(&s[5..7], 16).map_err(serde::de::Error::custom)?;
                let b = u8::from_str_radix(&s[7..9], 16).map_err(serde::de::Error::custom)?;
                Ok(Color { unused, r, g, b })
            }
            _ => Err(serde::de::Error::custom(
                "Invalid color format, expected #RRGGBB",
            )),
        }
    }
}

impl Color {
    pub fn from_rgb(arg: u32) -> Self {
        let r = ((arg >> 16) & 0xff) as u8;
        let g = ((arg >> 8) & 0xff) as u8;
        let b = (arg & 0xff) as u8;
        Color { r, g, b, unused: 0 }
    }

    pub fn to_rgb(&self) -> u32 {
        let r = (self.r as u32) << 16;
        let g = (self.g as u32) << 8;
        let b = self.b as u32;
        r | g | b
    }

    // Representation used in vpinball is Windows GDI COLORREF
    // https://learn.microsoft.com/en-us/windows/win32/gdi/colorref
    // 0x00bbggrr
    pub fn to_win_color(&self) -> u32 {
        let unused = (self.unused as u32) << 24;
        let r = self.r as u32;
        let g = (self.g as u32) << 8;
        let b = (self.b as u32) << 16;
        unused | r | g | b
    }

    pub fn from_win_color(arg: u32) -> Self {
        let unused = ((arg >> 24) & 0xff) as u8;
        let r = (arg & 0xff) as u8;
        let g = ((arg >> 8) & 0xff) as u8;
        let b = ((arg >> 16) & 0xff) as u8;
        Color { r, g, b, unused }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, unused: 0 }
    }

    pub fn biff_read(reader: &mut BiffReader<'_>) -> Color {
        // since we read in little endian, we need to read the color in BGR0 format
        let r = reader.get_u8();
        let g = reader.get_u8();
        let b = reader.get_u8();
        let unused = reader.get_u8();
        // if unused != 0 {
        //     eprintln!("Random data found in color: {unused} {r} {g} {b}");
        // }
        Color { r, g, b, unused }
    }

    pub fn biff_write(&self, writer: &mut BiffWriter) {
        // since we write in little endian, we need to write the color in BGR0 format
        writer.write_u8(self.r);
        writer.write_u8(self.g);
        writer.write_u8(self.b);
        writer.write_u8(self.unused);
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_serde() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        let s = serde_json::to_string(&color).unwrap();
        assert_eq!(s, "\"#123456\"");
        let color2: Color = serde_json::from_str(&s).unwrap();
        assert_eq!(color, color2);
    }

    #[test]
    fn test_color_biff() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        let mut writer = BiffWriter::new();
        color.biff_write(&mut writer);
        let data = writer.get_data();
        let mut reader = BiffReader::with_remaining(data, 4);
        let color2 = Color::biff_read(&mut reader);
        assert_eq!(color, color2);
    }

    #[test]
    fn test_color_biff_with_random_data() {
        let data: Vec<u8> = vec![0xFF, 0xFF, 0xFF, 0x12];
        let mut reader = BiffReader::with_remaining(&data, 4);
        let color = Color::biff_read(&mut reader);
        assert_eq!(color.r, 0xFF);
        assert_eq!(color.g, 0xFF);
        assert_eq!(color.b, 0xFF);
        assert_eq!(color.unused, 0x12);
        let mut writer = BiffWriter::new();
        color.biff_write(&mut writer);
        let data = writer.get_data();
        assert_eq!(data, vec![0xFF, 0xFF, 0xFF, 0x12]);
    }

    #[test]
    fn test_win_color() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        let win_color = color.to_win_color();
        assert_eq!(win_color, 0x00563412);
        let color2 = Color::from_win_color(win_color);
        assert_eq!(color, color2);
    }
}
