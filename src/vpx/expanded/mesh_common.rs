//! Common utilities for mesh generation in walls and ramps
//!
//! This module contains shared types and functions used by both wall and ramp
//! mesh generation to avoid code duplication.

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
