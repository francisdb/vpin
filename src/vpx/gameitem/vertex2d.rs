use crate::vpx::biff::{BiffRead, BiffReader, BiffWrite, BiffWriter};
use fake::Dummy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Dummy, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Vertex2D {
    pub x: f32,
    pub y: f32,
}

impl Vertex2D {
    pub fn new(x: f32, y: f32) -> Vertex2D {
        Vertex2D { x, y }
    }
}

impl std::fmt::Display for Vertex2D {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

impl Default for Vertex2D {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl BiffRead for Vertex2D {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self {
        let x = reader.get_f32();
        let y = reader.get_f32();
        Vertex2D { x, y }
    }
}

impl BiffWrite for Vertex2D {
    fn biff_write(&self, writer: &mut BiffWriter) {
        writer.write_f32(self.x);
        writer.write_f32(self.y);
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
        let vertex = Vertex2D { x: 1.0, y: 2.0 };
        let mut writer = BiffWriter::new();
        Vertex2D::biff_write(&vertex, &mut writer);
        let mut reader = BiffReader::with_remaining(writer.get_data(), 8);
        let vertex_read = Vertex2D::biff_read(&mut reader);
        assert_eq!(vertex, vertex_read);
    }
}
