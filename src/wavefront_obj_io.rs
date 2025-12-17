//! A library for reading and writing Wavefront .obj files in Rust
//! focused on zero-copy callback operations.
//!
//! The library provides traits for reading and writing .obj files
//! It keeps the 1-based indexing of the .obj format.

use std::io;
use std::io::{BufRead, BufReader, Error, ErrorKind};

pub trait ObjWriter {
    fn write_comment<S: AsRef<str>>(&mut self, comment: S) -> io::Result<()>;
    fn write_object_name<S: AsRef<str>>(&mut self, name: S) -> io::Result<()>;
    fn write_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) -> io::Result<()>;
    fn write_texture_coordinate(
        &mut self,
        u: f64,
        v: Option<f64>,
        w: Option<f64>,
    ) -> io::Result<()>;
    fn write_normal(&mut self, nx: f64, ny: f64, nz: f64) -> io::Result<()>;
    fn write_face(
        &mut self,
        vertex_indices: &[(usize, Option<usize>, Option<usize>)],
    ) -> io::Result<()>;
}

pub trait ObjReader {
    fn read_comment(&mut self, comment: &str) -> ();
    fn read_object_name(&mut self, name: &str) -> ();
    fn read_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) -> ();
    fn read_texture_coordinate(&mut self, u: f64, v: Option<f64>, w: Option<f64>) -> ();
    fn read_normal(&mut self, nx: f64, ny: f64, nz: f64) -> ();
    fn read_face(&mut self, vertex_indices: &[(usize, Option<usize>, Option<usize>)]) -> ();
}

pub fn read_obj_file<R: io::Read, T: ObjReader>(reader: R, obj_reader: &mut T) -> io::Result<()> {
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

        let parse_f = |s: &str, kind: &str| -> io::Result<f64> {
            s.parse::<f64>().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("line {}: invalid {} float: {}", lineno, kind, s),
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
                    Some(s) => Some(s.parse::<f64>().map_err(|_| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid vertex w float: {}", lineno, s),
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
                    Some(s) => Some(s.parse::<f64>().map_err(|_| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid texture v float: {}", lineno, s),
                        )
                    })?),
                    None => None,
                };
                let w = match parts.next() {
                    Some(s) => Some(s.parse::<f64>().map_err(|_| {
                        Error::new(
                            ErrorKind::InvalidData,
                            format!("line {}: invalid texture w float: {}", lineno, s),
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

pub struct IoObjWriter<W: io::Write> {
    out: W,
    line_buf: Vec<u8>,
}
impl<W: io::Write> IoObjWriter<W> {
    pub fn new(writer: W) -> Self {
        IoObjWriter {
            out: writer,
            line_buf: Vec::with_capacity(256),
        }
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
    fn push_f(&mut self, v: f64) {
        // we want 0 as "0" not "0.0"
        if v.fract() == 0.0 {
            self.push_str(&format!("{}", v));
            return;
        }
        let mut buf = ryu::Buffer::new();
        self.push_str(buf.format(v));
    }

    #[inline]
    fn flush_line(&mut self) -> io::Result<()> {
        self.line_buf.push(b'\n');
        self.out.write_all(&self.line_buf)?;
        self.line_buf.clear();
        Ok(())
    }
}
impl<W: io::Write> ObjWriter for IoObjWriter<W> {
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

    fn write_vertex(&mut self, x: f64, y: f64, z: f64, w: Option<f64>) -> io::Result<()> {
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

    fn write_texture_coordinate(
        &mut self,
        u: f64,
        v: Option<f64>,
        w: Option<f64>,
    ) -> io::Result<()> {
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

    fn write_normal(&mut self, nx: f64, ny: f64, nz: f64) -> io::Result<()> {
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
    struct TestObjReader {
        comments: Vec<String>,
        names: Vec<String>,
        vertices: Vec<(f64, f64, f64, Option<f64>)>,
        texture_coordinates: Vec<(f64, Option<f64>, Option<f64>)>,
        normals: Vec<(f64, f64, f64)>,
        faces: Vec<Face>,
    }

    impl ObjReader for TestObjReader {
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
        let obj_data = include_str!("../testdata/screw.obj");
        let cursor = Cursor::new(obj_data);
        let mut reader: TestObjReader = Default::default();
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
        let mut test_reader: TestObjReader = Default::default();
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

    struct WritingReader {
        writer: IoObjWriter<Vec<u8>>,
    }
    impl ObjReader for WritingReader {
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
    fn test_obj_read_write_compare() {
        let obj_data = include_str!("../testdata/screw.obj").replace("\r\n", "\n");
        let cursor = Cursor::new(&obj_data);
        let writer = IoObjWriter::new(Vec::new());
        let mut reader = WritingReader { writer };
        read_obj_file(cursor, &mut reader).unwrap();

        let output = String::from_utf8(reader.writer.out).unwrap();
        assert_eq!(output, obj_data);
    }
}
