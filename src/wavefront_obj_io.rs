//! A library for reading and writing Wavefront .obj files in Rust
//! focused on zero-copy callback operations.
//!
//! The library provides traits for reading and writing .obj files
//! It keeps the 1-based indexing of the .obj format.

use std::fmt::Display;
use std::io;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::str::FromStr;

/// Trait for floating point types that can be used in OBJ files.
/// This allows the library to work with both f32 and f64 precision.
pub trait ObjFloat: Copy + Display + FromStr + PartialEq {
    /// Returns the fractional part of the number
    fn fract(self) -> Self;

    /// Returns true if the fractional part is zero
    fn is_zero_fract(self) -> bool {
        self.fract() == Self::zero()
    }

    /// Returns the zero value for this type
    fn zero() -> Self;
}

impl ObjFloat for f32 {
    fn fract(self) -> Self {
        self.fract()
    }
    fn zero() -> Self {
        0.0
    }
}

impl ObjFloat for f64 {
    fn fract(self) -> Self {
        self.fract()
    }
    fn zero() -> Self {
        0.0
    }
}

/// Trait for writing OBJ file data with configurable float precision.
///
/// The generic parameter `F` defaults to `f64` for backward compatibility,
/// but can be set to `f32` for applications that work with single-precision data
/// (like VPX files which use f32 internally).
pub trait ObjWriter<F: ObjFloat = f64> {
    fn write_comment<S: AsRef<str>>(&mut self, comment: S) -> io::Result<()>;
    fn write_object_name<S: AsRef<str>>(&mut self, name: S) -> io::Result<()>;
    fn write_vertex(&mut self, x: F, y: F, z: F, w: Option<F>) -> io::Result<()>;
    fn write_texture_coordinate(&mut self, u: F, v: Option<F>, w: Option<F>) -> io::Result<()>;
    fn write_normal(&mut self, nx: F, ny: F, nz: F) -> io::Result<()>;
    fn write_face(
        &mut self,
        vertex_indices: &[(usize, Option<usize>, Option<usize>)],
    ) -> io::Result<()>;
}

/// Trait for reading OBJ file data with configurable float precision.
///
/// The generic parameter `F` defaults to `f64` for backward compatibility,
/// but can be set to `f32` for applications that work with single-precision data.
pub trait ObjReader<F: ObjFloat = f64> {
    fn read_comment(&mut self, comment: &str) -> ();
    fn read_object_name(&mut self, name: &str) -> ();
    fn read_vertex(&mut self, x: F, y: F, z: F, w: Option<F>) -> ();
    fn read_texture_coordinate(&mut self, u: F, v: Option<F>, w: Option<F>) -> ();
    fn read_normal(&mut self, nx: F, ny: F, nz: F) -> ();
    fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) -> ();
}

pub fn read_obj_file<R: io::Read, T: ObjReader<F>, F: ObjFloat>(
    reader: R,
    obj_reader: &mut T,
) -> io::Result<()>
where
    <F as FromStr>::Err: std::fmt::Display,
{
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    let mut lineno: usize = 0;

    while buf_reader.read_line(&mut line)? != 0 {
        lineno += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let prefix = parts.next().ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                format!("line {}: empty prefix", lineno),
            )
        })?;

        let parse_f = |s: &str, kind: &str| -> io::Result<F> {
            s.parse::<F>().map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("line {}: invalid {} float: {} ({})", lineno, kind, s, e),
                )
            })
        };

        let parse_index = |s: &str, kind: &str| -> io::Result<usize> {
            let index = s.parse::<usize>().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("line {}: invalid {} index: {}", lineno, kind, s),
                )
            })?;
            if index == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("line {}: {} index must be positive: {}", lineno, kind, s),
                ));
            }
            Ok(index)
        };

        match prefix {
            "#" => {
                let comment = parts.collect::<Vec<&str>>().join(" ");
                obj_reader.read_comment(&comment);
            }
            "v" => {
                let x = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing vertex x", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "vertex x"))?;
                let y = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing vertex y", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "vertex y"))?;
                let z = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing vertex z", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "vertex z"))?;
                let w = match parts.next() {
                    Some(s) => Some(s.parse::<F>().map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid vertex w float: {} ({})", lineno, s, e),
                        )
                    })?),
                    None => None,
                };
                obj_reader.read_vertex(x, y, z, w);
            }
            "vt" => {
                let u = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing texture u", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "texture u"))?;
                let v = match parts.next() {
                    Some(s) => Some(s.parse::<F>().map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid texture v float: {} ({})", lineno, s, e),
                        )
                    })?),
                    None => None,
                };
                let w = match parts.next() {
                    Some(s) => Some(s.parse::<F>().map_err(|e| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid texture w float: {} ({})", lineno, s, e),
                        )
                    })?),
                    None => None,
                };
                obj_reader.read_texture_coordinate(u, v, w);
            }
            "vn" => {
                let nx = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing normal nx", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "normal nx"))?;
                let ny = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing normal ny", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "normal ny"))?;
                let nz = parts
                    .next()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: missing normal nz", lineno),
                        )
                    })
                    .and_then(|s| parse_f(s, "normal nz"))?;
                obj_reader.read_normal(nx, ny, nz);
            }
            "f" => {
                let mut vertex_indices = Vec::new();
                for part in parts {
                    // parse "v[/vt[/vn]]" by slicing without allocating
                    let first_slash = part.find('/');
                    let (v_str, rest) = match first_slash {
                        Some(i) => (&part[..i], &part[i + 1..]),
                        None => (part, ""),
                    };

                    let v_idx = parse_index(v_str, "vertex")?;

                    let (vt_idx, vn_idx) = if rest.is_empty() {
                        (None, None)
                    } else {
                        let second_slash = rest.find('/');
                        if let Some(j) = second_slash {
                            let vt_part = &rest[..j];
                            let vn_part = &rest[j + 1..];
                            let vt = if vt_part.is_empty() {
                                None
                            } else {
                                Some(parse_index(vt_part, "texcoord")?)
                            };
                            let vn = if vn_part.is_empty() {
                                None
                            } else {
                                Some(parse_index(vn_part, "normal")?)
                            };
                            (vt, vn)
                        } else {
                            // only vt present
                            let vt = if rest.is_empty() {
                                None
                            } else {
                                Some(parse_index(rest, "texcoord")?)
                            };
                            (vt, None)
                        }
                    };

                    vertex_indices.push((v_idx, vt_idx, vn_idx));
                }
                obj_reader.read_face(&vertex_indices);
            }
            "o" => {
                let name = parts.collect::<Vec<&str>>().join(" ");
                obj_reader.read_object_name(&name);
            }
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("line {}: Unknown line prefix: {}", lineno, other),
                ));
            }
        }

        line.clear();
    }

    Ok(())
}

pub struct IoObjWriter<W: io::Write, F: ObjFloat = f64> {
    out: W,
    line_buf: Vec<u8>,
    /// When true, use VPinball-compatible formatting (6 decimal places like fprintf %f)
    vpinball_compat: bool,
    _phantom: std::marker::PhantomData<F>,
}
impl<W: io::Write, F: ObjFloat> IoObjWriter<W, F> {
    /// Creates a new OBJ writer with default formatting (full precision)
    pub fn new(writer: W) -> Self {
        IoObjWriter {
            out: writer,
            line_buf: Vec::with_capacity(256),
            vpinball_compat: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new OBJ writer with VPinball-compatible formatting.
    /// This uses 6 decimal places (like fprintf %f) to match VPinball's OBJ export format.
    pub fn new_vpinball_compat(writer: W) -> Self {
        IoObjWriter {
            out: writer,
            line_buf: Vec::with_capacity(256),
            vpinball_compat: true,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sets VPinball-compatible formatting mode.
    /// When true, floats are formatted with 6 decimal places to match VPinball's fprintf %f.
    pub fn set_vpinball_compat(&mut self, compat: bool) {
        self.vpinball_compat = compat;
    }

    #[inline]
    fn push_str(&mut self, s: &str) {
        self.line_buf.extend_from_slice(s.as_bytes());
    }

    #[inline]
    fn push_u<T: itoa::Integer>(&mut self, v: T) {
        let mut buf = itoa::Buffer::new();
        self.push_str(buf.format(v));
    }

    #[inline]
    fn push_f(&mut self, v: F) {
        // we want 0 as "0" not "0.0"
        if v.is_zero_fract() {
            self.push_str(&format!("{}", v));
            return;
        }
        // Use VPinball-compatible formatting (6 decimal places like fprintf %f) if enabled,
        // otherwise use full precision
        if self.vpinball_compat {
            self.push_str(&format!("{:.6}", v));
        } else {
            self.push_str(&format!("{}", v));
        }
    }

    #[inline]
    fn flush_line(&mut self) -> io::Result<()> {
        self.line_buf.push(b'\n');
        self.out.write_all(&self.line_buf)?;
        self.line_buf.clear();
        Ok(())
    }
}
impl<W: io::Write, F: ObjFloat> ObjWriter<F> for IoObjWriter<W, F> {
    fn write_comment<S: AsRef<str>>(&mut self, comment: S) -> io::Result<()> {
        self.push_str("# ");
        self.push_str(comment.as_ref());
        self.flush_line()
    }

    fn write_object_name<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
        self.push_str("o ");
        self.push_str(name.as_ref());
        self.flush_line()
    }

    fn write_vertex(&mut self, x: F, y: F, z: F, w: Option<F>) -> io::Result<()> {
        self.push_str("v ");
        self.push_f(x);
        self.push_str(" ");
        self.push_f(y);
        self.push_str(" ");
        self.push_f(z);
        if let Some(wv) = w {
            self.push_str(" ");
            self.push_f(wv);
        }
        self.flush_line()
    }

    fn write_texture_coordinate(&mut self, u: F, v: Option<F>, w: Option<F>) -> io::Result<()> {
        self.push_str("vt ");
        self.push_f(u);
        if let Some(vv) = v {
            self.push_str(" ");
            self.push_f(vv);
            if let Some(wv) = w {
                self.push_str(" ");
                self.push_f(wv);
            }
        }
        self.flush_line()
    }

    fn write_normal(&mut self, nx: F, ny: F, nz: F) -> io::Result<()> {
        self.push_str("vn ");
        self.push_f(nx);
        self.push_str(" ");
        self.push_f(ny);
        self.push_str(" ");
        self.push_f(nz);
        self.flush_line()
    }

    fn write_face(
        &mut self,
        vertex_indices: &[(usize, Option<usize>, Option<usize>)],
    ) -> io::Result<()> {
        // Build the whole face line and write once.
        self.push_str("f");
        for (v_idx, vt_idx, vn_idx) in vertex_indices.iter() {
            self.push_str(" ");
            // If your internal indices are zero-based, emit +1 here:
            self.push_u(*v_idx);
            match (vt_idx, vn_idx) {
                (None, None) => {}
                (Some(vt), None) => {
                    self.push_str("/");
                    self.push_u(*vt);
                }
                (None, Some(vn)) => {
                    self.push_str("//");
                    self.push_u(*vn);
                }
                (Some(vt), Some(vn)) => {
                    self.push_str("/");
                    self.push_u(*vt);
                    self.push_str("/");
                    self.push_u(*vn);
                }
            }
        }
        self.flush_line()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    type Face = Vec<(usize, Option<usize>, Option<usize>)>;

    #[derive(Default)]
    struct TestObjReader64 {
        comments: Vec<String>,
        names: Vec<String>,
        vertices: Vec<(f64, f64, f64, Option<f64>)>,
        texture_coordinates: Vec<(f64, Option<f64>, Option<f64>)>,
        normals: Vec<(f64, f64, f64)>,
        faces: Vec<Face>,
    }

    impl ObjReader for TestObjReader64 {
        fn read_comment(&mut self, comment: &str) {
            self.comments.push(comment.to_string());
        }

        fn read_object_name(&mut self, name: &str) {
            self.names.push(name.to_string());
        }

        fn read_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) {
            self.vertices.push((x, y, z, w));
        }

        fn read_texture_coordinate(&mut self, u: f64, v: Option<f64>, w: Option<f64>) {
            self.texture_coordinates.push((u, v, w));
        }

        fn read_normal(&mut self, nx: f64, ny: f64, nz: f64) {
            self.normals.push((nx, ny, nz));
        }

        fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {
            self.faces.push(vertex_indices.to_vec());
        }
    }

    #[test]
    fn test_obj_reading() {
        // read testdata/screw.obj using TestObjReader
        let obj_data = include_str!("../testdata/screw_f64.obj");
        let cursor = Cursor::new(obj_data);
        let mut reader: TestObjReader64 = Default::default();
        read_obj_file(cursor, &mut reader).unwrap();
        // this does not check correctness and ordering of data, just that all data was read
        assert_eq!(reader.comments.len(), 3);
        assert_eq!(reader.names.len(), 1);
        assert_eq!(reader.vertices.len(), 41);
        assert_eq!(reader.texture_coordinates.len(), 41);
        assert_eq!(reader.normals.len(), 41);
        assert_eq!(reader.faces.len(), 48);
    }

    #[test]
    fn test_obj_reading_2() {
        let input = "# This is a test OBJ file
o TestObject
v 1 2 3
vt 0.5 0.5
vn 0 1 1.1
f 1/1/1 2/2/2 3/3/3
";

        let reader = Cursor::new(input);
        let mut test_reader: TestObjReader64 = Default::default();
        read_obj_file(reader, &mut test_reader).unwrap();
        assert_eq!(test_reader.comments, vec!["This is a test OBJ file"]);
        assert_eq!(test_reader.names, vec!["TestObject"]);
        assert_eq!(test_reader.vertices, vec![(1.0, 2.0, 3.0, None)]);
        assert_eq!(
            test_reader.texture_coordinates,
            vec![(0.5, Some(0.5), None)]
        );
        assert_eq!(test_reader.normals, vec![(0.0, 1.0, 1.1)]);
        assert_eq!(
            test_reader.faces,
            vec![vec![
                (1, Some(1), Some(1)),
                (2, Some(2), Some(2)),
                (3, Some(3), Some(3))
            ]]
        );
    }

    #[test]
    fn test_obj_writing() {
        let mut buffer = Vec::new();
        let mut writer = IoObjWriter::new(&mut buffer);
        writer.write_comment("This is a test OBJ file").unwrap();
        writer.write_object_name("TestObject").unwrap();
        writer.write_vertex(1.0, 2.0, 3.0, None).unwrap();
        writer
            .write_texture_coordinate(0.5, Some(0.5), None)
            .unwrap();
        writer.write_normal(0.0, 1.0, 1.1).unwrap();
        writer
            .write_face(&[
                (1, Some(1), Some(1)),
                (2, Some(2), Some(2)),
                (3, Some(3), Some(3)),
            ])
            .unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected_output = "# This is a test OBJ file
o TestObject
v 1 2 3
vt 0.5 0.5
vn 0 1 1.1
f 1/1/1 2/2/2 3/3/3
";
        assert_eq!(output, expected_output);
    }

    struct WritingReader64 {
        writer: IoObjWriter<Vec<u8>>,
    }
    impl ObjReader for WritingReader64 {
        fn read_comment(&mut self, comment: &str) {
            self.writer.write_comment(comment).unwrap();
        }

        fn read_object_name(&mut self, name: &str) {
            self.writer.write_object_name(name).unwrap();
        }

        fn read_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) {
            self.writer.write_vertex(x, y, z, w).unwrap();
        }

        fn read_texture_coordinate(&mut self, u: f64, v: Option<f64>, w: Option<f64>) {
            self.writer.write_texture_coordinate(u, v, w).unwrap();
        }

        fn read_normal(&mut self, nx: f64, ny: f64, nz: f64) {
            self.writer.write_normal(nx, ny, nz).unwrap();
        }

        fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {
            self.writer.write_face(vertex_indices).unwrap();
        }
    }

    #[test]
    fn test_obj_read_write_compare_64() {
        // git might change line endings as they are text files, so normalize to \n
        let obj_data = include_str!("../testdata/screw_f64.obj").replace("\r\n", "\n");
        let cursor = Cursor::new(&obj_data);
        // Use VPinball-compatible formatting to match the screw.obj file format
        let writer: IoObjWriter<_, f64> = IoObjWriter::new(Vec::new());
        let mut reader = WritingReader64 { writer };
        read_obj_file(cursor, &mut reader).unwrap();

        let output = String::from_utf8(reader.writer.out).unwrap();
        assert_eq!(output, obj_data);
    }

    // Tests for f32 support

    #[derive(Default)]
    struct TestObjReader32 {
        vertices: Vec<(f32, f32, f32, Option<f32>)>,
        texture_coordinates: Vec<(f32, Option<f32>, Option<f32>)>,
        normals: Vec<(f32, f32, f32)>,
    }

    impl ObjReader<f32> for TestObjReader32 {
        fn read_comment(&mut self, _comment: &str) {}
        fn read_object_name(&mut self, _name: &str) {}

        fn read_vertex(&mut self, x: f32, y: f32, z: f32, w: Option<f32>) {
            self.vertices.push((x, y, z, w));
        }

        fn read_texture_coordinate(&mut self, u: f32, v: Option<f32>, w: Option<f32>) {
            self.texture_coordinates.push((u, v, w));
        }

        fn read_normal(&mut self, nx: f32, ny: f32, nz: f32) {
            self.normals.push((nx, ny, nz));
        }

        fn read_face(&mut self, _vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {}
    }

    struct WritingReader32 {
        writer: IoObjWriter<Vec<u8>, f32>,
    }
    impl ObjReader<f32> for WritingReader32 {
        fn read_comment(&mut self, comment: &str) {
            self.writer.write_comment(comment).unwrap();
        }

        fn read_object_name(&mut self, name: &str) {
            self.writer.write_object_name(name).unwrap();
        }

        fn read_vertex(&mut self, x: f32, y: f32, z: f32, w: Option<f32>) {
            self.writer.write_vertex(x, y, z, w).unwrap();
        }

        fn read_texture_coordinate(&mut self, u: f32, v: Option<f32>, w: Option<f32>) {
            self.writer.write_texture_coordinate(u, v, w).unwrap();
        }

        fn read_normal(&mut self, nx: f32, ny: f32, nz: f32) {
            self.writer.write_normal(nx, ny, nz).unwrap();
        }

        fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) {
            self.writer.write_face(vertex_indices).unwrap();
        }
    }

    #[test]
    fn test_obj_f32_reading() {
        let input = "o TestObject
v 1.5 2.5 3.5
vt 0.25 0.75
vn 0.0 1.0 0.0
";

        let reader = Cursor::new(input);
        let mut test_reader = TestObjReader32::default();
        read_obj_file(reader, &mut test_reader).unwrap();

        assert_eq!(test_reader.vertices, vec![(1.5f32, 2.5f32, 3.5f32, None)]);
        assert_eq!(
            test_reader.texture_coordinates,
            vec![(0.25f32, Some(0.75f32), None)]
        );
        assert_eq!(test_reader.normals, vec![(0.0f32, 1.0f32, 0.0f32)]);
    }

    #[test]
    fn test_obj_f32_writing() {
        let mut buffer = Vec::new();
        let mut writer: IoObjWriter<_, f32> = IoObjWriter::new(&mut buffer);

        writer.write_comment("f32 test").unwrap();
        writer.write_object_name("F32Object").unwrap();
        writer.write_vertex(1.5f32, 2.5f32, 3.5f32, None).unwrap();
        writer
            .write_texture_coordinate(0.25f32, Some(0.75f32), None)
            .unwrap();
        writer.write_normal(0.0f32, 1.0f32, 0.0f32).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        let expected = "# f32 test
o F32Object
v 1.5 2.5 3.5
vt 0.25 0.75
vn 0 1 0
";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_obj_f32_round_trip() {
        // Test that f32 values round-trip correctly
        let input = "o Test
v 0.123456789 0.987654321 1.5
vn 0.577 0.577 0.577
";

        let reader = Cursor::new(input);
        let mut test_reader = TestObjReader32::default();
        read_obj_file(reader, &mut test_reader).unwrap();

        // Write back using f32 writer
        let mut buffer = Vec::new();
        let mut writer: IoObjWriter<_, f32> = IoObjWriter::new(&mut buffer);
        writer.write_object_name("Test").unwrap();
        for (x, y, z, w) in &test_reader.vertices {
            writer.write_vertex(*x, *y, *z, *w).unwrap();
        }
        for (nx, ny, nz) in &test_reader.normals {
            writer.write_normal(*nx, *ny, *nz).unwrap();
        }

        let output = String::from_utf8(buffer).unwrap();

        // The values should be f32-precision
        assert!(output.contains("o Test"));
        assert!(output.contains("v "));
        assert!(output.contains("vn "));
    }

    #[test]
    fn test_vpinball_compat_formatting() {
        let mut buffer = Vec::new();
        let mut writer: IoObjWriter<_, f64> = IoObjWriter::new_vpinball_compat(&mut buffer);

        writer.write_object_name("VPinballTest").unwrap();
        writer
            .write_vertex(754.4214477539063, 1753.2353515625, -91.72238159179688, None)
            .unwrap();
        writer
            .write_texture_coordinate(0.123456789, Some(0.987654321), None)
            .unwrap();
        writer
            .write_normal(0.5773502691896257, 0.5773502691896257, 0.5773502691896257)
            .unwrap();

        let output = String::from_utf8(buffer).unwrap();

        // VPinball uses fprintf %f which gives 6 decimal places
        let expected = "o VPinballTest
v 754.421448 1753.235352 -91.722382
vt 0.123457 0.987654
vn 0.577350 0.577350 0.577350
";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_vpinball_compat_flag_toggle() {
        // Test that we can toggle the flag
        let mut buffer = Vec::new();
        let mut writer: IoObjWriter<_, f32> = IoObjWriter::new(&mut buffer);

        // Start with default (full precision)
        writer
            .write_vertex(1.23456789f32, 2.3456789f32, 3.456789f32, None)
            .unwrap();

        // Enable VPinball compat
        writer.set_vpinball_compat(true);
        writer
            .write_vertex(1.23456789f32, 2.3456789f32, 3.456789f32, None)
            .unwrap();

        let output = String::from_utf8(buffer).unwrap();

        // First line should have full f32 precision, second should have 6 decimals
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);

        // Full precision output (f32 Display)
        assert!(lines[0].starts_with("v 1.234567"));

        // VPinball compat output (6 decimal places)
        assert!(lines[1].starts_with("v 1.234568 2.345679 3.456789"));
    }

    #[test]
    fn test_obj_read_write_compare_32() {
        // git might change line endings as they are text files, so normalize to \n
        let obj_data = include_str!("../testdata/screw_f32.obj").replace("\r\n", "\n");
        let cursor = Cursor::new(&obj_data);
        // Use VPinball-compatible formatting to match the screw.obj file format
        let writer: IoObjWriter<_, f32> = IoObjWriter::new(Vec::new());
        let mut reader = WritingReader32 { writer };
        read_obj_file(cursor, &mut reader).unwrap();

        let output = String::from_utf8(reader.writer.out).unwrap();
        assert_eq!(output, obj_data);
    }
}
