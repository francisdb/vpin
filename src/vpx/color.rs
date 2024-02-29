use crate::vpx::biff::BiffReader;
use fake::Dummy;
use serde::{Deserialize, Serialize};

use super::biff::BiffWriter;

#[derive(Debug, PartialEq, Clone, Copy, Dummy)]
pub struct Color {
    a: u8,
    r: u8,
    g: u8,
    b: u8,
}

// TODO we might want to switch to a more standard format like #AARRGGBB
#[derive(Debug, PartialEq)]
pub(crate) struct ColorJson {
    a: u8,
    r: u8,
    g: u8,
    b: u8,
}

impl ColorJson {
    pub fn from_color(color: &Color) -> Self {
        Self {
            a: color.a,
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
    pub fn to_color(&self) -> Color {
        Color {
            a: self.a,
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }
}

/**
 * This is a custom serializer for the ColorJson struct.
 * It serializes the color as a string in the format "#AARRGGBB".
 */
impl Serialize for ColorJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = format!("#{:02x}{:02x}{:02x}{:02x}", self.a, self.r, self.g, self.b);
        serializer.serialize_str(&s)
    }
}

/**
 * This is a custom deserializer for the ColorJson struct.
 * It deserializes the color from a string in the format "#AARRGGBB".
 */
impl<'de> Deserialize<'de> for ColorJson {
    fn deserialize<D>(deserializer: D) -> Result<ColorJson, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() != 9 {
            return Err(serde::de::Error::custom(
                "Invalid color format, expected #AARRGGBB",
            ));
        }
        if &s[0..1] != "#" {
            return Err(serde::de::Error::custom(
                "Invalid color format, expected #AARRGGBB",
            ));
        }
        let a = u8::from_str_radix(&s[1..3], 16).map_err(serde::de::Error::custom)?;
        let r = u8::from_str_radix(&s[3..5], 16).map_err(serde::de::Error::custom)?;
        let g = u8::from_str_radix(&s[5..7], 16).map_err(serde::de::Error::custom)?;
        let b = u8::from_str_radix(&s[7..9], 16).map_err(serde::de::Error::custom)?;
        Ok(ColorJson { a, r, g, b })
    }
}

impl Color {
    pub fn from_argb(arg: u32) -> Color {
        let a = ((arg >> 24) & 0xff) as u8;
        let r = ((arg >> 16) & 0xff) as u8;
        let g = ((arg >> 8) & 0xff) as u8;
        let b = (arg & 0xff) as u8;
        Color { a, r, g, b }
    }

    // deprecated
    #[deprecated(since = "0.1.0", note = "Please use `from_argb` instead")]
    pub fn new_argb(arg: u32) -> Color {
        Self::from_argb(arg)
    }

    pub fn new_bgr(arg: u32) -> Color {
        let a = ((arg >> 24) & 0xff) as u8;
        let b = ((arg >> 16) & 0xff) as u8;
        let g = ((arg >> 8) & 0xff) as u8;
        let r = (arg & 0xff) as u8;
        Color { a, r, g, b }
    }

    pub fn bgr(&self) -> u32 {
        let a = (self.a as u32) << 24;
        let b = (self.b as u32) << 16;
        let g = (self.g as u32) << 8;
        let r = self.r as u32;
        a | b | g | r
    }

    pub fn argb(&self) -> u32 {
        let a = (self.a as u32) << 24;
        let r = (self.r as u32) << 16;
        let g = (self.g as u32) << 8;
        let b = self.b as u32;
        a | r | g | b
    }

    pub const BLACK: Color = Color {
        a: 255,
        r: 0,
        g: 0,
        b: 0,
    };
    pub const WHITE: Color = Color {
        a: 255,
        r: 255,
        g: 255,
        b: 255,
    };
    pub const RED: Color = Color {
        a: 255,
        r: 255,
        g: 0,
        b: 0,
    };

    // TODO do we want a BiffRead with a parameter?

    pub fn biff_read_argb(reader: &mut BiffReader<'_>) -> Color {
        let a = reader.get_u8();
        let r = reader.get_u8();
        let g = reader.get_u8();
        let b = reader.get_u8();
        Color { a, r, g, b }
    }

    pub fn biff_read_bgr(reader: &mut BiffReader<'_>) -> Color {
        let a = reader.get_u8();
        let b = reader.get_u8();
        let g = reader.get_u8();
        let r = reader.get_u8();
        Color { a, r, g, b }
    }

    pub fn biff_write_argb(&self, writer: &mut BiffWriter) {
        writer.write_u8(self.a);
        writer.write_u8(self.r);
        writer.write_u8(self.g);
        writer.write_u8(self.b);
    }

    pub fn biff_write_bgr(&self, writer: &mut BiffWriter) {
        writer.write_u8(self.a);
        writer.write_u8(self.b);
        writer.write_u8(self.g);
        writer.write_u8(self.r);
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}",
            self.a, self.r, self.g, self.b
        )
    }
}
