use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Vertex4D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vertex4D {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

impl std::fmt::Display for Vertex4D {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{},{},{},{}", self.x, self.y, self.z, self.w)
    }
}

impl Default for Vertex4D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }
}

impl BiffRead for Vertex4D {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let x = reader.get_f32();
        let y = reader.get_f32();
        let z = reader.get_f32();
        let w = reader.get_f32();
        Vertex4D { x, y, z, w }
    }
}

impl BiffWrite for Vertex4D {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_f32(self.x);
        writer.write_f32(self.y);
        writer.write_f32(self.z);
        writer.write_f32(self.w);
    }
}

#[cfg(test)]
mod tests {
    use crate::vpx::biff::BiffWriter;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_read() {
        let vertex = Vertex4D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        let mut writer = BiffWriter::new();
        Vertex4D::biff_write(&vertex, &mut writer);
        println!("{:?}", writer.get_data());
        let mut reader = BiffReader::with_remaining(writer.get_data(), 16);
        let vertex_read = Vertex4D::biff_read(&mut reader);
        assert_eq!(vertex, vertex_read);
    }
}
