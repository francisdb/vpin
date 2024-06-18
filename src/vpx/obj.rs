//! Wavefront OBJ file reader and writer

use crate::vpx::expanded::ReadMesh;
use std::error::Error;
use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use wavefront_rs::obj::entity::{Entity, FaceVertex};
use wavefront_rs::obj::parser::Parser;
use wavefront_rs::obj::writer::Writer;

// We have some issues where the data in the vpx file contains NaN values for normals.
// Therefore, we came up with an elaborate way to store the vpx normals data as a comment in the obj file.
// To be seen if we keep this as it comes with considerable overhead.

type VpxNormalBytes = [u8; 12];

fn obj_vpx_comment(bytes: &VpxNormalBytes) -> String {
    // a comment with the full normal bytes as hex string
    let hex = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(" ");
    format!("vpx {}", hex)
}

fn obj_parse_vpx_comment(comment: &str) -> Option<VpxNormalBytes> {
    if let Some(hex) = comment.strip_prefix("vpx ") {
        let bytes = hex
            .split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).unwrap())
            .collect::<Vec<u8>>();
        if bytes.len() == 12 {
            let mut result = [0; 12];
            result.copy_from_slice(&bytes);
            Some(result)
        } else {
            None
        }
    } else {
        None
    }
}

/// Writes a wavefront obj file from the vertices and indices
/// as they are stored in the m3cx and m3ci fields of the primitive
///
/// Somehow the z axis is inverted compared to the vpx file values,
/// so we have to negate the z values.
pub(crate) fn write_obj(
    name: String,
    mesh: &ReadMesh,
    obj_file_path: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let mut obj_file = File::create(obj_file_path)?;
    let mut writer = std::io::BufWriter::new(&mut obj_file);
    let obj_writer = Writer { auto_newline: true };

    let comment = Entity::Comment {
        content: "VPXTOOL table OBJ file".to_string(),
    };

    obj_writer.write(&mut writer, &comment)?;

    // // material library
    // let mtl_file_path = obj_file_path.with_extension("mtl");
    // let mtllib = Entity::MtlLib {
    //     name: mtl_file_path
    //         .file_name()
    //         .unwrap()
    //         .to_str()
    //         .unwrap()
    //         .to_string(),
    // };
    // obj_writer.write(&mut writer, &mtllib)?;

    let comment = Entity::Comment {
        content: "VPXTOOL OBJ file".to_string(),
    };
    obj_writer.write(&mut writer, &comment)?;
    let comment = Entity::Comment {
        content: format!(
            "numVerts: {} numFaces: {}",
            mesh.vertices.len(),
            mesh.indices.len()
        ),
    };
    obj_writer.write(&mut writer, &comment)?;

    // object name
    let object = Entity::Object { name };
    obj_writer.write(&mut writer, &object)?;

    // write all vertices to the wavefront obj file
    for v in &mesh.vertices {
        let vertex = Entity::Vertex {
            x: v.vertex.x as f64,
            y: v.vertex.y as f64,
            z: v.vertex.z as f64,
            w: None,
        };
        obj_writer.write(&mut writer, &vertex)?;
    }
    // write all vertex texture coordinates to the wavefront obj file
    for v in &mesh.vertices {
        let vertex = Entity::VertexTexture {
            u: v.vertex.tu as f64,
            v: Some(v.vertex.tv as f64),
            w: None,
        };
        obj_writer.write(&mut writer, &vertex)?;
    }
    // write all vertex normals to the wavefront obj file
    for v in &mesh.vertices {
        // if one of the values is NaN we write a special comment with the bytes
        if v.vertex.nx.is_nan() || v.vertex.ny.is_nan() || v.vertex.nz.is_nan() {
            println!("NaN found in vertex normal: {:?}", v.vertex);
            let data = v.raw[12..24].try_into().unwrap();
            let content = obj_vpx_comment(&data);
            let comment = Entity::Comment { content };
            obj_writer.write(&mut writer, &comment)?;
        }
        let vertex = Entity::VertexNormal {
            x: if v.vertex.nx.is_nan() {
                0.0
            } else {
                v.vertex.nx as f64
            },
            y: if v.vertex.ny.is_nan() {
                0.0
            } else {
                v.vertex.ny as f64
            },
            z: if v.vertex.nz.is_nan() {
                0.0
            } else {
                v.vertex.nz as f64
            },
        };
        obj_writer.write(&mut writer, &vertex)?;
    }

    // write all faces to the wavefront obj file

    // write in groups of 3
    for chunk in mesh.indices.chunks(3) {
        // obj indices are 1 based
        // since the z axis is inverted we have to reverse the order of the vertices
        let v1 = chunk[0] + 1;
        let v2 = chunk[1] + 1;
        let v3 = chunk[2] + 1;
        let face = Entity::Face {
            vertices: vec![
                FaceVertex::new_vtn(v1, Some(v1), Some(v1)),
                FaceVertex::new_vtn(v2, Some(v2), Some(v2)),
                FaceVertex::new_vtn(v3, Some(v3), Some(v3)),
            ],
        };
        obj_writer.write(&mut writer, &face)?;
    }
    Ok(())
}

pub(crate) fn read_obj_file(obj_file_path: &PathBuf) -> Result<ObjData, Box<dyn Error>> {
    let obj_file = File::open(obj_file_path)?;
    let mut reader = std::io::BufReader::new(obj_file);
    read_obj(&mut reader)
}

pub(crate) fn read_obj<R: BufRead>(mut reader: &mut R) -> Result<ObjData, Box<dyn Error>> {
    let mut indices: Vec<i64> = Vec::new();
    let mut vertices: Vec<(f64, f64, f64, Option<f64>)> = Vec::new();
    let mut texture_coordinates: Vec<(f64, Option<f64>, Option<f64>)> = Vec::new();
    let mut normals: Vec<ObjNormal> = Vec::new();
    let mut object_count = 0;
    let mut previous_comment: Option<String> = None;
    let mut name = String::new();
    Parser::read_to_end(&mut reader, |entity| {
        let mut comment: Option<String> = None;
        match entity {
            Entity::Vertex { x, y, z, w } => {
                vertices.push((x, y, z, w));
            }
            Entity::VertexTexture { u, v, w } => {
                texture_coordinates.push((u, v, w));
            }
            Entity::VertexNormal { x, y, z } => {
                if let Some(comment) = &previous_comment {
                    // parse the comment as hex string
                    if let Some(bytes) = obj_parse_vpx_comment(comment) {
                        // use the bytes as the normal
                        normals.push(((x, y, z), Some(bytes)));
                    } else {
                        normals.push(((x, y, z), None));
                    }
                } else {
                    normals.push(((x, y, z), None));
                }
            }
            Entity::Face { vertices } => {
                indices.push(vertices[0].vertex - 1);
                indices.push(vertices[1].vertex - 1);
                indices.push(vertices[2].vertex - 1);
            }
            Entity::Comment { content } => {
                // ignored
                comment = Some(content);
            }
            Entity::Object { name: n } => {
                object_count += 1;
                name = n;
            }
            other => {
                println!(
                    "Warning, skipping OBJ file entity of type: {:?}",
                    other.token()
                );
            }
        }
        previous_comment = comment;
    })?;
    assert_eq!(
        object_count, 1,
        "Only a single object is supported, found {}",
        object_count
    );

    Ok(ObjData {
        name,
        vertices,
        texture_coordinates,
        normals,
        indices,
    })
}

pub type ObjNormal = ((f64, f64, f64), Option<VpxNormalBytes>);

#[derive(Debug, PartialEq)]
pub(crate) struct ObjData {
    pub name: String,
    pub vertices: Vec<(f64, f64, f64, Option<f64>)>,
    pub texture_coordinates: Vec<(f64, Option<f64>, Option<f64>)>,
    pub normals: Vec<ObjNormal>,
    /// Indices can also be relative, so they can be negative
    /// stored by three as vertex, texture, normal are all the same
    ///
    /// Here they are 0-based, in obj files they are 1-based
    pub indices: Vec<i64>,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::vpx::expanded::ReadVertex;
    use crate::vpx::model::Vertex3dNoTex2;
    use pretty_assertions::assert_eq;
    use std::io::BufReader;
    use testdir::testdir;
    use testresult::TestResult;

    #[test]
    fn read_minimal_obj() -> TestResult {
        let obj_contents = r#"
o minimal
v 1.0 2.0 3.0
vt 2.0 4.0
vn 0.0 1.0 0.0
f 1/1/1 1/1/1 1/1/1
        "#;
        let mut reader = BufReader::new(obj_contents.as_bytes());
        let read_data = read_obj(&mut reader)?;
        let expected = ObjData {
            name: "minimal".to_string(),
            vertices: vec![(1.0, 2.0, 3.0, None)],
            texture_coordinates: vec![(2.0, Some(4.0), None)],
            normals: vec![((0.0, 1.0, 0.0), None)],
            indices: vec![0, 0, 0],
        };
        assert_eq!(read_data, expected);
        Ok(())
    }

    #[test]
    fn test_read_obj_with_nan() -> TestResult {
        let obj_contents = r#"o with_nan
v 1.0 2.0 3.0
vt 2.0 4.0
# vpx 01 02 03 04 05 06 07 08 09 0a 0b 0c
vn NaN 1.0 0.0
f 1/1/1 1/1/1 1/1/1
        "#;
        let mut reader = BufReader::new(obj_contents.as_bytes());
        let read_data = read_obj(&mut reader)?;
        let expected = ObjData {
            name: "with_nan".to_string(),
            vertices: vec![(1.0, 2.0, 3.0, None)],
            texture_coordinates: vec![(2.0, Some(4.0), None)],
            normals: vec![(
                (f64::NAN, 1.0, 0.0),
                Some([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            )],
            indices: vec![0, 0, 0],
        };
        // we can't compare a structure with NaN values
        assert_eq!(read_data.name, expected.name);
        assert_eq!(read_data.vertices, expected.vertices);
        assert_eq!(read_data.texture_coordinates, expected.texture_coordinates);
        assert_eq!(
            read_data.normals.first().unwrap().1,
            expected.normals.first().unwrap().1
        );
        assert_eq!(read_data.indices, expected.indices);
        Ok(())
    }

    #[test]
    fn test_read_write_obj() -> TestResult {
        let screw_path = PathBuf::from("testdata/screw.obj");
        let testdir = testdir!();
        let obj_data = read_obj_file(&screw_path)?;
        let written_obj_path = testdir.join("screw.obj");

        // zip vertices, texture coordinates and normals into a single vec
        let vertices: Vec<ReadVertex> = obj_data
            .vertices
            .iter()
            .zip(&obj_data.texture_coordinates)
            .zip(&obj_data.normals)
            .map(|((v, vt), (vn, _))| ReadVertex {
                raw: [0u8; 32],
                vertex: Vertex3dNoTex2 {
                    x: v.0 as f32,
                    y: v.1 as f32,
                    z: v.2 as f32,
                    nx: vn.0 as f32,
                    ny: vn.1 as f32,
                    nz: vn.2 as f32,
                    tu: vt.0 as f32,
                    tv: vt.1.unwrap_or(0.0) as f32,
                },
            })
            .collect();

        let mesh = ReadMesh {
            vertices,
            indices: obj_data.indices.clone(),
        };

        write_obj(obj_data.name, &mesh, &written_obj_path)?;

        // compare both files as strings
        let mut original = std::fs::read_to_string(&screw_path)?;
        // When on Windows the original file will be checked out with \r\n line endings.
        if cfg!(windows) {
            original = original.replace("\r\n", "\n")
        }
        // The obj file will always be written with \n line endings.
        let written = std::fs::read_to_string(&written_obj_path)?;
        pretty_assertions::assert_eq!(original, written);
        Ok(())
    }

    #[test]
    fn test_write_read_vpx_comment() {
        let bytes: VpxNormalBytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let comment = obj_vpx_comment(&bytes);
        let parsed = obj_parse_vpx_comment(&comment).unwrap();
        assert_eq!(bytes, parsed);
    }
}
