use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Vertex3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vertex3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn write_unpadded(&self, writer: &mut BiffWriter) {
        writer.write_f32(self.x);
        writer.write_f32(self.y);
        writer.write_f32(self.z);
    }

    pub fn read_unpadded(reader: &mut BiffReader<'_>) -> Self {
        let x = reader.get_f32();
        let y = reader.get_f32();
        let z = reader.get_f32();
        Vertex3D { x, y, z }
    }

    pub fn write_padded(&self, writer: &mut BiffWriter) {
        writer.write_f32(self.x);
        writer.write_f32(self.y);
        writer.write_f32(self.z);
        writer.write_f32(0.0); // padding
    }

    pub fn read_padded(reader: &mut BiffReader<'_>) -> Self {
        let x = reader.get_f32();
        let y = reader.get_f32();
        let z = reader.get_f32();
        let _padding = reader.get_f32(); // read and ignore padding
        Vertex3D { x, y, z }
    }
}

impl std::fmt::Display for Vertex3D {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{},{},{}", self.x, self.y, self.z)
    }
}

impl Default for Vertex3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// For we default to 16 bytes with padding
impl BiffRead for Vertex3D {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        Vertex3D::read_padded(reader)
    }
}

/// For we default to 16 bytes with padding
impl BiffWrite for Vertex3D {
    fn biff_write(&self, writer: &mut BiffWriter) {
        Vertex3D::write_padded(self, writer)
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
        let vertex = Vertex3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let mut writer = BiffWriter::new();
        Vertex3D::biff_write(&vertex, &mut writer);
        let mut reader = BiffReader::with_remaining(writer.get_data(), 16);
        let vertex_read = Vertex3D::biff_read(&mut reader);
        assert_eq!(vertex, vertex_read);
    }
}
