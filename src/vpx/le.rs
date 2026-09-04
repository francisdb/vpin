//! Little-endian read and write helpers for `Read` and `Write` streams.

use std::io::{self, Read, Write};

/// Read little-endian values from a `Read` stream
pub(crate) trait ReadLe: Read {
    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut buf = [0; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_f32_le(&mut self) -> io::Result<f32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }
}

impl<R: Read + ?Sized> ReadLe for R {}

/// Write little-endian values to a `Write` stream
pub(crate) trait WriteLe: Write {
    fn write_u16_le(&mut self, value: u16) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_u32_le(&mut self, value: u32) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }

    fn write_f32_le(&mut self, value: f32) -> io::Result<()> {
        self.write_all(&value.to_le_bytes())
    }
}

impl<W: Write + ?Sized> WriteLe for W {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip() {
        let mut bytes = Vec::new();
        bytes.write_u16_le(0x1234).unwrap();
        bytes.write_u32_le(0xDEAD_BEEF).unwrap();
        bytes.write_f32_le(1.5).unwrap();
        assert_eq!(
            bytes,
            [0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE, 0, 0, 0xC0, 0x3F]
        );
        let mut cursor = Cursor::new(bytes);
        assert_eq!(cursor.read_u16_le().unwrap(), 0x1234);
        assert_eq!(cursor.read_u32_le().unwrap(), 0xDEAD_BEEF);
        assert_eq!(cursor.read_f32_le().unwrap(), 1.5);
        assert!(cursor.read_u32_le().is_err());
    }
}
