//! Common utilities for mesh generation

use super::{PrimitiveMeshFormat, WriteError};
use crate::filesystem::FileSystem;
use crate::vpx::gameitem::primitive::VertexWrapper;
use crate::vpx::gltf::{GltfContainer, write_gltf};
use crate::vpx::obj::{VpxFace, write_obj};
use std::path::Path;

/// A 2D vector helper used for geometry calculations
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Vec2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Vec2 {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Vec2 {
            x: self.x * s,
            y: self.y * s,
        }
    }
}

impl Vec2 {
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self { x: 0.0, y: 0.0 }
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }
}

/// A 3D vector helper used for geometry calculations
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Vec3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

impl Vec3 {
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        }
    }

    pub fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
        Vec3 {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }
}

/// A 2D render vertex used during spline generation
/// Mirrors VPinball's RenderVertex from mesh.h
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderVertex2D {
    pub x: f32,
    pub y: f32,
    #[allow(dead_code)]
    pub smooth: bool,
    #[allow(dead_code)]
    pub slingshot: bool,
    #[allow(dead_code)]
    pub control_point: bool,
}

/// A 3D render vertex used during curve generation
/// Mirrors VPinball's RenderVertex3D from mesh.h
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderVertex3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[allow(dead_code)]
    pub smooth: bool,
    #[allow(dead_code)]
    pub slingshot: bool,
    #[allow(dead_code)]
    pub control_point: bool,
}

/// Generate the file name for a generated mesh file
pub(super) fn generated_mesh_file_name(
    json_file_name: &str,
    mesh_format: PrimitiveMeshFormat,
) -> String {
    let extension = match mesh_format {
        PrimitiveMeshFormat::Obj => "obj",
        PrimitiveMeshFormat::Glb => "glb",
        PrimitiveMeshFormat::Gltf => "gltf",
    };
    format!("{json_file_name}-generated.{extension}")
}

/// Write a mesh to a file in the specified format
pub(super) fn write_mesh_to_file(
    mesh_path: &Path,
    name: &str,
    vertices: &[VertexWrapper],
    indices: &[VpxFace],
    mesh_format: PrimitiveMeshFormat,
    fs: &dyn FileSystem,
) -> Result<(), WriteError> {
    match mesh_format {
        PrimitiveMeshFormat::Obj => write_obj(name, vertices, indices, mesh_path, fs)
            .map_err(|e| WriteError::Io(std::io::Error::other(format!("{e}"))))?,
        PrimitiveMeshFormat::Glb => {
            write_gltf(name, vertices, indices, mesh_path, GltfContainer::Glb, fs)
                .map_err(|e| WriteError::Io(std::io::Error::other(format!("{e}"))))?
        }
        PrimitiveMeshFormat::Gltf => {
            write_gltf(name, vertices, indices, mesh_path, GltfContainer::Gltf, fs)
                .map_err(|e| WriteError::Io(std::io::Error::other(format!("{e}"))))?
        }
    }
    Ok(())
}

use crate::vpx::model::Vertex3dNoTex2;
use std::f32::consts::PI;

/// Compute normals for a mesh by accumulating face normals
pub(super) fn compute_normals(vertices: &mut [Vertex3dNoTex2], indices: &[u32]) {
    // Reset all normals
    for v in vertices.iter_mut() {
        v.nx = 0.0;
        v.ny = 0.0;
        v.nz = 0.0;
    }

    // Accumulate face normals
    for tri in indices.chunks_exact(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() {
            continue;
        }

        let v0 = &vertices[i0];
        let v1 = &vertices[i1];
        let v2 = &vertices[i2];

        let e1 = Vec3 {
            x: v1.x - v0.x,
            y: v1.y - v0.y,
            z: v1.z - v0.z,
        };
        let e2 = Vec3 {
            x: v2.x - v0.x,
            y: v2.y - v0.y,
            z: v2.z - v0.z,
        };
        let n = Vec3::cross(&e1, &e2);

        vertices[i0].nx += n.x;
        vertices[i0].ny += n.y;
        vertices[i0].nz += n.z;
        vertices[i1].nx += n.x;
        vertices[i1].ny += n.y;
        vertices[i1].nz += n.z;
        vertices[i2].nx += n.x;
        vertices[i2].ny += n.y;
        vertices[i2].nz += n.z;
    }

    // Normalize
    for v in vertices.iter_mut() {
        let len = (v.nx * v.nx + v.ny * v.ny + v.nz * v.nz).sqrt();
        if len > 0.0 {
            v.nx /= len;
            v.ny /= len;
            v.nz /= len;
        }
    }
}

/// Rotate a vector around an axis using Rodrigues' rotation formula
pub(super) fn get_rotated_axis(angle_degrees: f32, axis: &Vec3, temp: &Vec3) -> Vec3 {
    let u = axis.normalize();
    let angle_rad = angle_degrees * PI / 180.0;
    let sin_angle = angle_rad.sin();
    let cos_angle = angle_rad.cos();
    let one_minus_cos = 1.0 - cos_angle;

    let rot_row0 = Vec3 {
        x: u.x * u.x + cos_angle * (1.0 - u.x * u.x),
        y: u.x * u.y * one_minus_cos - sin_angle * u.z,
        z: u.x * u.z * one_minus_cos + sin_angle * u.y,
    };
    let rot_row1 = Vec3 {
        x: u.x * u.y * one_minus_cos + sin_angle * u.z,
        y: u.y * u.y + cos_angle * (1.0 - u.y * u.y),
        z: u.y * u.z * one_minus_cos - sin_angle * u.x,
    };
    let rot_row2 = Vec3 {
        x: u.x * u.z * one_minus_cos - sin_angle * u.y,
        y: u.y * u.z * one_minus_cos + sin_angle * u.x,
        z: u.z * u.z + cos_angle * (1.0 - u.z * u.z),
    };

    Vec3 {
        x: temp.x * rot_row0.x + temp.y * rot_row0.y + temp.z * rot_row0.z,
        y: temp.x * rot_row1.x + temp.y * rot_row1.y + temp.z * rot_row1.z,
        z: temp.x * rot_row2.x + temp.y * rot_row2.y + temp.z * rot_row2.z,
    }
}

/// Initialize cubic spline coefficients for p(s) = c0 + c1*s + c2*s^2 + c3*s^3
pub(super) fn init_cubic_spline_coeffs(x0: f32, x1: f32, t0: f32, t1: f32) -> (f32, f32, f32, f32) {
    let c0 = x0;
    let c1 = t0;
    let c2 = -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1;
    let c3 = 2.0 * x0 - 2.0 * x1 + t0 + t1;
    (c0, c1, c2, c3)
}

/// Initialize non-uniform Catmull-Rom spline coefficients
pub(super) fn init_nonuniform_catmull_coeffs(
    x0: f32,
    x1: f32,
    x2: f32,
    x3: f32,
    dt0: f32,
    dt1: f32,
    dt2: f32,
) -> (f32, f32, f32, f32) {
    // Compute tangents when parameterized in [t1,t2]
    let mut t1_tang = (x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1;
    let mut t2_tang = (x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2;

    // Rescale tangents for parametrization in [0,1]
    t1_tang *= dt1;
    t2_tang *= dt1;

    init_cubic_spline_coeffs(x1, x2, t1_tang, t2_tang)
}
